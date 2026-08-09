// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// Package profile defines the versioned Lua language and library surface that
// package code is allowed to observe.
package profile

import (
	"bytes"
	_ "embed"
	"errors"
	"fmt"
	"io"
	"unicode/utf8"

	"github.com/arnodel/golua/lib/base"
	"github.com/arnodel/golua/lib/coroutine"
	"github.com/arnodel/golua/lib/mathlib"
	"github.com/arnodel/golua/lib/stringlib"
	"github.com/arnodel/golua/lib/tablelib"
	"github.com/arnodel/golua/lib/utf8lib"
	rt "github.com/arnodel/golua/runtime"
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
	if bytes.HasPrefix(source, []byte("\x1bLua")) || rt.HasMarshalPrefix(source) {
		return ErrBytecode
	}
	return nil
}

// Runtime owns a production-configured Lua runtime and its library cleanup
// callbacks. Callers must not share one Runtime between Sessions.
type Runtime struct {
	lua      *rt.Runtime
	cleanups []func()
}

// Result is an opaque value owned by one production Runtime. It intentionally
// does not expose the candidate's raw table, function, thread or userdata APIs.
type Result struct {
	value rt.Value
}

func (r Result) IsNil() bool {
	return r.value.IsNil()
}

// CheckpointValue copies a basic result out of the VM. Opaque runtime values
// fail closed in the checkpoint converter.
func (r Result) CheckpointValue() (checkpoint.Value, error) {
	return checkpoint.FromLua(r.value)
}

// NewProductionRuntime constructs a runtime using an explicit library
// allowlist. Dangerous libraries are never loaded and cannot be enabled by an
// option or environment variable.
func NewProductionRuntime(stdout io.Writer) *Runtime {
	if stdout == nil {
		stdout = io.Discard
	}
	r := rt.New(stdout)
	r.SetWarner(rt.NewLogWarner(io.Discard, ""))
	result := &Runtime{lua: r}

	load := func(name string, loader func(*rt.Runtime) (rt.Value, func())) rt.Value {
		value, cleanup := loader(r)
		if cleanup != nil {
			result.cleanups = append(result.cleanups, cleanup)
		}
		if name != "" {
			r.SetEnv(r.GlobalEnv(), name, value)
		}
		return value
	}

	load("", base.LibLoader.Load)
	load("coroutine", coroutine.LibLoader.Load)
	stringValue := load("string", stringlib.LibLoader.Load)
	load("table", tablelib.LibLoader.Load)
	load("utf8", utf8lib.LibLoader.Load)
	load("math", mathlib.LibLoader.Load)

	// The platform accepts code only through its validated host boundary.
	// Removing all Lua-visible loaders also prevents the runtime's private
	// marshalled-code format from becoming a bytecode bypass.
	for _, name := range []string{"load", "loadfile", "dofile"} {
		r.SetEnv(r.GlobalEnv(), name, rt.NilValue)
	}

	// string.dump creates the implementation's private bytecode format.
	if stringTable, ok := stringValue.TryTable(); ok {
		r.SetEnv(stringTable, "dump", rt.NilValue)
	}

	// Package loading, native Go imports, OS/process/filesystem access and the
	// debug library are absent because their loaders are not invoked. Set the
	// names explicitly to nil so this invariant remains visible and testable.
	for _, name := range []string{"package", "require", "io", "os", "debug", "golib", "runtime"} {
		r.SetEnv(r.GlobalEnv(), name, rt.NilValue)
	}
	r.SetEnv(r.GlobalEnv(), "_VERSION", rt.StringValue("Lua 5.5"))

	return result
}

// Compile validates UTF-8/source-only input and compiles it as text. It never
// calls the candidate's source-or-bytecode loader.
func (r *Runtime) compile(name string, source []byte) (*rt.Closure, error) {
	if err := ValidateSource(source); err != nil {
		return nil, err
	}
	if name == "" {
		name = "chunk"
	}
	return r.lua.CompileAndLoadLuaChunk(name, source, rt.TableValue(r.lua.GlobalEnv()))
}

// Execute is the only code-loading operation exposed by a production Runtime.
// It returns a runtime value but never the underlying runtime itself, so callers
// cannot load additional libraries or select a bytecode-aware loader.
func (r *Runtime) Execute(name string, source []byte) (Result, error) {
	chunk, err := r.compile(name, source)
	if err != nil {
		return Result{}, err
	}
	value, err := rt.Call1(r.lua.MainThread(), rt.FunctionValue(chunk))
	return Result{value: value}, err
}

// Close releases library resources and the underlying runtime. A Runtime is
// owned by its VM, which guarantees this method is called once.
func (r *Runtime) Close() error {
	for i := len(r.cleanups) - 1; i >= 0; i-- {
		r.cleanups[i]()
	}
	r.cleanups = nil
	var closeErr error
	r.lua.Close(&closeErr)
	return closeErr
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

//go:embed licenses/Apache-2.0.txt
var apacheLicenseText string

// RuntimeLicenseText is embedded in lua-runner so binary distributions can
// reproduce the selected runtime's complete license.
func RuntimeLicenseText() string {
	return apacheLicenseText
}

// RuntimeCandidate returns the exact Lua 5.5 implementation selected for p1.
func RuntimeCandidate() Candidate {
	return Candidate{
		Module:        RuntimeModule,
		Version:       RuntimeVersion,
		Commit:        "1de6171e59db11713ce0e7f9ccbbba95f5ccad15",
		Upstream:      "https://github.com/arnodel/golua",
		License:       "Apache-2.0",
		LicenseSHA256: "abe774ad370d66aebc12a4aaeba3cb978a6e72e6823b0af6e3b56ec0473760ad",
		Reason:        "pure-Go Lua 5.5 runtime with isolated runtime instances and no native toolchain dependency",
		ModuleGraph: []Dependency{
			{
				Module:   "github.com/arnodel/strftime",
				Version:  "v0.1.6",
				Upstream: "https://github.com/arnodel/strftime",
				License:  "MIT",
				Linked:   false,
				Reason:   "declared by the candidate module; not linked into the production lua-runner dependency set",
			},
		},
	}
}
