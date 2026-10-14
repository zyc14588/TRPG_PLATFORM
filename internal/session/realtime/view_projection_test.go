// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package realtime

import (
	"encoding/json"
	"reflect"
	"strings"
	"testing"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/command"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
)

func selectedView(s *command.ViewSelector) command.ViewPolicy {
	return command.ViewPolicy{ViewProjection: &command.ViewSelector{Kind: "table", Fields: map[string]*command.ViewSelector{"selected": s}}}
}

func TestFilterViewExactDescendantsAreOwnedAndIdempotent(t *testing.T) {
	item := checkpoint.Object(map[string]checkpoint.Value{
		"visible": checkpoint.Text("allowed"), "secret": checkpoint.Text("other-seat-secret"),
		"tags": checkpoint.Array(checkpoint.Text("same"), checkpoint.Text("same")),
	})
	view := checkpoint.Object(map[string]checkpoint.Value{
		"counter": checkpoint.Int(7), "legacy-null": {Kind: "nil"},
		"record": item, "list": checkpoint.Array(item, item), "root-secret": checkpoint.Text("root-secret"),
	})
	p := command.ViewPolicy{
		ViewFields: []string{"counter", "legacy-null"},
		ViewProjection: &command.ViewSelector{Kind: "table", Fields: map[string]*command.ViewSelector{
			"record": {Kind: "table", Fields: map[string]*command.ViewSelector{"visible": {Kind: "scalar"}, "missing": {Kind: "scalar", Nullable: true}}},
			"list": {Kind: "array", Element: &command.ViewSelector{Kind: "table", Fields: map[string]*command.ViewSelector{
				"visible": {Kind: "scalar"}, "tags": {Kind: "array", Element: &command.ViewSelector{Kind: "scalar"}},
			}}},
		}},
	}
	before, _ := json.Marshal(view)
	got := FilterView(view, p)
	want := checkpoint.Object(map[string]checkpoint.Value{
		"counter": checkpoint.Int(7), "legacy-null": {Kind: "nil"},
		"record": checkpoint.Object(map[string]checkpoint.Value{"visible": checkpoint.Text("allowed")}),
		"list": checkpoint.Array(
			checkpoint.Object(map[string]checkpoint.Value{"visible": checkpoint.Text("allowed"), "tags": checkpoint.Array(checkpoint.Text("same"), checkpoint.Text("same"))}),
			checkpoint.Object(map[string]checkpoint.Value{"visible": checkpoint.Text("allowed"), "tags": checkpoint.Array(checkpoint.Text("same"), checkpoint.Text("same"))}),
		),
	})
	if !reflect.DeepEqual(got, want) || checkpoint.Validate(got) != nil {
		t.Fatal("explicit descendants or duplicates changed", got)
	}
	if again := FilterView(got, p); !reflect.DeepEqual(again, got) {
		t.Fatal("projection is not idempotent")
	}
	after, _ := json.Marshal(view)
	if string(before) != string(after) {
		t.Fatal("filter mutated its checkpoint")
	}
	got.Table["record"].Table["visible"] = checkpoint.Text("changed")
	got.Table["list"].Array[0].Table["visible"] = checkpoint.Text("changed")
	got.Table["list"].Array[0].Table["tags"].Array[0] = checkpoint.Text("changed")
	if item.Table["visible"].String != "allowed" || item.Table["tags"].Array[0].String != "same" || got.Table["list"].Array[1].Table["tags"].Array[0].String != "same" || got.Table["list"].Array[1].Table["visible"].String != "allowed" {
		t.Fatal("returned maps, array slices or repeated elements retained aliases")
	}
	delete(p.ViewProjection.Fields["record"].Fields, "visible")
	if got.Table["record"].Table["visible"].String != "changed" {
		t.Fatal("result retained policy aliases")
	}
}

func TestFilterViewMissingNullAndRuntimeKinds(t *testing.T) {
	nilValue := checkpoint.Value{Kind: "nil"}
	for _, tc := range []struct {
		name     string
		selector command.ViewSelector
		value    checkpoint.Value
		present  bool
	}{
		{"string", command.ViewSelector{Kind: "scalar"}, checkpoint.Text("text"), true},
		{"integer", command.ViewSelector{Kind: "scalar"}, checkpoint.Int(3), true},
		{"float", command.ViewSelector{Kind: "scalar"}, checkpoint.Value{Kind: "float", Number: "1.5"}, true},
		{"boolean", command.ViewSelector{Kind: "scalar"}, checkpoint.Bool(false), true},
		{"scalar-null-denied", command.ViewSelector{Kind: "scalar"}, nilValue, false},
		{"scalar-null-selected", command.ViewSelector{Kind: "scalar", Nullable: true}, nilValue, true},
		{"scalar-table-denied", command.ViewSelector{Kind: "scalar"}, checkpoint.Object(map[string]checkpoint.Value{"secret": checkpoint.Text("secret")}), false},
		{"scalar-array-denied", command.ViewSelector{Kind: "scalar"}, checkpoint.Array(checkpoint.Int(1)), false},
		{"empty-table", command.ViewSelector{Kind: "table"}, checkpoint.Object(map[string]checkpoint.Value{}), true},
		{"table-scalar-denied", command.ViewSelector{Kind: "table"}, checkpoint.Text("text"), false},
		{"table-array-denied", command.ViewSelector{Kind: "table"}, checkpoint.Array(), false},
		{"table-null-denied", command.ViewSelector{Kind: "table"}, nilValue, false},
		{"table-null-selected", command.ViewSelector{Kind: "table", Nullable: true}, nilValue, true},
		{"empty-array", command.ViewSelector{Kind: "array", Element: &command.ViewSelector{Kind: "scalar"}}, checkpoint.Array(), true},
		{"array-table-denied", command.ViewSelector{Kind: "array", Element: &command.ViewSelector{Kind: "scalar"}}, checkpoint.Object(map[string]checkpoint.Value{}), false},
		{"array-scalar-denied", command.ViewSelector{Kind: "array", Element: &command.ViewSelector{Kind: "scalar"}}, checkpoint.Int(1), false},
		{"array-null-denied", command.ViewSelector{Kind: "array", Element: &command.ViewSelector{Kind: "scalar"}}, nilValue, false},
		{"array-null-selected", command.ViewSelector{Kind: "array", Nullable: true, Element: &command.ViewSelector{Kind: "scalar"}}, nilValue, true},
	} {
		t.Run(tc.name, func(t *testing.T) {
			p := selectedView(&tc.selector)
			got := FilterView(checkpoint.Object(map[string]checkpoint.Value{"selected": tc.value}), p)
			x, exists := got.Table["selected"]
			if exists != tc.present || exists && (x.Kind != tc.value.Kind || x.Kind != "array" && !reflect.DeepEqual(x, tc.value)) {
				t.Fatal("null or runtime-kind behavior changed", got)
			}
			if absent := FilterView(checkpoint.Object(map[string]checkpoint.Value{}), p); len(absent.Table) != 0 {
				t.Fatal("missing field fabricated", absent)
			}
		})
	}
	view := checkpoint.Object(map[string]checkpoint.Value{"selected": checkpoint.Object(map[string]checkpoint.Value{
		"wrong": checkpoint.Array(checkpoint.Int(1)), "null": nilValue, "secret": checkpoint.Text("secret"),
	})})
	p := selectedView(&command.ViewSelector{Kind: "table", Fields: map[string]*command.ViewSelector{
		"wrong": {Kind: "scalar"}, "null": {Kind: "scalar"}, "missing": {Kind: "scalar", Nullable: true},
	}})
	got := FilterView(view, p)
	if len(got.Table) != 1 || got.Table["selected"].Kind != "table" || len(got.Table["selected"].Table) != 0 {
		t.Fatal("explicit container was not retained as a sparse empty object", got)
	}
}

func TestFilterViewArraysPreserveOrderOrDenyTheWholeArray(t *testing.T) {
	for _, nullable := range []bool{false, true} {
		p := selectedView(&command.ViewSelector{Kind: "array", Element: &command.ViewSelector{Kind: "scalar", Nullable: nullable}})
		for _, values := range [][]checkpoint.Value{
			{checkpoint.Text("a"), checkpoint.Int(2), checkpoint.Text("a"), checkpoint.Bool(true)},
			{checkpoint.Text("a"), {Kind: "nil"}, checkpoint.Text("a")},
			{checkpoint.Text("a"), checkpoint.Object(map[string]checkpoint.Value{}), checkpoint.Text("a")},
			{checkpoint.Text("a"), checkpoint.Array(checkpoint.Int(1)), checkpoint.Text("a")},
		} {
			got := FilterView(checkpoint.Object(map[string]checkpoint.Value{"selected": checkpoint.Array(values...)}), p)
			x, exists := got.Table["selected"]
			want := values[1].Kind != "table" && values[1].Kind != "array" && (values[1].Kind != "nil" || nullable)
			if exists != want || exists && !reflect.DeepEqual(x.Array, values) {
				t.Fatal("array shifted, fabricated a null or ignored an incompatible element", got)
			}
		}
	}
	element := &command.ViewSelector{Kind: "table", Fields: map[string]*command.ViewSelector{"visible": {Kind: "scalar"}}}
	p := selectedView(&command.ViewSelector{Kind: "array", Element: element})
	values := []checkpoint.Value{checkpoint.Object(map[string]checkpoint.Value{"secret": checkpoint.Text("denied")}), checkpoint.Object(map[string]checkpoint.Value{"visible": checkpoint.Int(1)})}
	got := FilterView(checkpoint.Object(map[string]checkpoint.Value{"selected": checkpoint.Array(values...)}), p)
	if len(got.Table["selected"].Array) != 2 || len(got.Table["selected"].Array[0].Table) != 0 || got.Table["selected"].Array[1].Table["visible"].Number != "1" {
		t.Fatal("selected table elements lost position or disclosed denied children", got)
	}
	values[0] = checkpoint.Value{Kind: "nil"}
	if got = FilterView(checkpoint.Object(map[string]checkpoint.Value{"selected": checkpoint.Array(values...)}), p); len(got.Table) != 0 {
		t.Fatal("nonnullable table element salvaged an array")
	}
	element.Nullable = true
	if got = FilterView(checkpoint.Object(map[string]checkpoint.Value{"selected": checkpoint.Array(values...)}), p); len(got.Table["selected"].Array) != 2 || got.Table["selected"].Array[0].Kind != "nil" {
		t.Fatal("nullable table element lost its explicit null")
	}
	p = selectedView(&command.ViewSelector{Kind: "array", Element: &command.ViewSelector{Kind: "array", Element: &command.ViewSelector{Kind: "scalar"}}})
	view := checkpoint.Object(map[string]checkpoint.Value{"selected": checkpoint.Array(checkpoint.Array(checkpoint.Int(1)), checkpoint.Array(checkpoint.Object(map[string]checkpoint.Value{})))})
	if got = FilterView(view, p); len(got.Table) != 0 {
		t.Fatal("nested array mismatch was partially salvaged")
	}
}

func TestFilterViewRejectsCompleteInvalidCheckpoints(t *testing.T) {
	tooDeep := checkpoint.Int(1)
	for range checkpoint.MaxDepth + 1 {
		tooDeep = checkpoint.Array(tooDeep)
	}
	tooMany := make([]checkpoint.Value, checkpoint.MaxNodes)
	for n := range tooMany {
		tooMany[n] = checkpoint.Int(1)
	}
	cycle := checkpoint.Object(map[string]checkpoint.Value{})
	cycle.Table["cycle"] = cycle
	for name, invalid := range map[string]checkpoint.Value{
		"capability":  checkpoint.Text("cap:private-handle"),
		"nan":         {Kind: "float", Number: "NaN"},
		"wrong-shape": {Kind: "string", String: "text", Table: map[string]checkpoint.Value{}},
		"utf8":        checkpoint.Text(string([]byte{0xff})),
		"depth":       tooDeep,
		"nodes":       checkpoint.Array(tooMany...),
		"bytes":       checkpoint.Text(strings.Repeat("a", checkpoint.MaxBytes)),
		"cycle":       cycle,
	} {
		t.Run(name, func(t *testing.T) {
			view := checkpoint.Object(map[string]checkpoint.Value{"counter": checkpoint.Int(1), "invalid-unselected": invalid})
			if got := FilterView(view, command.ViewPolicy{ViewFields: []string{"counter"}}); got.Kind != "table" || len(got.Table) != 0 {
				t.Fatal("invalid unselected input was salvaged", got)
			}
		})
	}
	if got := FilterView(checkpoint.Array(checkpoint.Int(1)), command.ViewPolicy{}); got.Kind != "table" || len(got.Table) != 0 {
		t.Fatal("non-table root accepted")
	}
}

func TestFilterViewDirectPolicyArgumentsFailClosed(t *testing.T) {
	cycle := &command.ViewSelector{Kind: "table"}
	cycle.Fields = map[string]*command.ViewSelector{"loop": cycle}
	deep := &command.ViewSelector{Kind: "scalar"}
	for range checkpoint.MaxDepth + 1 {
		deep = &command.ViewSelector{Kind: "table", Fields: map[string]*command.ViewSelector{"child": deep}}
	}
	view := checkpoint.Object(map[string]checkpoint.Value{"counter": checkpoint.Int(1), "selected": checkpoint.Text("private")})
	oversize := command.ViewPolicy{ViewFields: make([]string, 128), ViewProjection: &command.ViewSelector{Kind: "table"}}
	for n := range oversize.ViewFields {
		oversize.ViewFields[n] = "counter"
	}
	for _, p := range []command.ViewPolicy{
		oversize,
		{ViewFields: []string{"counter"}, ViewProjection: cycle},
		{ViewFields: []string{"counter"}, ViewProjection: deep},
		{ViewFields: []string{"counter"}, ViewProjection: &command.ViewSelector{Kind: "table", Fields: map[string]*command.ViewSelector{"selected": nil}}},
		{ViewFields: []string{"counter"}, ViewProjection: &command.ViewSelector{Kind: "table", Nullable: true}},
		{ViewFields: []string{"counter"}, ViewProjection: &command.ViewSelector{Kind: "scalar"}},
		{ViewFields: []string{"counter"}, ViewProjection: &command.ViewSelector{Kind: "table", Fields: map[string]*command.ViewSelector{"counter": {Kind: "scalar"}}}},
		{ViewFields: []string{"counter", "invalid/field"}},
	} {
		if got := FilterView(view, p); len(got.Table) != 0 {
			t.Fatal("invalid direct policy granted any view", got)
		}
	}
}

func TestNestedViewOptInPreservesLegacyResultsAndEvents(t *testing.T) {
	view := checkpoint.Object(map[string]checkpoint.Value{
		"counter": checkpoint.Int(1), "nil": {Kind: "nil"},
		"nested": checkpoint.Object(map[string]checkpoint.Value{"secret": checkpoint.Text("secret")}),
	})
	legacy := command.ViewPolicy{ViewFields: []string{"counter", "nil", "nested"}, ResultFields: []string{"counter", "nil", "nested"}, EventFields: map[string][]string{"event": {"counter", "nil", "nested"}}, ScalarResult: true}
	if got := FilterView(view, legacy); !reflect.DeepEqual(got, Filter(view, legacy.ViewFields)) || len(got.Table) != 2 {
		t.Fatal("legacy scalar or null view behavior changed", got)
	}
	optIn := legacy
	optIn.ViewFields = []string{"counter", "nil"}
	optIn.ViewProjection = &command.ViewSelector{Kind: "table", Fields: map[string]*command.ViewSelector{"nested": {Kind: "table", Fields: map[string]*command.ViewSelector{"secret": {Kind: "scalar"}}}}}
	if got := FilterResult(view, optIn); !reflect.DeepEqual(got, FilterResult(view, legacy)) || len(got.Table) != 2 {
		t.Fatal("view opt-in expanded result grants")
	}
	for _, scalar := range []checkpoint.Value{checkpoint.Int(7), checkpoint.Text("text"), checkpoint.Bool(false), {Kind: "nil"}} {
		if !reflect.DeepEqual(FilterResult(scalar, optIn), scalar) {
			t.Fatal("legacy scalar result changed")
		}
	}
	events := []data.JournalEvent{{Sequence: 1, Event: data.Event{Type: "event", Payload: view}}, {Sequence: 2, Event: data.Event{Type: "denied", Payload: view}}}
	got := FilterEvents(events, optIn)
	if len(got) != 1 || len(got[0].Event.Payload.Table) != 2 || !reflect.DeepEqual(got, FilterEvents(events, legacy)) {
		t.Fatal("view opt-in expanded event grants")
	}
}
