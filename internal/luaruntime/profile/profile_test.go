// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package profile

import (
	"context"
	"strings"
	"testing"
	"time"
)

func engine(t *testing.T) *Engine {
	t.Helper()
	e, err := New(Config{Limits: DefaultLimits()})
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { e.Close() })
	return e
}

func TestLua55Conformance(t *testing.T) {
	cases := map[string]string{
		"global":          "global x; x=7; return x",
		"named_varargs":   "local function sum(... args) return args[1]+args[2] end; return sum(3,4)",
		"prefix_const":    "local<const> n=7; return n",
		"table_create":    "local t=table.create(3,2); t[1]=7; return t[1]",
		"integer_bitwise": "return (3 << 1) | 1",
		"closure":         "local n=3; local function f(x) return n+x end; return f(4)",
		"close":           "local n=0; do local x <close> = setmetatable({}, {__close=function() n=7 end}) end; return n",
		"coroutine":       "local c=coroutine.create(function() coroutine.yield(3); return 7 end); assert(select(2,coroutine.resume(c))==3); return select(2,coroutine.resume(c))",
	}
	for name, source := range cases {
		t.Run(name, func(t *testing.T) {
			e := engine(t)
			r, err := e.Execute(context.Background(), []byte(source))
			if err != nil || len(r.Values) != 1 || r.Values[0].Number != "7" {
				t.Fatalf("%#v %v", r, err)
			}
		})
	}
	for _, source := range []string{"local<const> n=7; n=8", "for i=1,2 do i=4 end", "global x; return undeclared"} {
		if _, err := engine(t).Execute(context.Background(), []byte(source)); err == nil {
			t.Fatalf("accepted invalid Lua 5.5: %s", source)
		}
	}
}

func TestSourceOnlyAndProductionDenial(t *testing.T) {
	for _, s := range [][]byte{{0x1b, 'L', 'u', 'a'}, {255}, []byte("return 1\x00")} {
		if _, err := engine(t).Execute(context.Background(), s); err == nil {
			t.Fatal("invalid source accepted")
		}
	}
	e := engine(t)
	source := `assert(_VERSION=="Lua 5.5"); assert(io==nil and os==nil and debug==nil and package==nil and load==nil and loadfile==nil and dofile==nil); assert(exec==nil and chan==nil and time==nil and glob==nil and bit32==nil); assert(string.dump==nil and math.random==nil and math.randomseed==nil and collectgarbage==nil); return true`
	if _, err := e.Execute(context.Background(), []byte(source)); err != nil {
		t.Fatal(err)
	}
	for _, s := range []string{`return require("os")`, `return require("../secret")`, `return io.open("/etc/passwd")`, `return os.getenv("TOKEN")`, `return debug.getregistry()`, `return package.loadlib("x","x")`} {
		if _, err := engine(t).Execute(context.Background(), []byte(s)); err == nil {
			t.Fatalf("dangerous operation succeeded: %s", s)
		}
	}
}

func TestPostExecutionConversionPoisonsACC008(t *testing.T) {
	for _, value := range []string{`function() end`, `coroutine.create(function() end)`, `setmetatable({}, {})`, `0/0`, `string.char(255)`, `"cap:forged"`} {
		t.Run(value, func(t *testing.T) {
			e := engine(t)
			if _, err := e.Execute(context.Background(), []byte(`partial=41; return `+value)); err == nil {
				t.Fatal("invalid return accepted")
			}
			if _, err := e.Execute(context.Background(), []byte(`return partial+1`)); Code(err) != ErrPoisoned {
				t.Fatalf("008: continuation allowed: %v", err)
			}
		})
	}
	e := engine(t)
	if _, err := e.Execute(context.Background(), []byte(`partial=41; local x={};x.x=x;return x`)); err == nil {
		t.Fatal("cycle accepted")
	}
	if !e.Poisoned() {
		t.Fatal("cycle error did not poison VM")
	}
}

func TestExecutionBudgetsAndCancellation(t *testing.T) {
	for name, source := range map[string]string{"instructions": `while true do end`, "recursion": `local function f() return 1+f() end;return f()`, "output": `print(string.rep("x",65537))`, "caught_output": `pcall(function() print(string.rep("x",65537)) end); return 1`} {
		t.Run(name, func(t *testing.T) {
			e := engine(t)
			ctx, cancel := context.WithTimeout(context.Background(), time.Second)
			defer cancel()
			if _, err := e.Execute(ctx, []byte(source)); err == nil {
				t.Fatal("budget bypass")
			}
			if !e.Poisoned() {
				t.Fatal("budget failure did not poison")
			}
		})
	}
	ctx, cancel := context.WithCancel(context.Background())
	cancel()
	if _, err := engine(t).Execute(ctx, []byte(`return 7`)); err == nil {
		t.Fatal("cancellation ignored")
	}
	if _, err := engine(t).Execute(context.Background(), []byte(strings.Repeat(" ", MaxSourceBytes+1))); err == nil {
		t.Fatal("source limit ignored")
	}
}

func TestCaughtOutputBudgetIsSticky(t *testing.T) {
	e := engine(t)
	r, err := e.Execute(context.Background(), []byte(`return pcall(function() print(string.rep("x",65537)) end)`))
	if Code(err) != ErrBudget || r.Audit.Outcome != ErrBudget || !e.Poisoned() {
		t.Fatalf("caught output failure escaped budget: %#v %v", r, err)
	}
}

func TestCoroutineBudgetAndDeterministicModules(t *testing.T) {
	for _, source := range []string{
		`local c=coroutine.create(function() while true do end end);coroutine.resume(c);return 1`,
		`local f=coroutine.wrap(function() while true do end end);pcall(f);return 1`,
		`local function f() local c=coroutine.create(f);coroutine.resume(c) end;f();return 1`,
	} {
		e := engine(t)
		r, err := e.Execute(context.Background(), []byte(source))
		if err == nil || !e.Poisoned() || r.Audit.Outcome == "PASS" {
			t.Fatalf("coroutine budget escaped (%s): %#v %v", source, r, err)
		}
	}
	e, err := New(Config{Limits: DefaultLimits(), Modules: map[string][]byte{"example.test/lib/helper.lua": []byte(`return {count=0}`)}})
	if err != nil {
		t.Fatal(err)
	}
	defer e.Close()
	for range 2 {
		r, err := e.Execute(context.Background(), []byte(`local m=require("example.test/lib:helper");m.count=m.count+1;local keys={};for k in pairs({z=1,a=2,m=3}) do keys[#keys+1]=k end;return table.concat(keys),m.count`))
		if err != nil || r.Values[0].String != "amz" {
			t.Fatalf("unstable traversal: %#v %v", r, err)
		}
	}
}

func TestOpaqueIdentitiesCannotEnterDeterministicResults(t *testing.T) {
	for _, source := range []string{`return tostring({})`, `return string.format("%p", {})`, `return string.format("%p", "value")`, `return string.format("%s", function() end)`} {
		if _, err := engine(t).Execute(context.Background(), []byte(source)); err == nil {
			t.Fatal("process address rendered", source)
		}
	}
	r, err := engine(t).Execute(context.Background(), []byte(`return string.format("%d:%s",7,"value"),tostring(42),string.format("%%p")`))
	if err != nil || r.Values[0].String != "7:value" || r.Values[1].String != "42" || r.Values[2].String != "%p" {
		t.Fatalf("primitive formatting failed: %#v %v", r, err)
	}
}
