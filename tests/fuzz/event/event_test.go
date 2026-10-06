// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package event_test

import (
	"bytes"
	"reflect"
	"testing"

	"github.com/zyc14588/TRPG_PLATFORM/internal/eventstore"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/tests/fuzz/support"
)

func FuzzEventDeserialization(f *testing.F) {
	f.Add(support.JSON(support.Record()))
	f.Add([]byte(`{"format":1,"complete":true}`))
	f.Fuzz(func(t *testing.T, raw []byte) {
		if len(raw) > support.MaxInput {
			return
		}
		before := append([]byte(nil), raw...)
		r, err := eventstore.Decode(raw)
		if !bytes.Equal(raw, before) {
			t.Fatal("event decoder mutated bytes")
		}
		if err != nil {
			return
		}
		if eventstore.Validate(r) != nil {
			t.Fatal("decoder accepted invalid authoritative record")
		}
		round, err := eventstore.Decode(support.JSON(r))
		if err != nil || !reflect.DeepEqual(round, r) {
			t.Fatal("accepted event lost original facts on roundtrip")
		}
	})
}
func FuzzEventUpcaster(f *testing.F) {
	f.Add(support.JSON(support.Upcast()))
	f.Add([]byte(`{"Event":{},"Target":0,"Rules":[]}`))
	f.Fuzz(func(t *testing.T, raw []byte) {
		if len(raw) > support.MaxInput {
			return
		}
		var in support.UpcastInput
		if checkpoint.StrictDecode(raw, &in, support.MaxInput) != nil {
			return
		}
		before := support.JSON(in)
		out, err := eventstore.Read(in.Event, in.Target, in.Rules)
		if !bytes.Equal(before, support.JSON(in)) {
			t.Fatal("Upcaster rewrote original history")
		}
		if err != nil {
			return
		}
		if !reflect.DeepEqual(out.Original, in.Event) || out.Version != in.Target || checkpoint.Validate(out.Value) != nil {
			t.Fatal("accepted Upcaster lost original/version/value")
		}
	})
}
