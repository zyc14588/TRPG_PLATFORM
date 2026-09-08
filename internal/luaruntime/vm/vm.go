// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// Package vm provides the per-Session Lua VM lifecycle primitive. Each VM owns
// exactly one Lua runtime, module cache, random state, registry and capability
// namespace.
package vm

import (
	"context"
	"crypto/rand"
	"crypto/subtle"
	"errors"
	"fmt"
	"io"
	"strings"
	"sync"
	"unicode/utf8"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
)

var (
	ErrDestroyed         = errors.New("Lua VM is destroyed")
	ErrPoisoned          = errors.New("Lua VM is poisoned and must be reconstructed")
	ErrBusy              = errors.New("Lua VM has an operation in progress")
	ErrInvalidSession    = errors.New("invalid Session identity")
	ErrInvalidModule     = errors.New("invalid Lua module identity")
	ErrModuleLoaded      = errors.New("Lua module is already loaded")
	ErrModuleNotLoaded   = errors.New("Lua module is not loaded")
	ErrInvalidCapability = errors.New("invalid capability handle")
	ErrForeignCapability = errors.New("capability handle belongs to another VM")
	ErrBindingMismatch   = errors.New("checkpoint binding does not match VM")
)

type Config struct {
	SessionID      string
	ProfileID      string
	RuntimeVersion string
	Stdout         io.Writer
}

// ProductionConfig cannot be used to enable debug or dangerous libraries.
func ProductionConfig(sessionID string) Config {
	p := profile.Production()
	return Config{SessionID: sessionID, ProfileID: p.ID, RuntimeVersion: p.RuntimeVersion, Stdout: io.Discard}
}

type state uint8

const (
	stateLive state = iota + 1
	statePoisoned
	stateDestroyed
)

// CapabilityHandle is opaque outside this package. It is intentionally not a
// checkpoint-supported value.
type CapabilityHandle struct {
	owner  [32]byte
	serial uint64
	label  string
}

// RecoveryState contains only explicit reconstruction inputs.
type RecoveryState struct {
	Authoritative checkpoint.Value
	Checkpoint    checkpoint.Value
}

type VM struct {
	mu sync.Mutex

	config       Config
	profile      profile.Spec
	state        state
	runtime      *profile.Runtime
	modules      map[string]profile.Result
	owner        [32]byte
	nextHandle   uint64
	capabilities map[uint64]string
	recovery     *RecoveryState
}

func validateConfig(config Config) (profile.Spec, error) {
	if strings.TrimSpace(config.SessionID) == "" || len(config.SessionID) > 256 || !utf8.ValidString(config.SessionID) {
		return profile.Spec{}, ErrInvalidSession
	}
	p, err := profile.Resolve(config.ProfileID, config.RuntimeVersion)
	if err != nil {
		return profile.Spec{}, err
	}
	return p, nil
}

// New creates a fresh, unshared long-lived VM.
func New(config Config) (*VM, error) {
	p, err := validateConfig(config)
	if err != nil {
		return nil, err
	}
	var owner [32]byte
	if _, err := io.ReadFull(rand.Reader, owner[:]); err != nil {
		return nil, fmt.Errorf("create capability namespace: %w", err)
	}
	return &VM{
		config:       config,
		profile:      p,
		state:        stateLive,
		runtime:      profile.NewProductionRuntime(config.Stdout),
		modules:      make(map[string]profile.Result),
		owner:        owner,
		capabilities: make(map[uint64]string),
	}, nil
}

func (v *VM) acquire(ctx context.Context) error {
	if ctx == nil {
		ctx = context.Background()
	}
	if err := ctx.Err(); err != nil {
		return err
	}
	if !v.mu.TryLock() {
		return ErrBusy
	}
	if v.state == statePoisoned {
		v.mu.Unlock()
		return ErrPoisoned
	}
	if v.state != stateLive || v.runtime == nil {
		v.mu.Unlock()
		return ErrDestroyed
	}
	return nil
}

func (v *VM) release() {
	v.mu.Unlock()
}

func (v *VM) evalLocked(name string, source []byte) (checkpoint.Value, error) {
	result, err := v.runtime.Execute(name, source)
	if err != nil {
		v.state = statePoisoned
		return checkpoint.Value{}, err
	}
	return result.CheckpointValue()
}

// Eval executes one UTF-8 source chunk and copies its single return value into
// checkpoint-safe data. The VM remains alive after the call.
func (v *VM) Eval(ctx context.Context, name string, source []byte) (checkpoint.Value, error) {
	if err := v.acquire(ctx); err != nil {
		return checkpoint.Value{}, err
	}
	defer v.release()
	result, err := v.evalLocked(name, source)
	if err != nil {
		return checkpoint.Value{}, err
	}
	if ctx != nil {
		if err := ctx.Err(); err != nil {
			v.state = statePoisoned
			return checkpoint.Value{}, err
		}
	}
	return result, nil
}

// Exec executes a source chunk when its return value is not needed.
func (v *VM) Exec(ctx context.Context, name string, source []byte) error {
	_, err := v.Eval(ctx, name, source)
	return err
}

func validModuleName(name string) bool {
	return strings.TrimSpace(name) != "" && len(name) <= 256 && utf8.ValidString(name) && !strings.ContainsRune(name, '\x00')
}

// LoadModule compiles and executes one source module into this VM's private
// module cache. Lua-visible require/package/searchers remain absent.
func (v *VM) LoadModule(ctx context.Context, name string, source []byte) error {
	if !validModuleName(name) {
		return ErrInvalidModule
	}
	if err := v.acquire(ctx); err != nil {
		return err
	}
	defer v.release()
	if _, exists := v.modules[name]; exists {
		return ErrModuleLoaded
	}
	result, err := v.runtime.Execute("module:"+name, source)
	if err != nil {
		v.state = statePoisoned
		return err
	}
	if result.IsNil() {
		result, err = v.runtime.Execute("module:"+name+":default", []byte("return true"))
		if err != nil {
			v.state = statePoisoned
			return err
		}
	}
	v.modules[name] = result
	return nil
}

func (v *VM) HasModule(ctx context.Context, name string) (bool, error) {
	if !validModuleName(name) {
		return false, ErrInvalidModule
	}
	if err := v.acquire(ctx); err != nil {
		return false, err
	}
	defer v.release()
	_, exists := v.modules[name]
	return exists, nil
}

// ModuleValue returns a data copy, never a raw cross-VM runtime reference.
func (v *VM) ModuleValue(ctx context.Context, name string) (checkpoint.Value, error) {
	if !validModuleName(name) {
		return checkpoint.Value{}, ErrInvalidModule
	}
	if err := v.acquire(ctx); err != nil {
		return checkpoint.Value{}, err
	}
	defer v.release()
	value, exists := v.modules[name]
	if !exists {
		return checkpoint.Value{}, ErrModuleNotLoaded
	}
	return value.CheckpointValue()
}

// IssueCapability creates a VM-local opaque handle.
func (v *VM) IssueCapability(ctx context.Context, label string) (CapabilityHandle, error) {
	if strings.TrimSpace(label) == "" || !utf8.ValidString(label) || len(label) > 256 {
		return CapabilityHandle{}, ErrInvalidCapability
	}
	if err := v.acquire(ctx); err != nil {
		return CapabilityHandle{}, err
	}
	defer v.release()
	v.nextHandle++
	if v.nextHandle == 0 {
		return CapabilityHandle{}, ErrInvalidCapability
	}
	v.capabilities[v.nextHandle] = label
	return CapabilityHandle{owner: v.owner, serial: v.nextHandle, label: label}, nil
}

// CheckCapability rejects forged, revoked, cross-VM and stale handles.
func (v *VM) CheckCapability(ctx context.Context, handle CapabilityHandle, label string) error {
	if err := v.acquire(ctx); err != nil {
		return err
	}
	defer v.release()
	if subtle.ConstantTimeCompare(v.owner[:], handle.owner[:]) != 1 {
		return ErrForeignCapability
	}
	stored, exists := v.capabilities[handle.serial]
	if !exists || stored != handle.label || stored != label {
		return ErrInvalidCapability
	}
	return nil
}

// Destroy closes exactly this VM. It returns ErrBusy instead of racing with an
// active operation and makes every later operation fail with ErrDestroyed.
func (v *VM) Destroy(ctx context.Context) error {
	if ctx == nil {
		ctx = context.Background()
	}
	if err := ctx.Err(); err != nil {
		return err
	}
	if !v.mu.TryLock() {
		return ErrBusy
	}
	defer v.mu.Unlock()
	if v.state == stateDestroyed || v.runtime == nil {
		return ErrDestroyed
	}
	err := v.runtime.Close()
	v.runtime = nil
	v.modules = nil
	v.capabilities = nil
	v.recovery = nil
	v.owner = [32]byte{}
	v.state = stateDestroyed
	return err
}
