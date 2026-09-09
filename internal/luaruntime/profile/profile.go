// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// Package profile is the source-only engine hosted exclusively in a production
// runner process. Hosts use the vm package; New is also available to focused tests.
package profile

import (
	"context"
	"errors"
	"fmt"
	"math"
	"path"
	"sort"
	"strconv"
	"strings"
	"sync"
	"sync/atomic"
	"time"
	"unicode/utf8"
	"weak"

	"github.com/iceisfun/golua/v2/compiler"
	"github.com/iceisfun/golua/v2/parser"
	"github.com/iceisfun/golua/v2/stdlib"
	lua "github.com/iceisfun/golua/v2/vm"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
)

const (
	ID               = "platform-lua-5.5-p1"
	RuntimeVersion   = "golua-v2.0.5/p1"
	MaxSourceBytes   = 256 << 10
	MaxModuleBytes   = 1 << 20
	ErrSource        = "SOURCE_REJECTED"
	ErrScript        = "SCRIPT_FAILED"
	ErrBudget        = "BUDGET_EXCEEDED"
	ErrValue         = "CHECKPOINT_VALUE_REJECTED"
	ErrPoisoned      = "VM_POISONED"
	ErrDestroyed     = "VM_DESTROYED"
	ErrCapability    = "CAPABILITY_DENIED"
	ErrConfiguration = "CONFIGURATION_REJECTED"
)

type Failure struct {
	Code string `json:"code"`
}

func (e *Failure) Error() string { return e.Code }
func Code(err error) string {
	var e *Failure
	if errors.As(err, &e) {
		return e.Code
	}
	if errors.Is(err, context.Canceled) {
		return "EXECUTION_CANCELLED"
	}
	if errors.Is(err, context.DeadlineExceeded) {
		return "WALL_CLOCK_EXCEEDED"
	}
	if err != nil {
		return ErrScript
	}
	return "PASS"
}
func Fail(code string) error { return &Failure{Code: code} }

type Limits struct {
	Instructions int64  `json:"instructions"`
	WallMillis   int    `json:"wall_millis"`
	MemoryBytes  uint64 `json:"memory_bytes"`
	CPUSeconds   uint64 `json:"cpu_seconds"`
	CallDepth    int    `json:"call_depth"`
	OutputBytes  int    `json:"output_bytes"`
}

func DefaultLimits() Limits {
	return Limits{Instructions: 100000, WallMillis: 500, MemoryBytes: 128 << 20, CPUSeconds: 2, CallDepth: 128, OutputBytes: 64 << 10}
}
func (l Limits) Validate() error {
	d := DefaultLimits()
	if l.Instructions < 1 || l.Instructions > d.Instructions || l.WallMillis < 1 || l.WallMillis > d.WallMillis || l.MemoryBytes < 64<<20 || l.MemoryBytes > d.MemoryBytes || l.CPUSeconds < 1 || l.CPUSeconds > d.CPUSeconds || l.CallDepth < 8 || l.CallDepth > d.CallDepth || l.OutputBytes < 1 || l.OutputBytes > d.OutputBytes {
		return Fail(ErrConfiguration)
	}
	return nil
}

type Config struct {
	Limits  Limits            `json:"limits"`
	Modules map[string][]byte `json:"modules,omitempty"`
}
type Audit struct {
	Level    string `json:"level"`
	Sequence uint64 `json:"sequence"`
	Outcome  string `json:"outcome"`
	Kind     string `json:"kind"`
}
type Result struct {
	Values []checkpoint.Value `json:"values"`
	Output []string           `json:"output"`
	Audit  Audit              `json:"audit"`
}

type invocation struct {
	ctx    context.Context
	count  atomic.Int64
	failed atomic.Bool
}

type Engine struct {
	mu          sync.Mutex
	runtime     *lua.VM
	limits      Limits
	modules     map[string][]byte
	loaded      map[string]lua.Value
	shapes      map[weak.Pointer[lua.Table]]*checkpointShape
	loading     map[string]bool
	depthBase   map[*lua.VM]int
	active      atomic.Pointer[invocation]
	poisoned    bool
	destroyed   bool
	sequence    uint64
	output      []string
	outputBytes int
	hook        lua.Value
	life        context.Context
	cancel      context.CancelFunc
}

func New(config Config) (*Engine, error) {
	if err := config.Limits.Validate(); err != nil {
		return nil, err
	}
	e := &Engine{limits: config.Limits, modules: map[string][]byte{}, loaded: map[string]lua.Value{}, shapes: map[weak.Pointer[lua.Table]]*checkpointShape{}, loading: map[string]bool{}, depthBase: map[*lua.VM]int{}}
	if len(config.Modules) > 256 {
		return nil, Fail(ErrConfiguration)
	}
	total := 0
	for name, source := range config.Modules {
		total += len(source)
		if name == "" || path.Clean(name) != name || strings.HasPrefix(name, "/") || strings.Contains(name, "\\") || strings.Contains(name, "..") || !strings.HasSuffix(name, ".lua") || total > MaxModuleBytes {
			return nil, Fail(ErrSource)
		}
		if err := ValidateSource(source); err != nil {
			return nil, err
		}
		e.modules[name] = append([]byte(nil), source...)
	}
	e.life, e.cancel = context.WithCancel(context.Background())
	e.runtime = lua.New(lua.WithContext(e.life), lua.WithLimits(lua.Limits{MaxCallDepth: config.Limits.CallDepth + 16, MaxStackSlots: 32768, MaxMetaDepth: 128, MinGCInterval: -1}))
	stdlib.Open(e.runtime)
	for _, name := range []string{"io", "os", "debug", "package", "load", "loadfile", "dofile", "collectgarbage", "exec", "chan", "time", "glob", "bit32", "_lastoutput", "_outputlines"} {
		e.runtime.SetGlobal(name, lua.Nil)
	}
	for table, names := range map[string][]string{"string": {"dump"}, "math": {"random", "randomseed"}} {
		t := e.runtime.GetGlobal(table).AsTable()
		for _, name := range names {
			if err := t.Delete(lua.NewString(name)); err != nil {
				e.Close()
				return nil, Fail(ErrConfiguration)
			}
		}
	}
	e.runtime.SetGlobal("print", lua.NewNativeFunc(e.print))
	e.runtime.SetGlobal("warn", lua.NewNativeFunc(e.print))
	e.runtime.SetGlobal("require", lua.NewNativeFunc(e.require))
	e.runtime.SetGlobal("next", lua.NewNativeFunc(stableNext))
	e.runtime.SetGlobal("pairs", lua.NewNativeFunc(stablePairs))
	e.runtime.SetGlobal("tostring", lua.NewNativeFunc(func(v *lua.VM) int {
		if v.ArgCount() != 1 {
			panic(ErrScript)
		}
		x := v.Get(1)
		if !x.IsNil() && !x.IsBool() && !x.IsNumber() && !x.IsString() {
			panic(ErrValue)
		}
		v.Set(0, lua.NewString(x.String()))
		return 1
	}))
	stringsTable := e.runtime.GetGlobal("string").AsTable()
	format := stringsTable.Get(lua.NewString("format"))
	_ = stringsTable.Set(lua.NewString("format"), lua.NewNativeFunc(func(v *lua.VM) int {
		args := make([]lua.Value, v.ArgCount())
		for i := range args {
			args[i] = v.Get(i + 1)
			x := args[i]
			if !x.IsNil() && !x.IsBool() && !x.IsNumber() && !x.IsString() {
				panic(ErrValue)
			}
		}
		if len(args) == 0 || !args[0].IsString() {
			panic(ErrScript)
		}
		// Pointer formatting is never authoritative data, even for strings.
		text := args[0].AsString()
		for i := 0; i < len(text); i++ {
			if text[i] != '%' {
				continue
			}
			i++
			if i < len(text) && text[i] == '%' {
				continue
			}
			for i < len(text) && strings.ContainsRune("-+ #0.123456789", rune(text[i])) {
				i++
			}
			if i < len(text) && text[i] == 'p' {
				panic(ErrValue)
			}
		}
		values, err := v.ProtectedCall(format, args)
		if err != nil {
			panic(err)
		}
		for i, x := range values {
			v.Set(i, x)
		}
		return len(values)
	}))
	e.hook = lua.NewNativeFunc(func(v *lua.VM) int {
		active := e.active.Load()
		if active == nil {
			return 0
		}
		if active.failed.Load() || active.ctx.Err() != nil || active.count.Add(1) > e.limits.Instructions || v.StackDepth()+e.depthBase[v] > e.limits.CallDepth {
			active.failed.Store(true)
			panic(ErrBudget)
		}
		return 0
	})
	e.installHook(e.runtime)
	co := e.runtime.GetGlobal("coroutine").AsTable()
	resume := co.Get(lua.NewString("resume"))
	_ = co.Set(lua.NewString("resume"), lua.NewNativeFunc(func(v *lua.VM) int {
		args := make([]lua.Value, v.ArgCount())
		for i := range args {
			args[i] = v.Get(i + 1)
		}
		if len(args) > 0 && args[0].IsTable() {
			if child := args[0].AsTable().VMRef(); child != nil {
				e.depthBase[child] = e.depthBase[v] + v.StackDepth() + 2
			}
		}
		values, err := v.ProtectedCall(resume, args)
		if err != nil {
			panic(err)
		}
		for i, x := range values {
			v.Set(i, x)
		}
		return len(values)
	}))
	create := co.Get(lua.NewString("create"))
	if err := co.Set(lua.NewString("create"), lua.NewNativeFunc(func(v *lua.VM) int {
		if v.ArgCount() != 1 {
			panic(ErrScript)
		}
		results, err := v.ProtectedCall(create, []lua.Value{v.Get(1)})
		if err != nil {
			panic(err)
		}
		child := results[0].AsTable().VMRef()
		if child == nil {
			panic(ErrScript)
		}
		e.depthBase[child] = e.depthBase[v] + v.StackDepth()
		e.installHook(child)
		v.Set(0, results[0])
		return 1
	})); err != nil {
		e.Close()
		return nil, err
	}
	// Stock wrap creates a hidden coroutine without inheriting hooks. Build it
	// through our guarded create so every coroutine shares the invocation budget.
	proto, err := compile([]byte(`local create,resume,unpack,pack,raise,close=coroutine.create,coroutine.resume,table.unpack,table.pack,error,coroutine.close
coroutine.wrap=function(f) local c=create(f);return function(...) local r=pack(resume(c,...));if not r[1] then close(c);raise(r[2],2) end;return unpack(r,2,r.n) end end`))
	if err == nil {
		_, err = e.runtime.Run(proto)
	}
	if err != nil {
		e.Close()
		return nil, Fail(ErrConfiguration)
	}
	return e, nil
}

func (e *Engine) installHook(v *lua.VM) { v.SetHook(e.hook, lua.HookMaskCount, 1) }
func ValidateSource(source []byte) error {
	if len(source) == 0 || len(source) > MaxSourceBytes || !utf8.Valid(source) || strings.IndexByte(string(source), 0) >= 0 || source[0] == 0x1b {
		return Fail(ErrSource)
	}
	return nil
}
func compile(source []byte) (*compiler.Proto, error) {
	if err := ValidateSource(source); err != nil {
		return nil, err
	}
	block, err := parser.Parse("=package-source", string(source), false)
	if err != nil {
		return nil, Fail(ErrSource)
	}
	p, err := compiler.Compile("=package-source", block)
	if err != nil {
		return nil, Fail(ErrSource)
	}
	return p, nil
}

func (e *Engine) Execute(ctx context.Context, source []byte) (result Result, err error) {
	e.mu.Lock()
	defer e.mu.Unlock()
	e.sequence++
	result.Audit = Audit{Level: "AUDIT-0", Sequence: e.sequence, Kind: "execution", Outcome: "PASS"}
	started := false
	defer func() {
		if recover() != nil {
			err = Fail(ErrScript)
		}
		if err != nil {
			result.Values = nil
			result.Output = nil
			result.Audit.Outcome = Code(err)
			if started {
				e.poisoned = true
			}
		}
		e.active.Store(nil)
	}()
	if e.destroyed {
		return result, Fail(ErrDestroyed)
	}
	if e.poisoned {
		return result, Fail(ErrPoisoned)
	}
	if ctx == nil || ctx.Err() != nil {
		return result, Fail(ErrBudget)
	}
	ctx, cancel := context.WithTimeout(ctx, time.Duration(e.limits.WallMillis)*time.Millisecond)
	defer cancel()
	proto, err := compile(source)
	if err != nil {
		return result, err
	}
	active := &invocation{ctx: ctx}
	e.active.Store(active)
	e.output = nil
	e.outputBytes = 0
	started = true
	values, runErr := e.runtime.Run(proto)
	if active.failed.Load() || ctx.Err() != nil {
		return result, Fail(ErrBudget)
	}
	if runErr != nil {
		return result, Fail(ErrScript)
	}
	e.reconcileCheckpointShapes()
	if len(values) > checkpoint.MaxNodes {
		return result, Fail(ErrValue)
	}
	nodes, bytes := 0, 0
	for _, v := range values {
		value, convertErr := e.fromLua(v, map[lua.LuaTable]bool{}, 0, &nodes, &bytes)
		if convertErr != nil {
			return result, Fail(ErrValue)
		}
		result.Values = append(result.Values, value)
	}
	result.Output = append([]string(nil), e.output...)
	return result, nil
}

func (e *Engine) Poisoned() bool { e.mu.Lock(); defer e.mu.Unlock(); return e.poisoned }
func (e *Engine) Close() {
	e.mu.Lock()
	defer e.mu.Unlock()
	if e.destroyed {
		return
	}
	e.destroyed = true
	if e.cancel != nil {
		e.cancel()
	}
	if e.runtime != nil {
		_ = e.runtime.Close(context.Background())
	}
	e.runtime = nil
	e.modules = nil
	e.loaded = nil
	e.shapes = nil
	e.depthBase = nil
	e.output = nil
}

func (e *Engine) SetState(state, saved checkpoint.Value) error {
	if err := checkpoint.Validate(state); err != nil {
		return Fail(ErrValue)
	}
	if err := checkpoint.Validate(saved); err != nil {
		return Fail(ErrValue)
	}
	e.mu.Lock()
	defer e.mu.Unlock()
	if e.destroyed {
		return Fail(ErrDestroyed)
	}
	if e.poisoned {
		return Fail(ErrPoisoned)
	}
	e.reconcileCheckpointShapes()
	e.runtime.SetGlobal("state", e.toLua(state))
	e.runtime.SetGlobal("checkpoint", e.toLua(saved))
	return nil
}

func (e *Engine) print(v *lua.VM) int {
	parts := make([]string, 0, v.ArgCount())
	n := 1
	for i := 1; i <= v.ArgCount(); i++ {
		x := v.Get(i)
		s := x.String()
		if !x.IsNil() && !x.IsBool() && !x.IsNumber() && !x.IsString() {
			s = "<" + x.Type() + ">"
		}
		n += len(s) + 1
		if n > e.limits.OutputBytes-e.outputBytes {
			if a := e.active.Load(); a != nil {
				a.failed.Store(true)
			}
			panic(ErrBudget)
		}
		parts = append(parts, s)
	}
	e.outputBytes += n
	e.output = append(e.output, strings.Join(parts, "\t"))
	return 0
}

func (e *Engine) require(v *lua.VM) int {
	if v.ArgCount() != 1 || !v.Get(1).IsString() {
		panic(ErrSource)
	}
	name := v.Get(1).AsString()
	prefix := ""
	if namespace, module, qualified := strings.Cut(name, ":"); qualified {
		if namespace == "" || path.Clean(namespace) != namespace || strings.HasPrefix(namespace, "/") || strings.Contains(namespace, "..") || strings.ContainsAny(namespace, "\\\x00:") {
			panic(ErrSource)
		}
		prefix, name = namespace+"/", module
	}
	if name == "" || strings.ContainsAny(name, "/\\\x00:") || strings.Contains(name, "..") || strings.HasPrefix(name, ".") || strings.HasSuffix(name, ".") {
		panic(ErrSource)
	}
	file := prefix + strings.ReplaceAll(name, ".", "/") + ".lua"
	if result, ok := e.loaded[file]; ok {
		v.Set(0, result)
		return 1
	}
	source, ok := e.modules[file]
	if !ok || e.loading[file] {
		panic(ErrCapability)
	}
	e.loading[file] = true
	defer delete(e.loading, file)
	proto, err := compile(source)
	if err != nil {
		panic(err)
	}
	results, err := v.Run(proto)
	if err != nil {
		panic(err)
	}
	result := lua.NewBool(true)
	if len(results) > 0 && !results[0].IsNil() {
		result = results[0]
	}
	e.loaded[file] = result
	v.Set(0, result)
	return 1
}

func orderedKeys(table lua.LuaTable) []lua.Value {
	keys := []lua.Value{}
	key := lua.Nil
	for {
		k, _, err := table.Next(key)
		if err != nil {
			panic(ErrScript)
		}
		if k.IsNil() {
			break
		}
		if !k.IsString() && !k.IsNumber() && !k.IsBool() {
			panic(ErrValue)
		}
		keys = append(keys, k)
		if len(keys) > checkpoint.MaxNodes {
			panic(ErrBudget)
		}
		key = k
	}
	sort.Slice(keys, func(i, j int) bool {
		a, b := keys[i], keys[j]
		if a.Type() != b.Type() {
			return a.Type() < b.Type()
		}
		if a.IsNumber() {
			less, ok := a.LessThan(b)
			return ok && less
		}
		return a.String() < b.String()
	})
	return keys
}
func stableNext(v *lua.VM) int {
	if v.ArgCount() < 1 || !v.Get(1).IsTable() {
		panic(ErrScript)
	}
	table := v.Get(1).AsTable()
	previous := lua.Nil
	if v.ArgCount() > 1 {
		previous = v.Get(2)
	}
	keys := orderedKeys(table)
	index := 0
	if !previous.IsNil() {
		index = -1
		for i, k := range keys {
			if k.RawEqual(previous) {
				index = i + 1
				break
			}
		}
		if index < 0 {
			panic(ErrScript)
		}
	}
	if index == len(keys) {
		v.Set(0, lua.Nil)
		return 1
	}
	v.Set(0, keys[index])
	v.Set(1, table.Get(keys[index]))
	return 2
}
func stablePairs(v *lua.VM) int {
	if v.ArgCount() != 1 || !v.Get(1).IsTable() {
		panic(ErrScript)
	}
	table := v.Get(1)
	if mt := table.AsTable().Metatable(); mt != nil {
		if f := mt.Get(lua.NewString("__pairs")); !f.IsNil() {
			values, err := v.ProtectedCall(f, []lua.Value{table})
			if err != nil {
				panic(err)
			}
			for i, x := range values {
				v.Set(i, x)
			}
			return len(values)
		}
	}
	v.Set(0, lua.NewNativeFunc(stableNext))
	v.Set(1, table)
	v.Set(2, lua.Nil)
	return 3
}

func (e *Engine) fromLua(v lua.Value, seen map[lua.LuaTable]bool, depth int, nodes, bytes *int) (checkpoint.Value, error) {
	*nodes++
	if depth > checkpoint.MaxDepth || *nodes > checkpoint.MaxNodes {
		return checkpoint.Value{}, checkpoint.ErrRejected
	}
	var out checkpoint.Value
	switch {
	case v.IsNil():
		out = checkpoint.Value{Kind: "nil"}
	case v.IsBool():
		out = checkpoint.Bool(v.AsBool())
	case v.IsInt():
		out = checkpoint.Int(v.AsInt())
	case v.IsFloat():
		f := v.AsFloat()
		if math.IsNaN(f) || math.IsInf(f, 0) || math.Abs(f) > 1<<53 {
			return out, checkpoint.ErrRejected
		}
		out = checkpoint.Value{Kind: "float", Number: strconv.FormatFloat(f, 'g', -1, 64)}
	case v.IsString():
		out = checkpoint.Text(v.AsString())
		*bytes += len(out.String)
	case v.IsTable():
		table := v.AsTable()
		if table.IsThread() || table.Metatable() != nil || seen[table] {
			return out, checkpoint.ErrRejected
		}
		seen[table] = true
		defer delete(seen, table)
		object := map[string]checkpoint.Value{}
		array := map[int64]checkpoint.Value{}
		emptyArray := false
		if concrete, ok := table.(*lua.Table); ok && e.shapes[weak.Make(concrete)] != nil {
			shape := e.shapes[weak.Make(concrete)]
			emptyArray = shape.array
			for k := range shape.nilKeys {
				value, err := e.fromLua(lua.Nil, seen, depth+1, nodes, bytes)
				if err != nil {
					return out, err
				}
				if k.IsString() {
					*bytes += len(k.AsString())
					object[k.AsString()] = value
				} else {
					array[k.AsInt()] = value
				}
			}
		}
		key := lua.Nil
		for {
			k, x, err := table.Next(key)
			if err != nil {
				return out, err
			}
			if k.IsNil() {
				break
			}
			key = k
			value, err := e.fromLua(x, seen, depth+1, nodes, bytes)
			if err != nil {
				return out, err
			}
			if k.IsString() {
				*bytes += len(k.AsString())
				object[k.AsString()] = value
			} else if k.IsInt() && k.AsInt() > 0 && k.AsInt() <= checkpoint.MaxNodes {
				array[k.AsInt()] = value
			} else {
				return out, checkpoint.ErrRejected
			}
		}
		if len(array) > 0 || (emptyArray && len(object) == 0) {
			if len(object) > 0 {
				return out, checkpoint.ErrRejected
			}
			out = checkpoint.Value{Kind: "array", Array: make([]checkpoint.Value, len(array))}
			for i := range out.Array {
				x, ok := array[int64(i+1)]
				if !ok {
					return out, checkpoint.ErrRejected
				}
				out.Array[i] = x
			}
		} else {
			out = checkpoint.Object(object)
		}
	default:
		return out, checkpoint.ErrRejected
	}
	*bytes += 32
	if *bytes > checkpoint.MaxBytes {
		return out, checkpoint.ErrRejected
	}
	if err := checkpoint.Validate(out); err != nil {
		return out, err
	}
	return out, nil
}

func (e *Engine) toLua(v checkpoint.Value) lua.Value {
	switch v.Kind {
	case "nil":
		return lua.Nil
	case "boolean":
		return lua.NewBool(v.Boolean)
	case "integer":
		n, _ := strconv.ParseInt(v.Number, 10, 64)
		return lua.NewInt(n)
	case "float":
		n, _ := strconv.ParseFloat(v.Number, 64)
		return lua.NewFloat(n)
	case "string":
		return lua.NewString(v.String)
	case "array":
		t, shape := e.checkpointTable(true)
		for i, x := range v.Array {
			key := lua.NewInt(int64(i + 1))
			if x.Kind == "nil" {
				shape.nilKeys[key] = true
			} else {
				_ = t.Set(key, e.toLua(x)) // Validated checkpoint keys cannot fail.
			}
		}
		return lua.NewTable(t)
	case "table":
		t, shape := e.checkpointTable(false)
		for k, x := range v.Table {
			key := lua.NewString(k)
			if x.Kind == "nil" {
				shape.nilKeys[key] = true
			} else {
				_ = t.Set(key, e.toLua(x))
			}
		}
		return lua.NewTable(t)
	}
	panic(fmt.Sprintf("unvalidated checkpoint type %s", v.Kind))
}
