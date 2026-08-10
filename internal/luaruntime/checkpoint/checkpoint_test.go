// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package checkpoint

import (
	"bytes"
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"math"
	"reflect"
	"strings"
	"testing"

	"github.com/iceisfun/golua/v2/compiler"
	"github.com/iceisfun/golua/v2/parser"
	"github.com/iceisfun/golua/v2/stdlib"
	luavm "github.com/iceisfun/golua/v2/vm"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile/identity"
)

const testHash = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"

const testCanonicalLock = `{"schema_version":1,"root":"publisher/stable-name","packages":[{"package_id":"publisher/stable-name","version":"1.0.0","content_hash":"sha256:` + testHash + `","features":[],"dependencies":[]}]}`

func testBinding() Binding {
	return Binding{
		SessionID:      "session-a",
		StateVersion:   7,
		PackageHashes:  []PackageHash{{PackageID: "publisher/stable-name", SHA256: testHash}},
		DependencyLock: testCanonicalLock,
		LuaProfile:     identity.ProductionID,
		RuntimeVersion: identity.RuntimeIdentity,
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
		{name: "dependency lock", mutate: func(b *Binding) {
			b.DependencyLock = strings.Replace(b.DependencyLock, `"version":"1.0.0"`, `"version":"1.0.1"`, 1)
		}},
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

func executeLuaValue(t *testing.T, source string) luavm.Value {
	t.Helper()
	runtime := luavm.New()
	stdlib.Open(runtime)
	t.Cleanup(func() {
		if err := runtime.Close(context.Background()); err != nil {
			t.Errorf("close Lua runtime: %v", err)
		}
	})
	block, err := parser.Parse("checkpoint-test", source, false)
	if err != nil {
		t.Fatalf("parse Lua source: %v", err)
	}
	proto, err := compiler.Compile("checkpoint-test", block)
	if err != nil {
		t.Fatalf("compile Lua source: %v", err)
	}
	values, err := runtime.Run(proto)
	if err != nil {
		t.Fatalf("run Lua source: %v", err)
	}
	if len(values) != 1 {
		t.Fatalf("Lua result count = %d, want 1", len(values))
	}
	return values[0]
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

func TestBindingUsesCanonicalPackageIDs(t *testing.T) {
	valid := testBinding()
	valid.PackageHashes[0].PackageID = "publisher/stable-name"
	if _, err := NewBinding(valid); err != nil {
		t.Fatalf("canonical package ID rejected: %v", err)
	}

	invalid := []string{
		"pkg.rules",
		"malformed",
		"UPPER/pkg",
		"publisher/UPPER",
		" publisher/name",
		"publisher/name ",
		"publisher//name",
		"publisher/",
		" /name",
		"publisher/.name",
		"publisher/name.",
		strings.Repeat("a", 128) + "/" + strings.Repeat("b", 128),
	}
	for _, packageID := range invalid {
		t.Run(packageID, func(t *testing.T) {
			binding := testBinding()
			binding.PackageHashes[0].PackageID = packageID
			if _, err := NewBinding(binding); !errors.Is(err, ErrInvalidBinding) {
				t.Fatalf("NewBinding(%q) error = %v, want ErrInvalidBinding", packageID, err)
			}
		})
	}
}

func TestBindingPackageHashesAreUniqueCanonicalAndSorted(t *testing.T) {
	binding := testBinding()
	binding.PackageHashes = []PackageHash{
		{PackageID: "publisher/stable-name", SHA256: testHash},
		{PackageID: "another/dependency", SHA256: strings.Repeat("a", 64)},
	}
	canonical, err := NewBinding(binding)
	if err != nil {
		t.Fatal(err)
	}
	if canonical.PackageHashes[0].PackageID != "another/dependency" || canonical.PackageHashes[1].PackageID != "publisher/stable-name" {
		t.Fatalf("package hashes are not sorted: %#v", canonical.PackageHashes)
	}

	for _, mutate := range []func(*Binding){
		func(value *Binding) { value.PackageHashes[0].SHA256 = strings.ToUpper(testHash) },
		func(value *Binding) { value.PackageHashes[0].SHA256 = testHash[:63] + "g" },
		func(value *Binding) { value.PackageHashes = append(value.PackageHashes, value.PackageHashes[0]) },
	} {
		candidate := testBinding()
		mutate(&candidate)
		if _, err := NewBinding(candidate); !errors.Is(err, ErrInvalidBinding) {
			t.Fatalf("invalid package hash binding error = %v", err)
		}
	}
}

func lockNode(packageID, version, hash, features, dependencies string) string {
	return fmt.Sprintf(`{"package_id":%q,"version":%q,"content_hash":%q,"features":%s,"dependencies":%s}`,
		packageID, version, hash, features, dependencies)
}

func exactLock(root string, nodes ...string) string {
	return fmt.Sprintf(`{"schema_version":1,"root":%q,"packages":[%s]}`, root, strings.Join(nodes, ","))
}

func TestBindingRejectsInvalidExactLocks(t *testing.T) {
	hash := "sha256:" + testHash
	root := lockNode("publisher/stable-name", "1.0.0", hash, `[]`, `[]`)
	rootWithMissing := lockNode("publisher/stable-name", "1.0.0", hash, `[]`, `["publisher/missing"]`)
	extra := lockNode("publisher/extra", "1.0.0", "sha256:"+strings.Repeat("a", 64), `[]`, `[]`)
	cycle := lockNode("publisher/stable-name", "1.0.0", hash, `[]`, `["publisher/stable-name"]`)
	tests := []struct {
		name string
		lock string
	}{
		{name: "not a lock", lock: "not-a-lock"},
		{name: "empty object", lock: `{}`},
		{name: "JSON string", lock: `""`},
		{name: "whitespace", lock: " \t\n"},
		{name: "malformed JSON", lock: `{"schema_version":1,"root":`},
		{name: "invalid package ID", lock: exactLock("UPPER/pkg", lockNode("UPPER/pkg", "1.0.0", hash, `[]`, `[]`))},
		{name: "invalid semver", lock: exactLock("publisher/stable-name", lockNode("publisher/stable-name", "01.0.0", hash, `[]`, `[]`))},
		{name: "invalid hash", lock: exactLock("publisher/stable-name", lockNode("publisher/stable-name", "1.0.0", "sha256:short", `[]`, `[]`))},
		{name: "noncanonical hash", lock: exactLock("publisher/stable-name", lockNode("publisher/stable-name", "1.0.0", "sha256:"+strings.ToUpper(testHash), `[]`, `[]`))},
		{name: "duplicate packages", lock: exactLock("publisher/stable-name", root, root)},
		{name: "missing graph node", lock: exactLock("publisher/stable-name", rootWithMissing)},
		{name: "unreachable graph", lock: exactLock("publisher/stable-name", root, extra)},
		{name: "cycle", lock: exactLock("publisher/stable-name", cycle)},
		{name: "invalid feature", lock: exactLock("publisher/stable-name", lockNode("publisher/stable-name", "1.0.0", hash, `["Bad"]`, `[]`))},
		{name: "trailing JSON", lock: testCanonicalLock + `{}`},
		{name: "unknown field", lock: strings.TrimSuffix(testCanonicalLock, `}`) + `,"unknown":true}`},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			binding := testBinding()
			binding.DependencyLock = tt.lock
			if _, err := NewBinding(binding); !errors.Is(err, ErrInvalidBinding) {
				t.Fatalf("NewBinding invalid lock error = %v", err)
			}
		})
	}
}

func TestBindingCanonicalizesExactLockOnInputAndRequiresCanonicalStorage(t *testing.T) {
	binding := testBinding()
	binding.DependencyLock = " \n" + testCanonicalLock + "\t"
	canonical, err := NewBinding(binding)
	if err != nil {
		t.Fatalf("NewBinding semantic lock: %v", err)
	}
	if canonical.DependencyLock != testCanonicalLock {
		t.Fatalf("canonical lock = %q", canonical.DependencyLock)
	}
	encoded, err := Marshal(binding, map[string]any{"turn": 1})
	if err != nil {
		t.Fatalf("Marshal semantic lock: %v", err)
	}
	decoded, err := Unmarshal(encoded, testBinding())
	if err != nil {
		t.Fatalf("Unmarshal canonical lock: %v", err)
	}
	if decoded.Binding.DependencyLock != testCanonicalLock {
		t.Fatalf("stored lock = %q", decoded.Binding.DependencyLock)
	}

	noncanonical := testBinding()
	noncanonical.DependencyLock = " " + testCanonicalLock
	p := payload{FormatVersion: FormatVersion, Binding: noncanonical, State: Int(1)}
	payloadBytes, err := json.Marshal(p)
	if err != nil {
		t.Fatal(err)
	}
	digest := sha256.Sum256(payloadBytes)
	documentBytes, err := json.Marshal(document{Payload: p, PayloadSHA256: hex.EncodeToString(digest[:])})
	if err != nil {
		t.Fatal(err)
	}
	if _, err := Unmarshal(documentBytes, testBinding()); !errors.Is(err, ErrCorrupt) {
		t.Fatalf("noncanonical stored binding error = %v, want ErrCorrupt", err)
	}
}

func TestEmptyStringKeyCanonicalRoundTripAcrossAllInputs(t *testing.T) {
	want := Table(Entry("", Int(1)))
	tests := []struct {
		name  string
		value any
	}{
		{name: "Go map", value: map[string]any{"": 1}},
		{name: "checkpoint value", value: want},
		{name: "Lua table", value: executeLuaValue(t, `return {[""] = 1}`)},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			value, err := FromGo(tt.value)
			if err != nil {
				t.Fatalf("FromGo: %v", err)
			}
			if !reflect.DeepEqual(value, want) {
				t.Fatalf("checkpoint value = %#v, want %#v", value, want)
			}
			encoded, err := Marshal(testBinding(), value)
			if err != nil {
				t.Fatalf("Marshal: %v", err)
			}
			decoded, err := Unmarshal(encoded, testBinding())
			if err != nil {
				t.Fatalf("Unmarshal: %v", err)
			}
			if !reflect.DeepEqual(decoded.State, want) {
				t.Fatalf("round-trip state = %#v, want %#v", decoded.State, want)
			}
		})
	}
}

func TestEmptyStringKeyPreservesAllOtherKeyConstraints(t *testing.T) {
	normalized, err := Normalize(Table(Entry("z", Int(3)), Entry("", Int(1)), Entry("a", Int(2))))
	if err != nil {
		t.Fatal(err)
	}
	if got := []string{normalized.Table[0].Key, normalized.Table[1].Key, normalized.Table[2].Key}; !reflect.DeepEqual(got, []string{"", "a", "z"}) {
		t.Fatalf("canonical key order = %#v", got)
	}
	tests := []struct {
		name  string
		value Value
		want  error
	}{
		{name: "invalid UTF-8", value: Table(Entry(string([]byte{0xff}), Int(1))), want: ErrInvalidValue},
		{name: "overlength", value: Table(Entry(strings.Repeat("x", MaxStringBytes+1), Int(1))), want: ErrLimit},
		{name: "duplicate empty key", value: Table(Entry("", Int(1)), Entry("", Int(2))), want: ErrInvalidValue},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if _, err := Normalize(tt.value); !errors.Is(err, tt.want) {
				t.Fatalf("checkpoint key error = %v, want %v", err, tt.want)
			}
		})
	}
}

func TestCheckpointValueLimitBoundaries(t *testing.T) {
	if _, err := Normalize(String(strings.Repeat("x", MaxStringBytes))); err != nil {
		t.Fatalf("maximum-length string rejected: %v", err)
	}
	if _, err := Normalize(String(strings.Repeat("x", MaxStringBytes+1))); !errors.Is(err, ErrLimit) {
		t.Fatalf("overlength string error = %v", err)
	}

	atDepth := Int(1)
	for range MaxDepth {
		atDepth = Array(atDepth)
	}
	if _, err := Normalize(atDepth); err != nil {
		t.Fatalf("maximum checkpoint depth rejected: %v", err)
	}
	beyondDepth := Array(atDepth)
	if _, err := Normalize(beyondDepth); !errors.Is(err, ErrLimit) {
		t.Fatalf("over-depth checkpoint error = %v", err)
	}

	atNodeLimit := Value{Type: TypeArray, Array: make([]Value, MaxValueNodes-1)}
	for i := range atNodeLimit.Array {
		atNodeLimit.Array[i] = Nil()
	}
	if _, err := Normalize(atNodeLimit); err != nil {
		t.Fatalf("maximum node count rejected: %v", err)
	}
	beyondNodeLimit := Value{Type: TypeArray, Array: append(append([]Value(nil), atNodeLimit.Array...), Nil())}
	if _, err := Normalize(beyondNodeLimit); !errors.Is(err, ErrLimit) {
		t.Fatalf("over-node-limit checkpoint error = %v", err)
	}
}

func TestCheckpointRejectsAliasesSparseMixedAndUnsupportedKeyTables(t *testing.T) {
	shared := map[string]any{"value": 1}
	if _, err := FromGo(map[string]any{"first": shared, "second": shared}); !errors.Is(err, ErrAliasedTable) {
		t.Fatalf("Go alias error = %v", err)
	}
	tests := []struct {
		name   string
		source string
		want   error
	}{
		{name: "Lua alias", source: `local shared = {value = 1}; return {first = shared, second = shared}`, want: ErrAliasedTable},
		{name: "sparse array", source: `return {[1] = 1, [3] = 3}`, want: ErrInvalidValue},
		{name: "mixed table", source: `return {[1] = 1, name = 2}`, want: ErrInvalidValue},
		{name: "boolean key", source: `return {[true] = 1}`, want: ErrInvalidValue},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			value := executeLuaValue(t, tt.source)
			if _, err := FromLua(value); !errors.Is(err, tt.want) {
				t.Fatalf("FromLua error = %v, want %v", err, tt.want)
			}
		})
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
	function := executeLuaValue(t, "return function() return 1 end")
	coroutine := executeLuaValue(t, "return coroutine.create(function() coroutine.yield(1) end)")
	tableWithMeta := luavm.NewTableWithSize(0, 0)
	tableWithMeta.SetMetatable(luavm.NewTableWithSize(0, 0))
	tests := []struct {
		name  string
		value luavm.Value
		want  error
	}{
		{name: "function", value: function, want: ErrUnsupportedValue},
		{name: "coroutine", value: coroutine, want: ErrUnsupportedValue},
		{name: "userdata", value: luavm.NewUserdataValue("native", nil), want: ErrUnsupportedValue},
		{name: "metatable", value: luavm.NewTable(tableWithMeta), want: ErrMetatable},
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
	table := luavm.NewTableWithSize(0, 1)
	if err := table.Set(luavm.NewString("self"), luavm.NewTable(table)); err != nil {
		t.Fatal(err)
	}
	if _, err := FromLua(luavm.NewTable(table)); !errors.Is(err, ErrCycle) {
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
