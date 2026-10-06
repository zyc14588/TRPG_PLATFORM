// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package projection_test

import (
	"github.com/zyc14588/TRPG_PLATFORM/internal/eventstore"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/projection"
	fixture "github.com/zyc14588/TRPG_PLATFORM/internal/session/testdata/replay"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
	"reflect"
	"testing"
)

func TestEverySnapshotBoundaryMatchesFullHistory(t *testing.T) {
	h, o, e := fixture.History(4)
	if e != nil {
		t.Fatal(e)
	}
	original := eventstore.Digest(h)
	want, accepted, e := projection.Rebuild(h, o.Metadata, nil, o.ValidateRecord)
	if e != nil || accepted {
		t.Fatal(e)
	}
	if !reflect.DeepEqual(want.State.Table["counter"], checkpoint.Int(5)) || !reflect.DeepEqual(want.Rows[0].Value.Table["score"], checkpoint.Int(5)) || want.Quantities[0].Value != 5 || want.Cursor != 4 {
		t.Fatal("facts differ")
	}
	prefix, e := projection.Genesis(h.Creation)
	if e != nil {
		t.Fatal(e)
	}
	for n := 0; n <= 4; n++ {
		c, e := projection.Seal(o.Metadata, prefix)
		if e != nil {
			t.Fatal(e)
		}
		got, used, e := projection.Rebuild(h, o.Metadata, &c, o.ValidateRecord)
		if e != nil || !used || !reflect.DeepEqual(got, want) {
			t.Fatalf("boundary %d: %v used %v", n, e, used)
		}
		if n < 4 {
			prefix, e = projection.Apply(prefix, h.Records[n])
			if e != nil {
				t.Fatal(e)
			}
		}
	}
	if eventstore.Digest(h) != original {
		t.Fatal("history mutated")
	}
}
func TestSelfHashedIncompatibleSnapshotsAreIgnored(t *testing.T) {
	h, o, e := fixture.History(2)
	if e != nil {
		t.Fatal(e)
	}
	want, _, e := projection.Rebuild(h, o.Metadata, nil, o.ValidateRecord)
	if e != nil {
		t.Fatal(e)
	}
	good, e := projection.Seal(o.Metadata, want)
	if e != nil {
		t.Fatal(e)
	}
	changes := map[string]func(*projection.Cache){"lock": func(c *projection.Cache) { c.Metadata.Session.DependencyLock = eventstore.Digest("wrong") }, "package": func(c *projection.Cache) { c.Metadata.Session.PackageHashes["wrong"] = eventstore.Digest("wrong") }, "runtime": func(c *projection.Cache) { c.Metadata.Session.RuntimeVersion = "other" }, "profile": func(c *projection.Cache) { c.Metadata.Session.LuaProfile = "other" }, "runner": func(c *projection.Cache) { c.Metadata.RunnerHash = eventstore.Digest("other") }, "schema": func(c *projection.Cache) { c.Metadata.StateSchema = eventstore.Digest("other") }, "event-schema": func(c *projection.Cache) { c.Metadata.EventSchemas = eventstore.Digest("other") }, "checkpoint-schema": func(c *projection.Cache) { c.Metadata.CheckpointSchema = eventstore.Digest("other") }, "version": func(c *projection.Cache) { c.Metadata.Session.StateVersion++ }, "facts": func(c *projection.Cache) { c.Image.State.Table["counter"] = checkpoint.Int(99) }, "cursor": func(c *projection.Cache) { c.Image.Cursor++ }, "history-hash": func(c *projection.Cache) { c.Image.HistoryHash = eventstore.Digest("other") }}
	for name, change := range changes {
		t.Run(name, func(t *testing.T) {
			c := eventstore.Copy(good)
			change(&c)
			c.Hash = ""
			c.Hash = eventstore.Digest(c)
			got, used, e := projection.Rebuild(h, o.Metadata, &c, o.ValidateRecord)
			if e != nil || used || !reflect.DeepEqual(got, want) {
				t.Fatal("cache became authority", used, e)
			}
		})
	}
}
func TestHistoryGapsSchemaSubstitutionAndIncompleteEvidenceFailClosed(t *testing.T) {
	for _, name := range []string{"head", "gap", "duplicate", "incomplete", "schema", "seed", "bound", "ended"} {
		t.Run(name, func(t *testing.T) {
			h, o, e := fixture.History(2)
			if e != nil {
				t.Fatal(e)
			}
			switch name {
			case "head":
				h.Cursor++
			case "gap":
				h.Records = h.Records[1:]
			case "duplicate":
				h.Records[1].Header.CommandID = h.Records[0].Header.CommandID
			case "incomplete":
				h.Records[0].Complete = false
			case "schema":
				h.Records[0].Events[0].SchemaHash = eventstore.Digest("substitute")
			case "seed":
				h.Creation.Seed.Table["counter"] = checkpoint.Int(9)
			case "bound":
				h.Records = make([]data.EffectRecord, eventstore.MaxRecords+1)
			case "ended":
				h.Ended = true
			}
			if _, _, e = projection.Rebuild(h, o.Metadata, nil, o.ValidateRecord); e == nil {
				t.Fatal("invalid history accepted")
			}
		})
	}
}
