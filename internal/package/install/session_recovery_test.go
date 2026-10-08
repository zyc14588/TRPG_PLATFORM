// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package install_test

import (
	"github.com/zyc14588/TRPG_PLATFORM/internal/eventstore"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	fixture "github.com/zyc14588/TRPG_PLATFORM/internal/session/testdata/replay"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
	"testing"
)

func TestRecoveryRevalidatesApprovedIntentSchemasAndGoWrappers(t *testing.T) {
	h, o, err := fixture.History(1)
	if err != nil {
		t.Fatal(err)
	}
	r := h.Records[0]
	r.Tasks[0].Kind = "task"
	r.Outbox[0].Kind = "dispatch-task"
	r.Continuations = []data.Intent{{ID: "continuation", PackageID: r.Tasks[0].PackageID, Kind: "continuation", Payload: checkpoint.Object(map[string]checkpoint.Value{"task": checkpoint.Text(r.Tasks[0].ID), "value": eventstore.Copy(r.Tasks[0].Payload)})}}
	seal := func(r data.EffectRecord) data.EffectRecord { r.Hash = ""; r.Hash = eventstore.Digest(r); return r }
	r = seal(r)
	if err = o.ValidateRecord(r); err != nil {
		t.Fatal("valid Go wrappers rejected", err)
	}
	cases := map[string]func(*data.EffectRecord){
		"unapproved-task-schema": func(r *data.EffectRecord) { r.Tasks[0].Payload = checkpoint.Text("not-approved") },
		"unapproved-package":     func(r *data.EffectRecord) { r.Tasks[0].PackageID = "example.test/other" },
		"unapproved-tool-result": func(r *data.EffectRecord) { r.Inputs.ToolResults = []checkpoint.Value{checkpoint.Text("not-approved")} },
		"dispatch-reference":     func(r *data.EffectRecord) { r.Outbox[0].Payload.Table["task"] = checkpoint.Text("other-task") },
		"dispatch-kind":          func(r *data.EffectRecord) { r.Outbox[0].Kind = "dispatch-ai" },
		"dispatch-extra-field":   func(r *data.EffectRecord) { r.Outbox[0].Payload.Table["state"] = checkpoint.Int(5) },
		"continuation-reference": func(r *data.EffectRecord) { r.Continuations[0].Payload.Table["task"] = checkpoint.Text("other-task") },
		"continuation-schema": func(r *data.EffectRecord) {
			r.Continuations[0].Payload.Table["value"] = checkpoint.Text("not-approved")
		},
		"continuation-kind":         func(r *data.EffectRecord) { r.Continuations[0].Kind = "task" },
		"event-schema-substitution": func(r *data.EffectRecord) { r.Events[0].SchemaHash = eventstore.Digest("other-schema") },
		"row-schema-substitution":   func(r *data.EffectRecord) { r.Rows[0].SchemaHash = eventstore.Digest("other-schema") },
	}
	for name, change := range cases {
		t.Run(name, func(t *testing.T) {
			v := eventstore.Copy(r)
			change(&v)
			v = seal(v)
			if eventstore.Validate(v) != nil {
				t.Fatal("probe did not reach bound schema/wrapper validation")
			}
			if o.ValidateRecord(v) == nil {
				t.Fatal("unapproved replay effect accepted")
			}
		})
	}
}

func TestRecoveryRevalidatesTaskAndAIContinuationReferences(t *testing.T) {
	h, o, err := fixture.History(1)
	if err != nil {
		t.Fatal("bound replay fixture failed")
	}
	original := h.Records[0]
	seal := func(r data.EffectRecord) data.EffectRecord { r.Hash = ""; r.Hash = eventstore.Digest(r); return r }
	bound := func(kind string) data.EffectRecord {
		r := eventstore.Copy(original)
		r.Tasks[0].Kind = kind
		r.Outbox[0].Kind = "dispatch-" + kind
		r.Continuations = []data.Intent{{ID: "continuation", PackageID: r.Tasks[0].PackageID, Kind: "continuation", Payload: checkpoint.Object(map[string]checkpoint.Value{"task": checkpoint.Text(r.Tasks[0].ID), "value": eventstore.Copy(r.Tasks[0].Payload)})}}
		return seal(r)
	}
	for _, kind := range []string{"task", "ai"} {
		t.Run("approved-"+kind, func(t *testing.T) {
			r := bound(kind)
			if eventstore.Validate(r) != nil {
				t.Fatal("fixture failed immutable effect validation")
			}
			if o.ValidateRecord(r) != nil {
				t.Fatal("approved bound continuation rejected")
			}
		})
	}
	cases := map[string]func(*data.EffectRecord){
		"unknown-source":                func(r *data.EffectRecord) { r.Continuations[0].Payload.Table["task"] = checkpoint.Text("unknown-task") },
		"different-source-package":      func(r *data.EffectRecord) { r.Tasks[0].PackageID = "example.test/other" },
		"unapproved-source-kind":        func(r *data.EffectRecord) { r.Tasks[0].Kind = "unapproved"; r.Outbox[0].Kind = "dispatch-unapproved" },
		"unapproved-continuation-value": func(r *data.EffectRecord) { r.Continuations[0].Payload.Table["value"] = checkpoint.Text("unapproved") },
		"extra-wrapper-key":             func(r *data.EffectRecord) { r.Continuations[0].Payload.Table["extra"] = checkpoint.Int(1) },
	}
	for name, change := range cases {
		t.Run(name, func(t *testing.T) {
			r := bound("ai")
			change(&r)
			if o.ValidateRecord(seal(r)) == nil {
				t.Fatal("unapproved continuation reference accepted")
			}
		})
	}
}
