// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// Package replayfixture supplies synthetic data, not production certificates.
package replayfixture

import (
	"fmt"
	"github.com/zyc14588/TRPG_PLATFORM/internal/eventstore"
	"github.com/zyc14588/TRPG_PLATFORM/internal/hostapi"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/install"
	base "github.com/zyc14588/TRPG_PLATFORM/internal/session/testdata"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
)

// FactsSource checks actual package-data reads inside the installed VM. The
// creation seed has no document; every committed command materializes one.
var FactsSource = base.IncrementSource[:len(base.IncrementSource)-len("return M\n")] + `
M.project_view=function()
 local counter=host.state.get({"counter"});local doc=host.db.get("docs","one")
 if counter>1 then assert(doc.score==counter) else assert(doc==nil) end
 return {counter=counter,secret=host.state.get({"secret"})}
end
M.on_session_restore=function(input)
 if input.counter~=nil then
  assert(input.counter==host.state.get({"counter"}))
  if input.counter>1 then assert(host.db.get("docs","one").score==input.counter) end
 end
 return {}
end
M.restore_checkpoint=function(input)
 if input.counter~=nil then assert(input.counter==host.state.get({"counter"})) end
 return {}
end
return M
`

func Context() (install.RecoveryContext, error) {
	runner := eventstore.Digest("synthetic-unit-runner")
	pkg, cfg, err := base.Build(install.RuntimeConfig{SHA256: runner, Limits: profile.DefaultLimits()}, "")
	if err != nil {
		return install.RecoveryContext{}, err
	}
	h := cfg.Artifacts[string(pkg.ArtifactIdentity().Digest())].Host
	bind := func(r install.SchemaReference) (hostapi.Schema, error) {
		return hostapi.BindSchema(pkg, r.Path, r.Digest, r.Seed)
	}
	o := install.RecoveryContext{Binding: data.Binding{Workspace: "unit-workspace", Session: "unit-session", GraphHash: eventstore.Digest("synthetic-lock")}, Seed: base.State(1), EventSchemas: map[string]hostapi.Schema{}, Namespaces: map[string]hostapi.Schema{}, IntentSchemas: map[string]hostapi.Schema{}}
	if o.StateSchema, err = bind(h.State); err != nil {
		return o, err
	}
	if o.ResultSchema, err = bind(h.Result); err != nil {
		return o, err
	}
	if o.CheckpointSchema, err = bind(h.Results["create_checkpoint"]); err != nil {
		return o, err
	}
	for name, r := range h.Events {
		s, e := bind(r)
		if e != nil {
			return o, e
		}
		o.EventSchemas[name] = s
	}
	for name, r := range h.Intents {
		s, e := bind(r)
		if e != nil {
			return o, e
		}
		o.IntentSchemas[name] = s
	}
	for name, r := range h.Namespaces {
		s, e := bind(r.Schema)
		if e != nil {
			return o, e
		}
		o.Namespaces[name] = s
	}
	o.Metadata = checkpoint.RecoveryBinding{Session: checkpoint.Binding{SessionID: o.Binding.Session, StateVersion: 1, PackageHashes: map[string]string{base.PackageID: string(pkg.ContentHash())}, DependencyLock: o.Binding.GraphHash, LuaProfile: profile.ID, RuntimeVersion: profile.RuntimeVersion}, Workspace: o.Binding.Workspace, ArtifactsHash: eventstore.Digest("synthetic-unit-artifacts"), StateSchema: o.StateSchema.Digest(), EventSchemas: eventstore.Digest("synthetic-unit-event-schemas"), CheckpointSchema: o.CheckpointSchema.Digest(), RunnerHash: runner}
	return o, nil
}

func History(n int) (data.ReplayHistory, install.RecoveryContext, error) {
	o, err := Context()
	if err != nil {
		return data.ReplayHistory{}, o, err
	}
	h := data.ReplayHistory{Creation: data.Creation{Binding: o.Binding, Version: 1, SchemaHash: o.StateSchema.Digest(), Seed: base.State(1), SeedHash: eventstore.Digest(base.State(1)), ArtifactsHash: o.Metadata.ArtifactsHash}, Version: 1}
	for k := 1; k <= n; k++ {
		before, after := checkpoint.Int(int64(k)), checkpoint.Int(int64(k+1))
		id := fmt.Sprintf("command-%d", k)
		eid := fmt.Sprintf("event-%d", k)
		inputs := data.Inputs{Callback: "command", Time: 1000, Random: []int64{7}, ToolResults: []checkpoint.Value{checkpoint.Int(7)}, Command: base.CommandInput("increment", 1), Envelope: &data.EnvelopeMetadata{Seat: "gm", Type: "increment", Correlation: "fixture-case"}}
		header := data.Header{Binding: o.Binding, Principal: "gm", CommandID: id, Fingerprint: eventstore.Digest(inputs), ExpectedVersion: uint64(k)}
		intent := data.Intent{ID: fmt.Sprintf("intent-%d", k), PackageID: base.PackageID, Kind: "ai", Payload: checkpoint.Object(map[string]checkpoint.Value{"value": after})}
		c := data.Commit{Header: header, State: base.State(int64(k + 1)), SchemaHash: h.Creation.SchemaHash, Inputs: inputs, Result: after, Patches: []data.Patch{{Path: []string{"counter"}, Before: &before, After: &after, BeforeHash: eventstore.Digest(before), AfterHash: eventstore.Digest(after), CommandID: id, EventID: eid}}, Events: []data.Event{{ID: eid, Type: base.PackageID + "/change", Payload: base.State(int64(k + 1)), SchemaVersion: 1, SchemaHash: o.EventSchemas[base.PackageID+"/change"].Digest()}}, Rows: []data.Row{{PackageID: base.PackageID, Namespace: "docs", Key: "one", SchemaHash: o.Namespaces[base.PackageID+"/docs"].Digest(), Value: checkpoint.Object(map[string]checkpoint.Value{"score": after})}}, Quantities: []data.Quantity{{PackageID: base.PackageID, Table: "quantity", Key: "one", Value: int64(k + 1)}}, Tasks: []data.Intent{intent}, Outbox: []data.Intent{{ID: fmt.Sprintf("outbox-%d", k), PackageID: base.PackageID, Kind: "dispatch-ai", Payload: checkpoint.Object(map[string]checkpoint.Value{"task": checkpoint.Text(intent.ID)})}}}
		r, e := eventstore.Record(data.Snapshot{State: base.State(int64(k))}, c, h.Cursor)
		if e != nil {
			return h, o, e
		}
		h.Records = append(h.Records, r)
		h.Version = r.Version
		h.Cursor = r.Cursor
	}
	return h, o, nil
}
