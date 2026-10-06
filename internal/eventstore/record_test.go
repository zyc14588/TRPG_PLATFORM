// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package eventstore_test

import (
	"encoding/json"
	"github.com/zyc14588/TRPG_PLATFORM/internal/eventstore"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	fixture "github.com/zyc14588/TRPG_PLATFORM/internal/session/testdata/replay"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
	"reflect"
	"testing"
)

func record(t *testing.T) data.EffectRecord {
	t.Helper()
	h, _, e := fixture.History(1)
	if e != nil {
		t.Fatal(e)
	}
	return h.Records[0]
}
func reseal(r data.EffectRecord) data.EffectRecord {
	r.Hash = ""
	r.Hash = eventstore.Digest(r)
	return r
}

func TestOriginalEffectsInputsAndSchemaRoundTrip(t *testing.T) {
	r := record(t)
	before := eventstore.Digest(r)
	raw, _ := json.Marshal(r)
	out, e := eventstore.Decode(raw)
	if e != nil || !reflect.DeepEqual(out, r) || out.Events[0].SchemaVersion != 1 || out.Inputs.Time != 1000 || out.Inputs.Random[0] != 7 || !reflect.DeepEqual(out.Inputs.ToolResults[0], checkpoint.Int(7)) {
		t.Fatal("lost original evidence", e)
	}
	out.Patches[0].After.Number = "100"
	out.Events[0].Payload.Table["counter"] = checkpoint.Int(99)
	out.Inputs.Random[0] = 9
	if eventstore.Digest(r) != before {
		t.Fatal("decode aliases original")
	}
}
func TestIncompleteTamperedOrUnboundedHistoryRejected(t *testing.T) {
	cases := map[string]func(*data.EffectRecord){
		"incomplete":           func(r *data.EffectRecord) { r.Complete = false },
		"unknown-schema":       func(r *data.EffectRecord) { r.Events[0].SchemaVersion = 0 },
		"cursor-gap":           func(r *data.EffectRecord) { r.Cursor++ },
		"version-gap":          func(r *data.EffectRecord) { r.Version++ },
		"event-duplicate":      func(r *data.EffectRecord) { r.Events = append(r.Events, r.Events[0]); r.Cursor++ },
		"patch-missing-before": func(r *data.EffectRecord) { r.Patches[0].Before = nil },
		"patch-hash":           func(r *data.EffectRecord) { r.Patches[0].AfterHash = eventstore.Digest("wrong") },
		"authority-event":      func(r *data.EffectRecord) { r.Events = nil; r.Cursor = r.BeforeCursor },
		"capability-value":     func(r *data.EffectRecord) { r.Inputs.ToolResults = []checkpoint.Value{checkpoint.Text("cap:forged")} },
		"input-bound":          func(r *data.EffectRecord) { r.Inputs.Random = make([]int64, 257) },
		"tool-result-bound":    func(r *data.EffectRecord) { r.Inputs.ToolResults = make([]checkpoint.Value, 33) },
		"tenant":               func(r *data.EffectRecord) { r.Header.Binding.Workspace = "" },
	}
	for name, change := range cases {
		t.Run(name, func(t *testing.T) {
			r := record(t)
			change(&r)
			r = reseal(r)
			if eventstore.Validate(r) == nil {
				t.Fatal("accepted invalid evidence")
			}
		})
	}
	r := record(t)
	r.Inputs.Time++
	if eventstore.Validate(r) == nil {
		t.Fatal("accepted changed bytes without reseal")
	}
	raw, _ := json.Marshal(record(t))
	raw = append(raw, []byte(" {}")...)
	if _, e := eventstore.Decode(raw); e == nil {
		t.Fatal("accepted trailing object")
	}
}
func TestPatchesCheckEveryBeforeAndNeverMutateSeed(t *testing.T) {
	h, _, e := fixture.History(1)
	if e != nil {
		t.Fatal(e)
	}
	seedHash := eventstore.Digest(h.Creation.Seed)
	out, e := eventstore.ApplyPatches(h.Creation.Seed, h.Records[0])
	if e != nil || !reflect.DeepEqual(out.Table["counter"], checkpoint.Int(2)) {
		t.Fatal(e)
	}
	if eventstore.Digest(h.Creation.Seed) != seedHash {
		t.Fatal("mutated genesis")
	}
	r := h.Records[0]
	wrong := checkpoint.Int(55)
	r.Patches[0].Before = &wrong
	r.Patches[0].BeforeHash = eventstore.Digest(wrong)
	if _, e = eventstore.ApplyPatches(h.Creation.Seed, r); e == nil {
		t.Fatal("ignored intermediate before fact")
	}
	r = h.Records[0]
	before, after := h.Creation.Seed, out
	r.Patches = []data.Patch{{Before: &before, After: &after, BeforeHash: eventstore.Digest(before), AfterHash: eventstore.Digest(after)}}
	if got, e := eventstore.ApplyPatches(before, r); e != nil || !reflect.DeepEqual(got, after) {
		t.Fatal("root patch", e)
	}
}
func TestUpcastPreservesOriginalAndRejectsAmbiguity(t *testing.T) {
	e := record(t).Events[0]
	original := eventstore.Digest(e)
	rule := eventstore.Upcast{Type: e.Type, From: 1, To: 2, SchemaHash: eventstore.Digest("schema-v2"), Rename: map[string]string{"counter": "count"}}
	out, err := eventstore.Read(e, 2, []eventstore.Upcast{rule})
	if err != nil || !reflect.DeepEqual(out.Value.Table["count"], checkpoint.Int(2)) || out.Version != 2 || !reflect.DeepEqual(out.Original, e) {
		t.Fatal(err)
	}
	out.Value.Table["secret"] = checkpoint.Text("changed")
	if eventstore.Digest(e) != original {
		t.Fatal("rewrote original")
	}
	for name, rules := range map[string][]eventstore.Upcast{"missing": nil, "ambiguous": {rule, rule}, "conflict": {{Type: e.Type, From: 1, To: 2, SchemaHash: rule.SchemaHash, Rename: map[string]string{"counter": "secret"}}}, "gap": {{Type: e.Type, From: 1, To: 3, SchemaHash: rule.SchemaHash}}} {
		t.Run(name, func(t *testing.T) {
			if _, e := eventstore.Read(e, 2, rules); e == nil {
				t.Fatal("invalid upcast accepted")
			}
		})
	}
}
