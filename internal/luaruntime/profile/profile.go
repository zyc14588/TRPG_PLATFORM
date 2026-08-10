// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// Package profile defines the versioned Lua language and library surface that
// package code is allowed to observe.
package profile

import (
	"bytes"
	"context"
	_ "embed"
	"errors"
	"fmt"
	"io"
	"unicode/utf8"

	"github.com/iceisfun/golua/v2/compiler"
	"github.com/iceisfun/golua/v2/parser"
	"github.com/iceisfun/golua/v2/stdlib"
	luavm "github.com/iceisfun/golua/v2/vm"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile/identity"
)

const (
	// ProductionID is the stable identity of the first platform Lua profile.
	ProductionID = identity.ProductionID

	LanguageVersion = identity.LanguageVersion
	RuntimeModule   = identity.RuntimeModule
	RuntimeVersion  = identity.RuntimeVersion
	RuntimeIdentity = identity.RuntimeIdentity
)

var (
	ErrUnknownProfile        = errors.New("unknown Lua platform profile")
	ErrRuntimeMismatch       = errors.New("Lua runtime version does not match profile")
	ErrInvalidUTF8           = errors.New("Lua source is not valid UTF-8")
	ErrBytecode              = errors.New("Lua bytecode is forbidden by the source-only profile")
	ErrProductionDebugDenied = errors.New("production Lua debugging is forbidden")
	ErrRuntimeClosed         = errors.New("Lua runtime is closed")
)

const (
	// RecoveryAuthoritativeGlobal and RecoveryCheckpointGlobal are the standard
	// read/write inputs visible while a deterministic restore program runs.
	RecoveryAuthoritativeGlobal = "__trpg_authoritative_state"
	RecoveryCheckpointGlobal    = "__trpg_checkpoint_state"
)

// Spec is an immutable identity for a supported platform profile.
type Spec struct {
	ID              string
	LanguageVersion string
	RuntimeVersion  string
	Production      bool
}

// Production returns the only profile supported by this batch.
func Production() Spec {
	return Spec{
		ID:              ProductionID,
		LanguageVersion: LanguageVersion,
		RuntimeVersion:  RuntimeIdentity,
		Production:      true,
	}
}

// Resolve fails closed for unknown profiles and runtime versions. In
// particular, it never maps an unknown identity to a default or latest value.
func Resolve(profileID, runtimeVersion string) (Spec, error) {
	p := Production()
	if profileID != p.ID {
		return Spec{}, fmt.Errorf("%w: %q", ErrUnknownProfile, profileID)
	}
	if runtimeVersion != p.RuntimeVersion {
		return Spec{}, fmt.Errorf("%w: got %q, require %q", ErrRuntimeMismatch, runtimeVersion, p.RuntimeVersion)
	}
	return p, nil
}

// ValidateSource enforces the source-only input boundary before parsing.
func ValidateSource(source []byte) error {
	if !utf8.Valid(source) {
		return ErrInvalidUTF8
	}
	if bytes.HasPrefix(source, []byte("\x1bLua")) {
		return ErrBytecode
	}
	return nil
}

// Runtime owns a production-configured Lua runtime and its library cleanup
// callbacks. Callers must not share one Runtime between Sessions.
type Runtime struct {
	lua *luavm.VM
}

// Result is an opaque value owned by one production Runtime. It intentionally
// does not expose the candidate's raw table, function, thread or userdata APIs.
type Result struct {
	value luavm.Value
}

func (r Result) IsNil() bool {
	return r.value.IsNil()
}

// CheckpointValue copies a basic result out of the VM. Opaque runtime values
// fail closed in the checkpoint converter.
func (r Result) CheckpointValue() (checkpoint.Value, error) {
	return checkpoint.FromLua(r.value)
}

type outputProvider struct {
	writer io.Writer
}

func (p outputProvider) Print(_ context.Context, message string) {
	fmt.Fprintln(p.writer, message)
}

func (p outputProvider) Warn(_ context.Context, message string) {
	fmt.Fprintln(p.writer, message)
}

// NewProductionRuntime constructs a runtime using an explicit library
// allowlist. Dangerous libraries cannot be enabled by an option or environment
// variable and are removed before any package source can execute.
func NewProductionRuntime(stdout io.Writer) *Runtime {
	if stdout == nil {
		stdout = io.Discard
	}
	r := luavm.New()
	_ = r.SetPrintProvider(outputProvider{writer: stdout})
	stdlib.Open(r)

	// The platform accepts code only through its validated host boundary.
	// Removing all Lua-visible loaders prevents the runtime's binary chunk
	// format from becoming a bytecode bypass.
	if stringTable := r.GetGlobal("string").AsTable(); stringTable != nil {
		_ = stringTable.Set(luavm.NewString("dump"), luavm.Nil)
	}
	if mathTable := r.GetGlobal("math").AsTable(); mathTable != nil {
		// These are compatibility extensions in the selected runtime, not part
		// of the frozen Lua 5.5 platform math surface.
		_ = mathTable.Set(luavm.NewString("frexp"), luavm.Nil)
		_ = mathTable.Set(luavm.NewString("ldexp"), luavm.Nil)
	}

	// Package loading, native Go imports, runtime extensions and optional
	// OS/process/filesystem/debug providers are not part of the platform profile.
	// Set every corresponding global explicitly to nil so this invariant remains
	// visible and testable even if the upstream default library set changes.
	for _, name := range []string{
		"load", "loadfile", "dofile", "package", "require",
		"io", "os", "debug", "chan", "time", "exec", "http",
		"golib", "runtime", "bit32", "glob", "_lastoutput", "_outputlines",
	} {
		r.SetGlobal(name, luavm.Nil)
	}

	return &Runtime{lua: r}
}

// Compile validates UTF-8/source-only input and compiles it as text. It never
// calls the candidate's source-or-bytecode loader.
func (r *Runtime) compile(name string, source []byte) (*compiler.Proto, error) {
	if err := ValidateSource(source); err != nil {
		return nil, err
	}
	if name == "" {
		name = "chunk"
	}
	block, err := parser.Parse(name, string(source), false)
	if err != nil {
		return nil, err
	}
	return compiler.Compile(name, block)
}

// Execute is the only code-loading operation exposed by a production Runtime.
// It returns a runtime value but never the underlying runtime itself, so callers
// cannot load additional libraries or select a bytecode-aware loader.
func (r *Runtime) Execute(name string, source []byte) (Result, error) {
	if r == nil || r.lua == nil {
		return Result{}, ErrRuntimeClosed
	}
	chunk, err := r.compile(name, source)
	if err != nil {
		return Result{}, err
	}
	values, err := r.lua.Run(chunk)
	if err != nil {
		return Result{}, err
	}
	if len(values) == 0 {
		return Result{value: luavm.Nil}, nil
	}
	return Result{value: values[0]}, nil
}

// InstallRecoveryInputs deep-copies canonical authoritative and checkpoint
// values into the fresh runtime. The two values are installed only after both
// conversions succeed, so a restore program never observes a partial pair.
func (r *Runtime) InstallRecoveryInputs(authoritative, explicit checkpoint.Value) error {
	if r == nil || r.lua == nil {
		return ErrRuntimeClosed
	}
	authoritativeLua, err := checkpoint.ToLua(r.lua, authoritative)
	if err != nil {
		return fmt.Errorf("authoritative recovery input: %w", err)
	}
	checkpointLua, err := checkpoint.ToLua(r.lua, explicit)
	if err != nil {
		return fmt.Errorf("checkpoint recovery input: %w", err)
	}
	r.lua.SetGlobal(RecoveryAuthoritativeGlobal, authoritativeLua)
	r.lua.SetGlobal(RecoveryCheckpointGlobal, checkpointLua)
	return nil
}

// Close releases library resources and the underlying runtime. A Runtime is
// owned by its VM, which guarantees this method is called once.
func (r *Runtime) Close() error {
	if r.lua == nil {
		return nil
	}
	err := r.lua.Close(context.Background())
	r.lua = nil
	return err
}

// Candidate records the audited dependency identity in machine-readable Go
// data without changing repository licensing policy.
type Candidate struct {
	Module        string
	Version       string
	Commit        string
	Upstream      string
	License       string
	LicenseSHA256 string
	Reason        string
	ModuleGraph   []Dependency
}

type Dependency struct {
	Module   string
	Version  string
	Upstream string
	License  string
	Linked   bool
	Reason   string
}

//go:embed licenses/MIT.txt
var runtimeLicenseText string

// RuntimeLicenseText is embedded in lua-runner so binary distributions can
// reproduce the selected runtime's complete license.
func RuntimeLicenseText() string {
	return runtimeLicenseText
}

// RuntimeCandidate returns the exact Lua 5.5 implementation selected for p1.
func RuntimeCandidate() Candidate {
	return Candidate{
		Module:        RuntimeModule,
		Version:       RuntimeVersion,
		Commit:        "5c098c0c2a4301b7b2ee0ccaa12e6fc2ba10a2ad",
		Upstream:      "https://github.com/iceisfun/golua",
		License:       "MIT",
		LicenseSHA256: "a999ae4a02393c1044dd71ce592028984064550f0cceec71f4d1f83da537dd1f",
		Reason:        "pure-Go Lua 5.5.0 runtime with isolated VM instances, zero module dependencies, and no native toolchain dependency",
		ModuleGraph:   nil,
	}
}
