// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package profile

import (
	"context"
	"crypto/sha256"
	"fmt"
	"strings"

	lua "github.com/iceisfun/golua/v2/vm"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
)

const HostMajor = 1
const HostMinor = 0
const MaxHostCallbacks = 256

// HostConfig is runner-internal data. Grants and execution tokens never travel
// to the child; the parent independently authorizes every callback.
type HostConfig struct {
	Entry         string   `json:"entry"`
	Required      []string `json:"required"`
	CallbackLimit int      `json:"callback_limit"`
}
type HostCall struct {
	Module     string             `json:"module"`
	Line       int                `json:"line"`
	Phase      string             `json:"phase"`
	Capability string             `json:"capability"`
	Operation  string             `json:"operation"`
	Arguments  []checkpoint.Value `json:"arguments"`
}
type HostHandler func(context.Context, HostCall) (checkpoint.Value, error)

var standardCallbacks = []string{"on_session_create", "on_session_restore", "on_session_start", "validate_command", "execute_command", "list_legal_actions", "project_view", "create_checkpoint", "restore_checkpoint", "resume_continuation", "on_safe_migration_boundary", "on_session_end", "cleanup"}
var hostOperations = map[string][]string{
	"state": {"get", "put", "delete"}, "event": {"emit"}, "random": {"next"}, "time": {"now"}, "content": {"get"},
	"db": {"get", "put", "delete", "list", "compare_and_set", "named"}, "task": {"create", "continuation"}, "ai": {"request"}, "rules": {"call"}, "log": {"write"},
}

func (e *Engine) configureHost(c HostConfig) error {
	if _, ok := e.modules[c.Entry]; !ok || c.CallbackLimit < 1 || c.CallbackLimit > MaxHostCallbacks {
		return Fail(ErrConfiguration)
	}
	allowed := map[string]bool{}
	for _, n := range standardCallbacks {
		allowed[n] = true
	}
	seen := map[string]bool{}
	for _, n := range c.Required {
		if !allowed[n] || seen[n] {
			return Fail(ErrConfiguration)
		}
		seen[n] = true
	}
	if !seen["validate_command"] || !seen["execute_command"] {
		return Fail(ErrConfiguration)
	}
	clone := c
	clone.Required = append([]string(nil), c.Required...)
	e.host = &clone
	e.callbacks = map[string]lua.Value{}
	host := lua.NewEmptyTable()
	for category, operations := range hostOperations {
		group := lua.NewEmptyTable()
		for _, operation := range operations {
			cap, op := "host."+category, operation
			if err := group.Set(lua.NewString(op), lua.NewNativeFunc(func(v *lua.VM) int { return e.callHost(v, cap, op) })); err != nil {
				return Fail(ErrConfiguration)
			}
		}
		if err := host.Set(lua.NewString(category), lua.NewTable(group)); err != nil {
			return Fail(ErrConfiguration)
		}
	}
	e.runtime.SetGlobal("host", lua.NewTable(host))
	return nil
}
func (e *Engine) hostFail(code string) {
	if a := e.active.Load(); a != nil {
		a.hostFailure.CompareAndSwap(nil, &Failure{Code: code})
		a.failed.Store(true)
	}
	panic(code)
}
func (e *Engine) callHost(v *lua.VM, cap, op string) int {
	a := e.active.Load()
	if a == nil || e.host == nil || e.hostHandler == nil {
		e.hostFail(ErrCapability)
	}
	a.callbacks++
	if a.callbacks > e.host.CallbackLimit || a.ctx.Err() != nil {
		e.hostFail(ErrBudget)
	}
	module := ""
	line := 0
	// Native pcall/coroutine frames are skipped. The nearest actual Lua proto
	// owns the call, including functions returned by a dependency or cached later.
	for depth := 0; depth <= e.limits.CallDepth; depth++ {
		f := v.GetFrameInfo(depth)
		if f == nil {
			break
		}
		if f.What == "C" {
			continue
		}
		if strings.HasPrefix(f.Source, "@") {
			module = strings.TrimPrefix(f.Source, "@")
			line = f.CurrentLine
		}
		break
	}
	if _, ok := e.modules[module]; !ok {
		e.hostFail(ErrCapability)
	}
	args := make([]checkpoint.Value, 0, v.ArgCount())
	nodes, bytes := 0, 0
	for i := 1; i <= v.ArgCount(); i++ {
		x, err := e.fromLua(v.Get(i), map[lua.LuaTable]bool{}, 0, &nodes, &bytes)
		if err != nil {
			e.hostFail(ErrValue)
		}
		args = append(args, x)
	}
	value, err := e.hostHandler(a.ctx, HostCall{Module: module, Line: line, Phase: e.phase, Capability: cap, Operation: op, Arguments: args})
	if err != nil {
		e.hostFail(Code(err))
	}
	if checkpoint.Validate(value) != nil {
		e.hostFail(ErrValue)
	}
	v.Set(0, e.toLua(value))
	return 1
}

// LoadHostEntrypoint retains only the standard Lua functions, never serialized
// closures or a caller-supplied executable string.
func (e *Engine) LoadHostEntrypoint(ctx context.Context) (Result, error) {
	if e.host == nil {
		return Result{}, Fail(ErrConfiguration)
	}
	return e.run(ctx, "host-load", nil, func() ([]lua.Value, error) {
		p, err := compileNamed(e.modules[e.host.Entry], "@"+e.host.Entry)
		if err != nil {
			return nil, err
		}
		values, err := e.runtime.Run(p)
		if err != nil {
			return nil, err
		}
		if len(values) != 1 || !values[0].IsTable() {
			return nil, Fail(ErrConfiguration)
		}
		table := values[0].AsTable()
		for _, n := range e.host.Required {
			f := table.Get(lua.NewString(n))
			if !f.IsFunction() {
				e.hostFail(ErrConfiguration)
			}
			e.callbacks[n] = f
		}
		for _, n := range standardCallbacks {
			f := table.Get(lua.NewString(n))
			if f.IsFunction() {
				e.callbacks[n] = f
			}
		}
		return nil, nil
	})
}

func (e *Engine) InvokeHost(ctx context.Context, name string, args []checkpoint.Value, handler HostHandler) (Result, error) {
	if e.host == nil || handler == nil {
		return Result{}, Fail(ErrConfiguration)
	}
	return e.run(ctx, "host-"+name, handler, func() ([]lua.Value, error) {
		call := func(n string) ([]lua.Value, error) {
			f, ok := e.callbacks[n]
			if !ok {
				return nil, Fail(ErrConfiguration)
			}
			arguments := make([]lua.Value, len(args))
			for i, x := range args {
				if checkpoint.Validate(x) != nil {
					return nil, Fail(ErrValue)
				}
				arguments[i] = e.toLua(x)
			}
			return e.runtime.ProtectedCall(f, arguments)
		}
		if name == "command" {
			e.phase = "validate"
			valid, err := call("validate_command")
			if err != nil {
				return nil, err
			}
			if len(valid) != 1 || !valid[0].IsBool() || !valid[0].AsBool() {
				e.hostFail("COMMAND_VALIDATION_FAILED")
			}
			if a := e.active.Load(); a.failed.Load() {
				return nil, Fail(ErrCapability)
			}
			e.phase = "execute"
			return call("execute_command")
		}
		if name == "list_legal_actions" || name == "project_view" || name == "create_checkpoint" || name == "on_safe_migration_boundary" {
			e.phase = "read"
		} else {
			e.phase = "execute"
		}
		return call(name)
	})
}

// Only digests of host-mode print output may leave the runner. AUDIT policies
// cannot turn these into raw credential-bearing logs.
func redactHostOutput(parts []string) string {
	return fmt.Sprintf("sha256:%x", sha256.Sum256([]byte(strings.Join(parts, "\t"))))
}

func KnownHostOperation(capability, operation string) bool {
	category, ok := strings.CutPrefix(capability, "host.")
	if !ok {
		return false
	}
	for _, op := range hostOperations[category] {
		if op == operation {
			return true
		}
	}
	return false
}

func StandardCallbackNames() []string { return append([]string(nil), standardCallbacks...) }
func IsStandardCallback(name string) bool {
	for _, n := range standardCallbacks {
		if n == name {
			return true
		}
	}
	return false
}
