// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package migration

import (
	"github.com/zyc14588/TRPG_PLATFORM/internal/eventstore"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"reflect"
	"testing"
)

func TestFieldTransformPreservesIntegersAndOriginalFacts(t *testing.T) {
	before := checkpoint.Object(map[string]checkpoint.Value{"counter": checkpoint.Int(9007199254740993), "secret": checkpoint.Text("private")})
	digest := eventstore.Digest(before)
	after, err := transform(before, []FieldMove{{From: "counter", To: "total"}}, map[string]checkpoint.Value{"epoch": checkpoint.Int(2)})
	if err != nil || !reflect.DeepEqual(after.Table["total"], checkpoint.Int(9007199254740993)) || eventstore.Digest(before) != digest {
		t.Fatal("transform lost integer or mutated source", err)
	}
}
func TestFieldTransformRejectsAmbiguityAndResourceExpansion(t *testing.T) {
	input := checkpoint.Object(map[string]checkpoint.Value{"a": checkpoint.Int(1), "b": checkpoint.Int(2)})
	for _, c := range []struct {
		Name     string
		Moves    []FieldMove
		Defaults map[string]checkpoint.Value
	}{{Name: "overwrite", Moves: []FieldMove{{From: "a", To: "b"}}}, {Name: "missing", Moves: []FieldMove{{From: "missing", To: "c"}}}, {Name: "duplicate", Moves: []FieldMove{{From: "a", To: "c"}, {From: "a", To: "d"}}}, {Name: "capability-shaped-key", Moves: []FieldMove{{From: "a", To: "cap:unsafe"}}}, {Name: "default-overwrite", Defaults: map[string]checkpoint.Value{"a": checkpoint.Int(7)}}, {Name: "too-many-fields", Moves: make([]FieldMove, 65)}} {
		t.Run(c.Name, func(t *testing.T) {
			if _, err := transform(input, c.Moves, c.Defaults); err == nil {
				t.Fatal("ambiguous plan accepted")
			}
		})
	}
}
func TestOperatorRequiresExplicitFactoryAndLease(t *testing.T) {
	if _, err := New(nil, nil); err == nil {
		t.Fatal("zero authority operator accepted")
	}
}
