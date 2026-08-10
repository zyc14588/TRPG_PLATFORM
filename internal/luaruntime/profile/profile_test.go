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
	if c.Module != "github.com/arnodel/golua" || c.Version != "v0.2.0" {
		t.Fatalf("candidate is not pinned: %#v", c)
	}
	if len(c.Commit) != 40 || c.License != "Apache-2.0" || c.Upstream == "" || c.Reason == "" {
		t.Fatalf("candidate audit metadata incomplete: %#v", c)
	}
	if len(c.ModuleGraph) != 1 || c.ModuleGraph[0].Module != "github.com/arnodel/strftime" || c.ModuleGraph[0].Version != "v0.1.6" || c.ModuleGraph[0].License != "MIT" || c.ModuleGraph[0].Linked {
		t.Fatalf("candidate module graph metadata incomplete: %#v", c.ModuleGraph)
	}
	if text := RuntimeLicenseText(); !strings.Contains(text, "Apache License") || !strings.Contains(text, "Copyright 2017-2021 Arnaud Delobelle") {
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
		{name: "puc bytecode", src: append([]byte("\x1bLua"), 0, 1), want: ErrBytecode},
		{name: "golua bytecode", src: []byte{6, 0, 4, 1}, want: ErrBytecode},
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
  string.dump, HOME, AWS_SECRET_ACCESS_KEY,
}
for i = 1, 13 do
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
	for _, src := range [][]byte{{0xff}, {0x1b, 'L', 'u', 'a'}, {6, 0, 4}} {
		if _, err := r.Execute("binary", src); err == nil {
			t.Fatalf("Execute(%x) succeeded", src)
		}
	}
}
