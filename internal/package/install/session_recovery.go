// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package install

import (
	"context"
	"encoding/json"
	"reflect"
	"sort"

	"github.com/zyc14588/TRPG_PLATFORM/internal/eventstore"
	"github.com/zyc14588/TRPG_PLATFORM/internal/hostapi"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/migration"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
)

// RecoveryContext is created only after current ACL/object/policy/runtime
// authentication. Its schemas are bound to the verified installed graph.
type RecoveryContext struct {
	Graph                *store.Graph
	Binding              data.Binding
	Seed                 checkpoint.Value
	Metadata             checkpoint.RecoveryBinding
	StateSchema          hostapi.Schema
	ResultSchema         hostapi.Schema
	CheckpointSchema     hostapi.Schema
	EventSchemas         map[string]hostapi.Schema
	Namespaces           map[string]hostapi.Schema
	IntentSchemas        map[string]hostapi.Schema
	Lock                 data.SessionLock
	Epochs               map[string]RecoveryContext
	OriginGraphHash      string
	SafeBoundaryDeclared bool
	QuantityPackages     map[string]bool
}
type RecoveryResult struct {
	Snapshot   data.Snapshot
	Checkpoint *data.CheckpointCache
	Time       int64
	Random     []int64
}
type RecoveryBuilder func(context.Context, RecoveryContext) (RecoveryResult, error)

func (f *SessionFactory) Recover(ctx context.Context, r SessionRequest, build RecoveryBuilder) (*InstalledSession, error) {
	if build == nil {
		return nil, ErrPolicy
	}
	return f.open(ctx, r, true, build)
}
func recoveryContext(g *store.Graph, b data.Binding, h *HostContract, runtime RuntimeConfig) (RecoveryContext, error) {
	o := RecoveryContext{Graph: g, Binding: b, Seed: eventstore.Copy(h.State.Seed), EventSchemas: map[string]hostapi.Schema{}, Namespaces: map[string]hostapi.Schema{}, IntentSchemas: map[string]hostapi.Schema{}}
	packages := graphPackages(g.Root(), g.Dependencies())
	bind := func(r SchemaReference) (hostapi.Schema, error) {
		return hostapi.BindSchema(packages[r.PackageID], r.Path, r.Digest, r.Seed)
	}
	var err error
	if o.StateSchema, err = bind(h.State); err != nil {
		return o, err
	}
	if o.ResultSchema, err = bind(h.Result); err != nil {
		return o, err
	}
	c := h.Result
	if ref, ok := h.Results["create_checkpoint"]; ok {
		c = ref
	}
	if o.CheckpointSchema, err = bind(c); err != nil {
		return o, err
	}
	for name, ref := range h.Events {
		s, e := bind(ref)
		if e != nil {
			return o, e
		}
		o.EventSchemas[name] = s
	}
	for name, ref := range h.Intents {
		s, e := bind(ref)
		if e != nil {
			return o, e
		}
		o.IntentSchemas[name] = s
	}
	for name, ref := range h.Namespaces {
		s, e := bind(ref.Schema)
		if e != nil {
			return o, e
		}
		o.Namespaces[name] = s
	}
	a := g.Artifacts()
	sort.Slice(a, func(i, j int) bool { return a[i].PackageID < a[j].PackageID })
	raw, err := json.Marshal(a)
	if err != nil {
		return o, err
	}
	hashes := map[string]string{}
	for _, a := range a {
		hashes[a.PackageID] = a.ContentHash
	}
	versions := map[string]struct {
		Version uint64
		Hash    string
	}{}
	for name, s := range o.EventSchemas {
		versions[name] = struct {
			Version uint64
			Hash    string
		}{1, s.Digest()}
	}
	o.Metadata = checkpoint.RecoveryBinding{Session: checkpoint.Binding{SessionID: b.Session, StateVersion: 1, PackageHashes: hashes, DependencyLock: b.GraphHash, LuaProfile: profile.ID, RuntimeVersion: profile.RuntimeVersion}, Workspace: b.Workspace, ArtifactsHash: checkpoint.Hash(raw), StateSchema: o.StateSchema.Digest(), EventSchemas: eventstore.Digest(versions), CheckpointSchema: o.CheckpointSchema.Digest(), RunnerHash: runtime.SHA256}
	if o.Metadata.Validate() != nil {
		return o, ErrPolicy
	}
	if o.Lock, err = migration.ExactLock(g); err != nil {
		return o, err
	}
	_, o.SafeBoundaryDeclared = h.Results["on_safe_migration_boundary"]
	o.QuantityPackages = map[string]bool{}
	for _, n := range h.Named {
		if n.Plan == "quantity-add" || n.Plan == "quantity-get" {
			o.QuantityPackages[n.PackageID] = true
		}
	}
	return o, nil
}
func (o RecoveryContext) ValidateRecord(r data.EffectRecord) error {
	if len(o.Epochs) > 0 {
		source, ok := o.Epochs[r.Header.Binding.GraphHash]
		if !ok {
			return eventstore.ErrHistory
		}
		source.Epochs = o.Epochs
		o = source
	}
	if eventstore.Validate(r) != nil || r.Header.Binding != o.Binding || r.SchemaHash != o.StateSchema.Digest() {
		return eventstore.ErrHistory
	}
	target := o
	if r.Migration != nil {
		var ok bool
		target, ok = o.Epochs[r.Migration.To.GraphHash]
		if !ok || !reflect.DeepEqual(o.Lock, r.Migration.From) || !reflect.DeepEqual(target.Lock, r.Migration.To) {
			return eventstore.ErrHistory
		}
		before, after := eventstore.Copy(o.Metadata), eventstore.Copy(target.Metadata)
		before.Session.StateVersion = r.Header.ExpectedVersion
		after.Session.StateVersion = r.Version
		if !reflect.DeepEqual(before, r.Migration.Before) || !reflect.DeepEqual(after, r.Migration.After) {
			return eventstore.ErrHistory
		}
	}
	for _, v := range r.Inputs.ToolResults {
		if o.ResultSchema.Validate(v) != nil {
			return eventstore.ErrHistory
		}
	}
	for _, e := range r.Events {
		s, ok := o.EventSchemas[e.Type]
		if !ok || e.SchemaVersion != 1 || e.SchemaHash != s.Digest() || s.Validate(e.Payload) != nil {
			return eventstore.ErrHistory
		}
	}
	for _, row := range r.Rows {
		context := target
		if row.Deleted {
			context = o
		}
		s, ok := context.Namespaces[row.PackageID+"/"+row.Namespace]
		if !ok || row.SchemaHash != s.Digest() || (!row.Deleted && s.Validate(row.Value) != nil) {
			return eventstore.ErrHistory
		}
	}
	for _, q := range r.Quantities {
		context := target
		if q.Deleted {
			context = o
		}
		if r.Migration != nil && !context.QuantityPackages[q.PackageID] {
			return eventstore.ErrHistory
		}
	}
	tasks := map[string]data.Intent{}
	for _, i := range r.Tasks {
		schema, ok := o.IntentSchemas[i.PackageID+"/"+i.Kind]
		if !ok || (i.Kind != "task" && i.Kind != "ai") || schema.Validate(i.Payload) != nil {
			return eventstore.ErrHistory
		}
		tasks[i.ID] = i
	}
	for _, i := range r.Continuations {
		schema, ok := o.IntentSchemas[i.PackageID+"/continuation"]
		task, found := tasks[i.Payload.Table["task"].String]
		if !ok || i.Kind != "continuation" || i.Payload.Kind != "table" || len(i.Payload.Table) != 2 || i.Payload.Table["task"].Kind != "string" || !found || task.PackageID != i.PackageID || task.Kind != "task" || schema.Validate(i.Payload.Table["value"]) != nil {
			return eventstore.ErrHistory
		}
	}
	for _, i := range r.Outbox {
		if i.Kind == "session-notify" {
			if o.Graph == nil || i.PackageID != string(o.Graph.Root().ExactLock().Root()) || i.Payload.Kind != "table" || len(i.Payload.Table) != 1 || i.Payload.Table["command_id"].Kind != "string" || i.Payload.Table["command_id"].String != r.Header.CommandID {
				return eventstore.ErrHistory
			}
			continue
		}
		task, found := tasks[i.Payload.Table["task"].String]
		if !found || i.Payload.Kind != "table" || len(i.Payload.Table) != 1 || i.Payload.Table["task"].Kind != "string" || task.PackageID != i.PackageID || i.Kind != "dispatch-"+task.Kind {
			return eventstore.ErrHistory
		}
	}

	return nil
}

// ValidateSnapshot checks the entire target image, including unchanged rows.
// A changed namespace cannot inherit old values merely because patches pass.
func (o RecoveryContext) ValidateSnapshot(s data.Snapshot) error {
	if s.Binding != o.Binding || s.SchemaHash != o.StateSchema.Digest() || o.StateSchema.Validate(s.State) != nil || len(s.Rows) > 128 || len(s.Quantities) > 128 {
		return eventstore.ErrHistory
	}
	rows, quantities := map[string]bool{}, map[string]bool{}
	for _, v := range s.Rows {
		k := v.PackageID + "/" + v.Namespace + "/" + v.Key
		schema, ok := o.Namespaces[v.PackageID+"/"+v.Namespace]
		if rows[k] || !ok || v.Deleted || v.SchemaHash != schema.Digest() || schema.Validate(v.Value) != nil {
			return eventstore.ErrHistory
		}
		rows[k] = true
	}
	for _, v := range s.Quantities {
		k := v.PackageID + "/" + v.Table + "/" + v.Key
		if quantities[k] || v.Deleted || v.Table != "quantity" || (o.Graph != nil && !o.QuantityPackages[v.PackageID]) {
			return eventstore.ErrHistory
		}
		quantities[k] = true
	}
	return nil
}
