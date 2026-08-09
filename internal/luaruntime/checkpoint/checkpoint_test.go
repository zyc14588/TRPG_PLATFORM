// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package checkpoint

import (
	"bytes"
	"errors"
	"math"
	"testing"

	rt "github.com/arnodel/golua/runtime"
)

const testHash = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"

func testBinding() Binding {
	return Binding{
		SessionID:      "session-a",
		StateVersion:   7,
		PackageHashes:  []PackageHash{{PackageID: "pkg.rules", SHA256: testHash}},
		DependencyLock: `{"packages":[{"id":"pkg.rules","version":"1.0.0"}]}`,
		LuaProfile:     "platform-lua-5.5-p1",
		RuntimeVersion: "github.com/arnodel/golua@v0.2.0",
	}
}

func TestCheckpointRoundTripIsDeterministic(t *testing.T) {
	state := map[string]any{
		"turn":  int64(3),
		"flags": []any{true, false, nil},
		"cache": map[string]any{"name": "世界", "ratio": 1.25},
	}
	first, err := Marshal(testBinding(), state)
	if err != nil {
		t.Fatalf("Marshal first: %v", err)
	}
	second, err := Marshal(testBinding(), state)
	if err != nil {
		t.Fatalf("Marshal second: %v", err)
	}
	if !bytes.Equal(first, second) {
		t.Fatal("canonical checkpoint bytes differ")
	}
	decoded, err := Unmarshal(first, testBinding())
	if err != nil {
		t.Fatalf("Unmarshal: %v", err)
	}
	if decoded.State.Type != TypeTable || len(decoded.State.Table) != 3 {
		t.Fatalf("decoded state = %#v", decoded.State)
	}
}

func TestCheckpointRejectsEveryBindingMismatch(t *testing.T) {
	encoded, err := Marshal(testBinding(), map[string]any{"turn": 3})
	if err != nil {
		t.Fatal(err)
	}
	tests := []struct {
		name   string
		mutate func(*Binding)
	}{
		{name: "session", mutate: func(b *Binding) { b.SessionID = "session-b" }},
		{name: "state version", mutate: func(b *Binding) { b.StateVersion++ }},
		{name: "package hash", mutate: func(b *Binding) {
			b.PackageHashes[0].SHA256 = "1123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
		}},
		{name: "dependency lock", mutate: func(b *Binding) { b.DependencyLock += " " }},
		{name: "profile", mutate: func(b *Binding) { b.LuaProfile = "platform-lua-5.5-p2" }},
		{name: "runtime", mutate: func(b *Binding) { b.RuntimeVersion = "latest" }},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			expected := testBinding()
			expected.PackageHashes = append([]PackageHash(nil), expected.PackageHashes...)
			tt.mutate(&expected)
			_, err := Unmarshal(encoded, expected)
			if !errors.Is(err, ErrIncompatibleBinding) && !errors.Is(err, ErrInvalidBinding) {
				t.Fatalf("Unmarshal error = %v", err)
			}
		})
	}
}

func TestCheckpointUnknownProfileAndRuntimeFailClosed(t *testing.T) {
	for _, mutate := range []func(*Binding){
		func(binding *Binding) { binding.LuaProfile = "platform-lua-5.5-latest" },
		func(binding *Binding) { binding.RuntimeVersion = "latest" },
	} {
		candidate := testBinding()
		mutate(&candidate)
		if _, err := Marshal(candidate, map[string]any{"turn": 1}); !errors.Is(err, ErrInvalidBinding) {
			t.Fatalf("unknown identity Marshal error = %v", err)
		}
	}
}

func TestCheckpointRejectsCyclesFunctionsAndInvalidNumbers(t *testing.T) {
	cycle := map[string]any{}
	cycle["self"] = cycle
	tests := []struct {
		name  string
		value any
		want  error
	}{
		{name: "cycle", value: cycle, want: ErrCycle},
		{name: "function", value: func() {}, want: ErrUnsupportedValue},
		{name: "nan", value: math.NaN(), want: ErrInvalidValue},
		{name: "infinity", value: math.Inf(1), want: ErrInvalidValue},
		{name: "native handle", value: struct{ Token string }{Token: "opaque"}, want: ErrUnsupportedValue},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if _, err := Marshal(testBinding(), tt.value); !errors.Is(err, tt.want) {
				t.Fatalf("Marshal error = %v, want %v", err, tt.want)
			}
		})
	}
}

func TestLuaValueConversionRejectsOpaqueState(t *testing.T) {
	runtime := rt.New(nil)
	defer runtime.Close(nil)
	function, err := runtime.CompileAndLoadLuaChunk("fn", []byte("return 1"), rt.TableValue(runtime.GlobalEnv()))
	if err != nil {
		t.Fatal(err)
	}
	tableWithMeta := rt.NewTable()
	tableWithMeta.SetMetatable(rt.NewTable())
	tests := []struct {
		name  string
		value rt.Value
		want  error
	}{
		{name: "function", value: rt.FunctionValue(function), want: ErrUnsupportedValue},
		{name: "coroutine", value: rt.ThreadValue(rt.NewThread(runtime)), want: ErrUnsupportedValue},
		{name: "userdata", value: runtime.NewUserDataValue("native", nil), want: ErrUnsupportedValue},
		{name: "metatable", value: rt.TableValue(tableWithMeta), want: ErrMetatable},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if _, err := FromLua(tt.value); !errors.Is(err, tt.want) {
				t.Fatalf("FromLua error = %v, want %v", err, tt.want)
			}
		})
	}
}

func TestLuaValueConversionRejectsCycle(t *testing.T) {
	table := rt.NewTable()
	table.Set(rt.StringValue("self"), rt.TableValue(table))
	if _, err := FromLua(rt.TableValue(table)); !errors.Is(err, ErrCycle) {
		t.Fatalf("FromLua cycle error = %v", err)
	}
}

func TestCheckpointRejectsCorruptionAndOversize(t *testing.T) {
	encoded, err := Marshal(testBinding(), map[string]any{"turn": 3})
	if err != nil {
		t.Fatal(err)
	}
	corrupted := append([]byte(nil), encoded...)
	corrupted[len(corrupted)/2] ^= 1
	if _, err := Unmarshal(corrupted, testBinding()); !errors.Is(err, ErrCorrupt) {
		t.Fatalf("corruption error = %v", err)
	}
	noncanonical := append(append([]byte(nil), encoded...), '\n')
	if _, err := Unmarshal(noncanonical, testBinding()); !errors.Is(err, ErrCorrupt) {
		t.Fatalf("noncanonical JSON error = %v", err)
	}
	oversized := bytes.Repeat([]byte{'x'}, MaxDocumentSize+1)
	if _, err := Unmarshal(oversized, testBinding()); !errors.Is(err, ErrOversized) {
		t.Fatalf("oversize error = %v", err)
	}
}
