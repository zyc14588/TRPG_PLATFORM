// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// Package support supplies bounded, synthetic seed data. It does not create a
// Session, grant a capability, or impersonate factory authentication.
package support

import (
	"encoding/json"

	"github.com/zyc14588/TRPG_PLATFORM/internal/eventstore"
	"github.com/zyc14588/TRPG_PLATFORM/internal/hostapi"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/ipc"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/archive"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/extension"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/install"
	fixture "github.com/zyc14588/TRPG_PLATFORM/internal/package/testdata/migration"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/command"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/migration"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
	minimal "github.com/zyc14588/TRPG_PLATFORM/tests/fixture-minimal"
)

const MaxInput = 64 << 10

func JSON(v any) []byte {
	raw, err := json.Marshal(v)
	if err != nil {
		panic(err)
	}
	return raw
}
func Pair() fixture.Pair {
	p, err := minimal.Template()
	if err != nil {
		panic(err)
	}
	return p
}
func Schema(p *archive.Package, path string, seed checkpoint.Value) hostapi.Schema {
	v, ok := p.Entry(path)
	if !ok {
		panic("missing seed schema")
	}
	s, err := hostapi.BindSchema(p, path, checkpoint.Hash(v.Bytes()), seed)
	if err != nil {
		panic(err)
	}
	return s
}
func Archive() []byte {
	s, err := Pair().Old.Root.Export()
	if err != nil {
		panic(err)
	}
	return s.Bytes()
}
func Manifest() []byte { return Pair().Old.Root.ManifestBytes() }
func Protocol() []byte {
	return JSON(command.Envelope{CommandID: "fixed", SessionID: "fixture", ExpectedStateVersion: 1, SeatID: "gm", Type: "increment", Payload: checkpoint.Object(map[string]checkpoint.Value{"delta": checkpoint.Int(7)}), CorrelationID: "fixture"})
}
func Callback() []byte {
	return JSON(ipc.Callback{Kind: "callback", Version: ipc.Version, ID: 1, Sequence: 1, PID: 1, Profile: profile.ID, Runtime: profile.RuntimeVersion, Call: profile.HostCall{Module: "fixture", Line: 1, Phase: "execute", Capability: "host.db", Operation: "named", Arguments: []checkpoint.Value{checkpoint.Text("increase"), checkpoint.Object(map[string]checkpoint.Value{"key": checkpoint.Text("one"), "delta": checkpoint.Int(7)})}}})
}

func Record() data.EffectRecord {
	before, after := checkpoint.Int(1), checkpoint.Int(2)
	schema := checkpoint.Hash([]byte("bounded-event-schema"))
	b := data.Binding{Workspace: "fixture", Session: "fixture", GraphHash: checkpoint.Hash([]byte("bounded-graph"))}
	s := data.Snapshot{Binding: b, Version: 1, SchemaHash: schema, State: checkpoint.Object(map[string]checkpoint.Value{"counter": before})}
	h := data.Header{Binding: b, Principal: "gm", CommandID: "fixed", Fingerprint: checkpoint.Hash([]byte("fixed-input")), ExpectedVersion: 1}
	c := data.Commit{Header: h, SchemaHash: schema, State: checkpoint.Object(map[string]checkpoint.Value{"counter": after}), Result: after, Inputs: data.Inputs{Callback: "execute_command", Time: 1700000000, Random: []int64{7}, Command: checkpoint.Object(map[string]checkpoint.Value{})}, Patches: []data.Patch{{Path: []string{"counter"}, BeforeHash: eventstore.Digest(before), AfterHash: eventstore.Digest(after), Before: &before, After: &after, Module: "fixture", Line: 1, CommandID: "fixed"}}, Events: []data.Event{{ID: "fixed-event", Type: "example.test/fuzz/change", Payload: checkpoint.Object(map[string]checkpoint.Value{"counter": after}), SchemaHash: schema, SchemaVersion: 1}}}
	r, err := eventstore.Record(s, c, 1)
	if err != nil {
		panic(err)
	}
	return r
}

type UpcastInput struct {
	Event  data.Event
	Target uint64
	Rules  []eventstore.Upcast
}

func Upcast() UpcastInput {
	h := checkpoint.Hash([]byte("bounded-event-schema"))
	return UpcastInput{Event: data.Event{ID: "fixed-event", Type: "example.test/fuzz/change", Payload: checkpoint.Object(map[string]checkpoint.Value{"counter": checkpoint.Int(9007199254740993)}), SchemaHash: h, SchemaVersion: 1}, Target: 2, Rules: []eventstore.Upcast{{Type: "example.test/fuzz/change", From: 1, To: 2, SchemaHash: checkpoint.Hash([]byte("target-event-schema")), Rename: map[string]string{"counter": "total"}}}}
}
func Migration() (data.Snapshot, install.RecoveryContext, migration.Plan) {
	p := Pair()
	b := data.Binding{Workspace: "fixture", Session: "fixture", GraphHash: p.Old.Hash()}
	target := data.Binding{Workspace: b.Workspace, Session: b.Session, GraphHash: p.New.Hash()}
	oldState := Schema(p.Old.Root, "schemas/state.schema.json", fixture.State(false, 8))
	oldRow := Schema(p.Old.Root, "schemas/row.schema.json", checkpoint.Object(map[string]checkpoint.Value{"score": checkpoint.Int(8)}))
	newState := Schema(p.New.Root, "schemas/state.schema.json", fixture.State(true, 8))
	newRow := Schema(p.New.Root, "schemas/row.schema.json", checkpoint.Object(map[string]checkpoint.Value{"value": checkpoint.Int(8)}))
	s := data.Snapshot{Binding: b, Version: 2, SchemaHash: oldState.Digest(), State: fixture.State(false, 8), Rows: []data.Row{{PackageID: fixture.PackageID, Namespace: "docs", Key: "one", SchemaHash: oldRow.Digest(), Value: checkpoint.Object(map[string]checkpoint.Value{"score": checkpoint.Int(8)})}}, Quantities: []data.Quantity{{PackageID: fixture.PackageID, Table: "quantity", Key: "one", Value: 7}}}
	o := install.RecoveryContext{Binding: target, StateSchema: newState, Namespaces: map[string]hostapi.Schema{fixture.PackageID + "/records": newRow}}
	return s, o, p.Plan()
}

// WithSchema passes fuzzed schema bytes through the real immutable archive
// and package-bound Host schema loader, with its local-only resolver/budgets.
func WithSchema(raw []byte) (hostapi.Schema, error) {
	p := Pair().Old.Root
	files := map[string][]byte{}
	for _, e := range p.Entries() {
		files[e.Path()] = e.Bytes()
	}
	files["schemas/fuzz.schema.json"] = append([]byte(nil), raw...)
	p, err := archive.FromFiles(files, p.ExactLock(), extension.DefaultSupport)
	if err != nil {
		return hostapi.Schema{}, err
	}
	return hostapi.BindSchema(p, "schemas/fuzz.schema.json", checkpoint.Hash(raw), checkpoint.Bool(true))
}
