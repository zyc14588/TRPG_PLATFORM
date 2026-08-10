// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package profile

import (
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"io"
	"strings"
	"testing"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
)

func TestProfileIdentityFailsClosed(t *testing.T) {
	p := Production()
	if p.ID != "platform-lua-5.5-p1" || p.LanguageVersion != "5.5" || p.RuntimeVersion != RuntimeIdentity || !p.Production {
		t.Fatalf("unexpected production profile: %#v", p)
	}
	if _, err := Resolve(p.ID, p.RuntimeVersion); err != nil {
		t.Fatalf("resolve production profile: %v", err)
	}
	if _, err := Resolve("platform-lua-5.5-p999", p.RuntimeVersion); !errors.Is(err, ErrUnknownProfile) {
		t.Fatalf("unknown profile error = %v", err)
	}
	if _, err := Resolve(p.ID, "latest"); !errors.Is(err, ErrRuntimeMismatch) {
		t.Fatalf("unknown runtime error = %v", err)
	}
}

func TestRuntimeCandidateIsPinnedAndAudited(t *testing.T) {
	c := RuntimeCandidate()
	if c.Module != "github.com/iceisfun/golua/v2" || c.Version != "v2.0.5" {
		t.Fatalf("candidate is not pinned: %#v", c)
	}
	if c.Commit != "5c098c0c2a4301b7b2ee0ccaa12e6fc2ba10a2ad" || c.License != "MIT" || c.Upstream == "" || c.Reason == "" {
		t.Fatalf("candidate audit metadata incomplete: %#v", c)
	}
	if len(c.ModuleGraph) != 0 {
		t.Fatalf("zero-dependency candidate unexpectedly has a module graph: %#v", c.ModuleGraph)
	}
	if text := RuntimeLicenseText(); !strings.Contains(text, "Permission is hereby granted") || !strings.Contains(text, "Copyright 2026 github.com/iceisfun") {
		t.Fatal("embedded runtime license is incomplete")
	} else {
		digest := sha256.Sum256([]byte(text))
		if hex.EncodeToString(digest[:]) != c.LicenseSHA256 {
			t.Fatalf("embedded runtime license digest = %x, want %s", digest, c.LicenseSHA256)
		}
	}
}

func TestValidateSourceRejectsBinaryAndInvalidUTF8(t *testing.T) {
	tests := []struct {
		name string
		src  []byte
		want error
	}{
		{name: "utf8 source", src: []byte("return '世界'"), want: nil},
		{name: "puc bytecode", src: append([]byte("\x1bLua"), 0x55, 0), want: ErrBytecode},
		{name: "runtime marshalled bytecode", src: append([]byte("\x1bLua"), 0x54, 0), want: ErrBytecode},
		{name: "invalid utf8", src: []byte{0xff, 0xfe}, want: ErrInvalidUTF8},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			err := ValidateSource(tt.src)
			if !errors.Is(err, tt.want) {
				t.Fatalf("ValidateSource() error = %v, want %v", err, tt.want)
			}
		})
	}
}

func TestLua55ConformanceFeatures(t *testing.T) {
	r := NewProductionRuntime(io.Discard)
	defer func() {
		if err := r.Close(); err != nil {
			t.Errorf("close runtime: %v", err)
		}
	}()

	got, err := r.Execute("lua55", []byte(`
global answer, _VERSION, tostring
answer = 40
local function add(...args)
  return args[1] + args[2]
end
return _VERSION .. ":" .. tostring(answer + add(1, 1))
`))
	if err != nil {
		t.Fatalf("execute Lua 5.5 source: %v", err)
	}
	value, err := got.CheckpointValue()
	if err != nil || value.Type != checkpoint.TypeString || value.String != "Lua 5.5:42" {
		t.Fatalf("result = %#v err=%v, want Lua 5.5:42", value, err)
	}
}

// TestLua55ReferenceDivergences pins parser and library behavior that differs
// across nominally Lua-5.5-compatible runtimes.  These are semantic probes;
// _VERSION is deliberately not part of their evidence.
func TestLua55ReferenceDivergences(t *testing.T) {
	tests := []struct {
		name      string
		source    string
		wantError bool
	}{
		{
			name:      "math random rejects three arguments",
			source:    "return math.random(1, 2, 3)",
			wantError: true,
		},
		{
			name: "goto cannot skip global star declaration",
			source: `goto continue
global *
::continue::
return 42`,
			wantError: true,
		},
		{
			name: "same label is legal when inner declaration comes first",
			source: `if true then
  goto repeated
  ::repeated::
end
::repeated::
return 42`,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			r := NewProductionRuntime(io.Discard)
			defer r.Close()

			result, err := r.Execute(tt.name, []byte(tt.source))
			if tt.wantError {
				if err == nil {
					t.Fatal("nonconforming Lua 5.5 source succeeded")
				}
				return
			}
			if err != nil {
				t.Fatalf("conforming Lua 5.5 source failed: %v", err)
			}
			value, err := result.CheckpointValue()
			if err != nil || value.Type != checkpoint.TypeInt || value.Integer != 42 {
				t.Fatalf("result = %#v err=%v, want integer 42", value, err)
			}
		})
	}
}

func TestLua55BoundedPlatformConformanceCorpus(t *testing.T) {
	tests := []struct {
		name   string
		source string
	}{
		{
			name:   "integer and float distinction",
			source: `return math.type(7) == "integer" and math.type(7.0) == "float"`,
		},
		{
			name:   "arithmetic",
			source: `return 6 + 4 == 10 and 6 - 4 == 2 and 6 * 4 == 24 and 8 / 4 == 2.0 and 2 ^ 5 == 32`,
		},
		{
			name:   "floor division",
			source: `return -7 // 3 == -3 and 7 // -3 == -3`,
		},
		{
			name:   "negative modulo",
			source: `return -7 % 3 == 2 and 7 % -3 == -2`,
		},
		{
			name: "closure and mutable upvalue",
			source: `local function counter()
  local n = 40
  return function(step) n = n + step; return n end
end
local next = counter()
return next(1) == 41 and next(1) == 42`,
		},
		{
			name: "coroutine create resume and status",
			source: `local co = coroutine.create(function(value)
  local resumed = coroutine.yield(value + 1)
  return resumed + 2
end)
if coroutine.status(co) ~= "suspended" then return false end
local ok1, first = coroutine.resume(co, 41)
if not ok1 or first ~= 42 or coroutine.status(co) ~= "suspended" then return false end
local ok2, second = coroutine.resume(co, 40)
return ok2 and second == 42 and coroutine.status(co) == "dead"`,
		},
		{
			name: "scoped goto",
			source: `local total = 0
for i = 1, 4 do
  if i == 2 then goto continue end
  total = total + i
  ::continue::
end
return total == 8`,
		},
		{
			name: "global declaration",
			source: `global bounded_corpus_global
bounded_corpus_global = 42
return bounded_corpus_global == 42`,
		},
		{
			name: "table create",
			source: `local t = table.create(3, 1)
t[1], t[2], t[3], t.answer = 10, 20, 12, 42
return #t == 3 and t[1] + t[2] + t[3] == t.answer`,
		},
		{
			name:   "string library",
			source: `local value, count = string.gsub("lua 5.5", "lua", "Lua"); return value == "Lua 5.5" and count == 1`,
		},
		{
			name:   "utf8 library",
			source: `local s = "A世🙂"; return utf8.len(s) == 3 and utf8.codepoint(s, 2) == 0x4e16`,
		},
		{
			name: "table iteration",
			source: `local sum, count = 0, 0
for _, value in pairs({a = 10, b = 20, c = 12}) do sum, count = sum + value, count + 1 end
local ordered = 0
for index, value in ipairs({3, 4, 5}) do ordered = ordered + index * value end
return sum == 42 and count == 3 and ordered == 26`,
		},
		{
			name: "allowed standard library surface",
			source: `return type(assert) == "function"
  and type(pcall) == "function"
  and type(string) == "table"
  and type(math) == "table"
  and type(table) == "table"
  and type(coroutine) == "table"
  and type(utf8) == "table"`,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			r := NewProductionRuntime(io.Discard)
			defer r.Close()
			result, err := r.Execute(tt.name, []byte(tt.source))
			if err != nil {
				t.Fatalf("Lua 5.5 conformance source failed: %v", err)
			}
			value, err := result.CheckpointValue()
			if err != nil || value.Type != checkpoint.TypeBool || !value.Boolean {
				t.Fatalf("result = %#v err=%v, want true", value, err)
			}
		})
	}
}

func TestLua55ProfileConformanceNegatives(t *testing.T) {
	tests := []struct {
		name   string
		source string
		wantOK bool
	}{
		{
			name: "table create",
			source: `local t = table.create(2, 1)
t[1], t[2] = 40, 2
return t[1] + t[2]`,
			wantOK: true,
		},
		{
			name:   "const global is read only",
			source: "global<const> locked = 1\nlocked = 2\nreturn locked",
		},
		{
			name:   "numeric loop variable is read only",
			source: "for i = 1, 1 do i = 2 end\nreturn true",
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			r := NewProductionRuntime(io.Discard)
			defer r.Close()
			result, err := r.Execute(tt.name, []byte(tt.source))
			if tt.wantOK {
				if err != nil {
					t.Fatalf("Execute error = %v", err)
				}
				value, err := result.CheckpointValue()
				if err != nil || value.Type != checkpoint.TypeInt || value.Integer != 42 {
					t.Fatalf("result = %#v err=%v", value, err)
				}
			} else if err == nil {
				t.Fatal("nonconforming Lua 5.5 source succeeded")
			}
		})
	}
}

func TestProductionDangerousSurfacesAreDenied(t *testing.T) {
	r := NewProductionRuntime(io.Discard)
	defer r.Close()

	got, err := r.Execute("surface", []byte(`
local denied = {
  io, os, debug, package, require, load, loadfile, dofile, golib, runtime,
  chan, time, exec, http, bit32, glob, _lastoutput, _outputlines,
  string.dump, math.frexp, math.ldexp, HOME, AWS_SECRET_ACCESS_KEY,
}
for i = 1, 23 do
  if denied[i] ~= nil then return false end
end
return true
`))
	if err != nil {
		t.Fatalf("execute denial probe: %v", err)
	}
	value, err := got.CheckpointValue()
	if err != nil || value.Type != checkpoint.TypeBool || !value.Boolean {
		t.Fatalf("dangerous surface was reachable: value=%#v err=%v", value, err)
	}
}

func TestProductionCompileRejectsEveryNonSourceInput(t *testing.T) {
	r := NewProductionRuntime(io.Discard)
	defer r.Close()
	for _, src := range [][]byte{{0xff}, {0x1b, 'L', 'u', 'a'}, {0x1b, 'L', 'u', 'a', 0x54}} {
		if _, err := r.Execute("binary", src); err == nil {
			t.Fatalf("Execute(%x) succeeded", src)
		}
	}
}
