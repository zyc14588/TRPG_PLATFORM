// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package checkpoint

import (
	"strings"
	"testing"
)

func binding() Binding {
	return Binding{SessionID: "session-a", StateVersion: 7, PackageHashes: map[string]string{"test/game": "sha256:" + strings.Repeat("a", 64)}, DependencyLock: "sha256:" + strings.Repeat("b", 64), LuaProfile: "platform-lua-5.5-p1", RuntimeVersion: "golua-v2.0.5/p1"}
}

func TestCheckpointRoundTripAndDeterminism(t *testing.T) {
	v := Object(map[string]Value{"z": Array(Value{Kind: "nil"}, Bool(true), Int(9223372036854775807)), "a": Text("你好")})
	a, err := Seal(binding(), v)
	if err != nil {
		t.Fatal(err)
	}
	b, err := Seal(binding(), v)
	if err != nil || a.Digest != b.Digest {
		t.Fatalf("nondeterministic: %v", err)
	}
	raw, err := Encode(a)
	if err != nil {
		t.Fatal(err)
	}
	got, err := Decode(raw, binding())
	if err != nil || got.Digest != a.Digest {
		t.Fatalf("roundtrip: %v", err)
	}
}

func TestCheckpointRejectsInvalidValuesAndBindings(t *testing.T) {
	for _, v := range []Value{{Kind: "function"}, {Kind: "thread"}, {Kind: "userdata"}, {Kind: "handle"}, {Kind: "float", Number: "NaN"}, {Kind: "float", Number: "+Inf"}, {Kind: "string", String: string([]byte{255})}, Text("cap:forged"), {Kind: "integer", Number: "9223372036854775808"}} {
		if _, err := Seal(binding(), v); err == nil {
			t.Fatalf("accepted %#v", v)
		}
	}
	cycle := Object(map[string]Value{})
	cycle.Table["self"] = cycle
	if _, err := Seal(binding(), cycle); err == nil {
		t.Fatal("cycle accepted")
	}
	c, _ := Seal(binding(), Int(42))
	raw, _ := Encode(c)
	changes := []func(*Binding){func(b *Binding) { b.SessionID = "other" }, func(b *Binding) { b.StateVersion++ }, func(b *Binding) { b.LuaProfile = "other" }, func(b *Binding) { b.RuntimeVersion = "other" }, func(b *Binding) { b.DependencyLock = "sha256:" + strings.Repeat("c", 64) }, func(b *Binding) {
		b.PackageHashes = map[string]string{"test/game": "sha256:" + strings.Repeat("d", 64)}
	}}
	for _, change := range changes {
		b := binding()
		change(&b)
		if _, err := Decode(raw, b); err == nil {
			t.Fatal("binding mismatch accepted")
		}
	}
	if _, err := Decode(append(raw, []byte("{}")...), binding()); err == nil {
		t.Fatal("trailing JSON accepted")
	}
	if _, err := Decode([]byte(`{"binding":{},"binding":{},"state":{"kind":"nil"},"digest":"x"}`), binding()); err == nil {
		t.Fatal("duplicate keys accepted")
	}
}

func TestCheckpointSizeDepthAndTampering(t *testing.T) {
	v := Text(strings.Repeat("x", MaxBytes+1))
	if _, err := Seal(binding(), v); err == nil {
		t.Fatal("large value accepted")
	}
	v = Int(1)
	for range MaxDepth + 1 {
		v = Array(v)
	}
	if _, err := Seal(binding(), v); err == nil {
		t.Fatal("deep value accepted")
	}
	c, _ := Seal(binding(), Int(1))
	c.State = Int(2)
	raw, _ := Encode(c)
	if _, err := Decode(raw, binding()); err == nil {
		t.Fatal("tampering accepted")
	}
}

func TestCheckpointUnicodeAndMaximumValidDepth(t *testing.T) {
	for _, raw := range []string{`"\ud800"`, `"\udfff"`, `"\ud800\u0041"`} {
		var s string
		if err := StrictDecode([]byte(raw), &s, MaxBytes); err == nil {
			t.Fatal("surrogate accepted", raw)
		}
	}
	var s string
	if err := StrictDecode([]byte(`"\ud83d\ude00"`), &s, MaxBytes); err != nil || s != "😀" {
		t.Fatal("valid Unicode rejected", err)
	}
	v := Int(1)
	for range MaxDepth {
		v = Array(v)
	}
	c, err := Seal(binding(), v)
	if err != nil {
		t.Fatal(err)
	}
	raw, err := Encode(c)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := Decode(raw, binding()); err != nil {
		t.Fatal("valid maximum depth is not round-trippable", err)
	}
}
