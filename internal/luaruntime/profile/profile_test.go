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

func TestStringSHA256VectorsAndByteBoundaries(t *testing.T) {
	cases := []struct {
		name, source, digest string
	}{
		{"empty", `return string.sha256("")`, "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"},
		{"abc", `return string.sha256("abc")`, "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"},
		{"FIPS56", `return string.sha256("abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq")`, "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"},
		{"binary-NUL", `return string.sha256(string.char(0, 1, 2, 255, 0))`, "79eebf0b2aae05b2490234d0b884581935b2e28127f06a74a5b48e758f86bd25"},
		{"unicode-UTF8", `return string.sha256("雪😀")`, "c7d6344b3b16cb73d55e41f1bf3ec5e71eafcf3296b3eb41e8f9e776e7d5bd45"},
		{"exact65536", `return string.sha256(string.rep("a", 65536))`, "bf718b6f653bebc184e1479f1935b8da974d701b893afcf49e701f3e2f9f9c5a"},
		{"unicode-exact65536-bytes", `return string.sha256(string.rep("雪", 21845) .. "a")`, "6992b6498f8beffe494b5b5082c7819ee5664df4298c377bf3d50d780df1d4dd"},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			e := engine(t)
			for attempt := 0; attempt < 2; attempt++ {
				r, err := e.Execute(context.Background(), []byte(tc.source))
				if err != nil || len(r.Values) != 1 || r.Values[0].Kind != "string" || r.Values[0].String != tc.digest {
					t.Fatalf("digest mismatch: %#v %v", r, err)
				}
				if len(r.Values[0].String) != 64 || strings.Trim(r.Values[0].String, "0123456789abcdef") != "" {
					t.Fatal("digest must contain exactly64 lowercase hex characters")
				}
			}
		})
	}
}

func TestStringSHA256StrictArgumentsAndNonDisclosingErrors(t *testing.T) {
	cases := []struct {
		name, source, code string
	}{
		{"missing", `return string.sha256()`, ErrScript},
		{"extra", `return string.sha256("input-secret", "extra")`, ErrScript},
		{"nil", `return string.sha256(nil)`, ErrValue},
		{"boolean", `return string.sha256(true)`, ErrValue},
		{"integer", `return string.sha256(7)`, ErrValue},
		{"float", `return string.sha256(1.5)`, ErrValue},
		{"table", `return string.sha256({secret="input-secret"})`, ErrValue},
		{"function", `return string.sha256(function() return "input-secret" end)`, ErrValue},
		{"coroutine", `return string.sha256(coroutine.create(function() end))`, ErrValue},
		{"65537-bytes", `return string.sha256(string.rep("a", 65537))`, ErrValue},
		{"unicode-over-byte-bound", `return string.sha256(string.rep("雪", 21846))`, ErrValue},
		{"over-bound-secret", `return string.sha256(string.rep("input-secret", 6554))`, ErrValue},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			r, err := engine(t).Execute(context.Background(), []byte(tc.source))
			if Code(err) != tc.code || err == nil || err.Error() != tc.code || r.Audit.Outcome != tc.code {
				t.Fatalf("expected fixed error %s, got %#v %v", tc.code, r, err)
			}
			if len(r.Output) != 0 || strings.Contains(err.Error(), "input-secret") {
				t.Fatal("hash argument was disclosed")
			}
		})
	}
}

func TestStringSHA256RequiresNoHostAuthority(t *testing.T) {
	r, err := engine(t).Execute(context.Background(), []byte(`
		assert(host == nil and io == nil and os == nil and debug == nil and package == nil)
		return string.sha256("abc")
	`))
	if err != nil || len(r.Values) != 1 || r.Values[0].String != "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad" {
		t.Fatalf("pure hash changed sandbox authority: %#v %v", r, err)
	}
}
