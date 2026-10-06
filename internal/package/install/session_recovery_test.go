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
