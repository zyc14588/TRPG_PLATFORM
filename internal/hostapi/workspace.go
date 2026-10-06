// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package hostapi

import (
	"context"
	"fmt"
	"math"
	"sort"
	"strings"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/vm"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/capability"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
)

type Namespace struct {
	Schema   Schema
	Indices  map[string][]string
	MaxRows  int
	MaxBytes int
}

// NamedOperation selects one of two reviewed integer-table plans implemented in
// Go. It accepts no SQL, identifier interpolation, DDL or connection callback.
type NamedOperation struct {
	PackageID string
	ID        string
	Plan      string
	Input     Schema
	Output    Schema
}
type workspace struct {
	header                                      data.Header
	inputs                                      data.Inputs
	state                                       checkpoint.Value
	modules                                     map[string]vm.ModuleIdentity
	options                                     Options
	rows                                        map[string]data.Row
	quantities                                  map[string]data.Quantity
	changed                                     map[string]data.Row
	quantityChanges                             map[string]data.Quantity
	patches                                     []data.Patch
	events                                      []data.Event
	tasks, continuations, outbox                []data.Intent
	audits                                      []data.Audit
	callbacks, rowsUsed, bytesUsed, randomIndex int
	outputBytes                                 int
	failure                                     error
	readonly                                    bool
}

func rowKey(pkg, namespace, key string) string { return pkg + "\x00" + namespace + "\x00" + key }
func (w *workspace) reject(code string) error {
	if w.failure == nil {
		w.failure = profile.Fail(code)
	}
	return w.failure
}
func (w *workspace) id(kind string, n int) string {
	return kind + "-" + strings.TrimPrefix(digest([]any{w.header, kind, n}), "sha256:")[:32]
}
func (w *workspace) measure(v checkpoint.Value, rows int) error {
	raw, err := nativeJSON(v)
	if err != nil {
		return w.reject(profile.ErrValue)
	}
	w.bytesUsed += len(raw)
	w.rowsUsed += rows
	if w.bytesUsed > w.options.Budget.DataBytes || w.rowsUsed > w.options.Budget.Rows {
		return w.reject(profile.ErrBudget)
	}
	return nil
}
func (w *workspace) call(ctx context.Context, c profile.HostCall) (value checkpoint.Value, err error) {
	value = checkpoint.Value{Kind: "nil"}
	started := time.Now()
	var tables []string
	m, ok := w.modules[c.Module]
	defer func() {
		outcome := profile.Code(err)
		level := w.options.AuditPolicy.Level
		if level == "" {
			level = "AUDIT-1"
		}
		event := ""
		if len(w.events) > 0 {
			event = w.events[len(w.events)-1].ID
		}
		a := data.Audit{Workspace: w.header.Binding.Workspace, Session: w.header.Binding.Session, CommandID: w.header.CommandID, Level: level, Module: c.Module, PackageID: m.PackageID, Operation: c.Capability + "." + c.Operation, Phase: c.Phase, ArgumentsHash: digest(c.Arguments), ResultHash: digest(value), Outcome: outcome, DurationNanos: time.Since(started).Nanoseconds(), StateVersion: w.header.ExpectedVersion, CallbackCount: w.callbacks, Tables: tables, EventID: event}
		w.audits = append(w.audits, a)
		sealedAudit, _ := clone(a)
		if e := w.options.Audit(sealedAudit); e != nil {
			err = w.reject("AUDIT_FAILED")
			value = checkpoint.Value{Kind: "nil"}
		}
	}()
	if w.failure != nil {
		return value, w.failure
	}
	if ctx == nil || ctx.Err() != nil {
		return value, w.reject(profile.ErrBudget)
	}
	if w.options.AuditPolicy.validate(time.Now()) != nil {
		return value, w.reject("AUDIT_POLICY_REJECTED")
	}
	w.callbacks++
	if w.callbacks > w.options.Budget.Callbacks {
		return value, w.reject(profile.ErrBudget)
	}
	cap, e := capability.ParseName(c.Capability)
	if !ok || e != nil || !m.Capabilities.Contains(cap) || !profile.KnownHostOperation(c.Capability, c.Operation) || c.Line < 1 {
		return value, w.reject(profile.ErrCapability)
	}
	for _, x := range c.Arguments {
		if checkpoint.Validate(x) != nil {
			return value, w.reject(profile.ErrValue)
		}
	}
	write := c.Capability == "host.state" && c.Operation != "get" || c.Capability == "host.event" || c.Capability == "host.task" || c.Capability == "host.ai" || c.Capability == "host.db" && (c.Operation == "put" || c.Operation == "delete" || c.Operation == "compare_and_set")
	if write && (w.readonly || c.Phase != "execute") {
		return value, w.reject("VALIDATION_EFFECT_DENIED")
	}
	if w.options.Fault != nil {
		if err = w.options.Fault(ctx, "callback:"+c.Capability+"."+c.Operation); err != nil {
			return value, w.reject(profile.Code(err))
		}
	}
	args := c.Arguments
	switch c.Capability {
	case "host.state":
		if len(args) < 1 || args[0].Kind != "array" {
			return value, w.reject(profile.ErrValue)
		}
		path := []string{}
		for _, v := range args[0].Array {
			if v.Kind != "string" || v.String == "" || len(path) >= 32 {
				return value, w.reject(profile.ErrValue)
			}
			path = append(path, v.String)
		}
		old, exists := lookup(w.state, path)
		if c.Operation == "get" {
			if len(args) != 1 {
				return value, w.reject(profile.ErrValue)
			}
			if exists {
				value = old
			}
			return value, w.measure(value, 0)
		}
		if (c.Operation == "put" && len(args) != 2) || (c.Operation == "delete" && len(args) != 1) {
			return value, w.reject(profile.ErrValue)
		}
		next := checkpoint.Value{Kind: "nil"}
		if c.Operation == "put" {
			next = args[1]
		}
		if e := assign(&w.state, path, next, c.Operation == "delete"); e != nil {
			return value, w.reject(profile.ErrValue)
		}
		before, _ := clone(old)
		after, _ := clone(next)
		w.patches = append(w.patches, data.Patch{Path: path, BeforeHash: digest(old), AfterHash: digest(next), Module: c.Module, Line: c.Line, CommandID: w.header.CommandID, Before: &before, After: &after, Delete: c.Operation == "delete"})
		if len(w.patches) > w.options.Budget.Patches {
			return value, w.reject(profile.ErrBudget)
		}
		return value, w.measure(next, 0)
	case "host.event":
		if len(args) != 2 || args[0].Kind != "string" {
			return value, w.reject(profile.ErrValue)
		}
		schema, ok := w.options.EventSchemas[m.PackageID+"/"+args[0].String]
		if !ok || schema.Validate(args[1]) != nil {
			return value, w.reject("SCHEMA_REJECTED")
		}
		event := data.Event{ID: w.id("evt", len(w.events)), Type: m.PackageID + "/" + args[0].String, Payload: args[1], SchemaVersion: 1, SchemaHash: schema.Digest()}
		w.events = append(w.events, event)
		if len(w.events) > w.options.Budget.Events {
			return value, w.reject(profile.ErrBudget)
		}
		return checkpoint.Text(event.ID), w.measure(args[1], 0)
	case "host.random":
		if len(args) != 1 {
			return value, w.reject(profile.ErrValue)
		}
		bound, ok := integer(args[0])
		if !ok || bound < 1 || w.randomIndex >= len(w.inputs.Random) {
			return value, w.reject("RANDOM_INPUT_REJECTED")
		}
		n := w.inputs.Random[w.randomIndex]
		w.randomIndex++
		if n < 0 || n >= bound {
			return value, w.reject("RANDOM_INPUT_REJECTED")
		}
		return checkpoint.Int(n), nil
	case "host.time":
		if len(args) != 0 {
			return value, w.reject(profile.ErrValue)
		}
		return checkpoint.Int(w.inputs.Time), nil
	case "host.content":
		if len(args) != 1 || args[0].Kind != "string" {
			return value, w.reject(profile.ErrValue)
		}
		pkg := w.options.Packages[m.PackageID]
		entry, ok := pkg.Entry(args[0].String)
		if !ok {
			return value, w.reject(profile.ErrCapability)
		}
		value = checkpoint.Text(string(entry.Bytes()))
		return value, w.measure(value, 0)
	case "host.db":
		if c.Operation == "named" {
			value, tables, err = w.named(m, args, c.Phase)
			return value, err
		}
		return w.database(m, args, c.Operation)
	case "host.task", "host.ai":
		kind := "task"
		if c.Capability == "host.ai" {
			kind = "ai"
		}
		if c.Operation == "continuation" {
			kind = "continuation"
		}
		n := 1
		if kind == "continuation" {
			n = 2
		}
		if len(args) != n {
			return value, w.reject(profile.ErrValue)
		}
		payload := args[n-1]
		schema, ok := w.options.IntentSchemas[m.PackageID+"/"+kind]
		if !ok || schema.Validate(payload) != nil {
			return value, w.reject("SCHEMA_REJECTED")
		}
		if kind == "continuation" {
			if args[0].Kind != "string" {
				return value, w.reject(profile.ErrValue)
			}
			found := false
			for _, t := range w.tasks {
				found = found || t.ID == args[0].String && t.PackageID == m.PackageID
			}
			if !found {
				return value, w.reject(profile.ErrCapability)
			}
			id := w.id("cont", len(w.continuations))
			w.continuations = append(w.continuations, data.Intent{ID: id, PackageID: m.PackageID, Kind: kind, Payload: checkpoint.Object(map[string]checkpoint.Value{"task": args[0], "value": payload})})
			if len(w.continuations) > w.options.Budget.Continuations {
				return value, w.reject(profile.ErrBudget)
			}
			return checkpoint.Text(id), w.measure(payload, 0)
		}
		id := w.id("task", len(w.tasks))
		intent := data.Intent{ID: id, PackageID: m.PackageID, Kind: kind, Payload: payload}
		w.tasks = append(w.tasks, intent)
		w.outbox = append(w.outbox, data.Intent{ID: w.id("outbox", len(w.outbox)), PackageID: m.PackageID, Kind: "dispatch-" + kind, Payload: checkpoint.Object(map[string]checkpoint.Value{"task": checkpoint.Text(id)})})
		if len(w.tasks) > w.options.Budget.Tasks || len(w.outbox) > w.options.Budget.Outbox {
			return value, w.reject(profile.ErrBudget)
		}
		return checkpoint.Text(id), w.measure(payload, 0)
	case "host.log":
		if len(args) != 1 {
			return value, w.reject(profile.ErrValue)
		}
		raw, e := nativeJSON(args[0])
		if e != nil {
			return value, w.reject(profile.ErrValue)
		}
		w.outputBytes += len(raw)
		if w.outputBytes > w.options.Budget.OutputBytes {
			return value, w.reject(profile.ErrBudget)
		}
		return value, w.measure(args[0], 0)
	case "host.rules":
		// A reviewed pure integer comparison; no model, network, worker or opaque handle.
		if len(args) != 3 || args[0].Kind != "string" || args[0].String != "integer.compare" {
			return value, w.reject(profile.ErrCapability)
		}
		a, oka := integer(args[1])
		b, okb := integer(args[2])
		if !oka || !okb {
			return value, w.reject(profile.ErrValue)
		}
		n := int64(0)
		if a < b {
			n = -1
		} else if a > b {
			n = 1
		}
		return checkpoint.Int(n), nil
	}
	return value, w.reject(profile.ErrCapability)
}
func lookup(root checkpoint.Value, path []string) (checkpoint.Value, bool) {
	v := root
	for _, p := range path {
		if v.Kind != "table" {
			return checkpoint.Value{}, false
		}
		n, ok := v.Table[p]
		if !ok {
			return checkpoint.Value{}, false
		}
		v = n
	}
	return v, true
}
func assign(root *checkpoint.Value, path []string, v checkpoint.Value, remove bool) error {
	if len(path) == 0 {
		if remove {
			return fmt.Errorf("cannot remove root")
		}
		*root = v
		return nil
	}
	if root.Kind != "table" {
		return fmt.Errorf("not object")
	}
	key := path[0]
	if len(path) == 1 {
		if remove {
			delete(root.Table, key)
		} else {
			root.Table[key] = v
		}
		return nil
	}
	child, ok := root.Table[key]
	if !ok {
		return fmt.Errorf("missing parent")
	}
	if err := assign(&child, path[1:], v, remove); err != nil {
		return err
	}
	root.Table[key] = child
	return nil
}
func (w *workspace) database(m vm.ModuleIdentity, args []checkpoint.Value, op string) (checkpoint.Value, error) {
	zero := checkpoint.Value{Kind: "nil"}
	if len(args) < 1 || args[0].Kind != "string" {
		return zero, w.reject(profile.ErrValue)
	}
	namespace := args[0].String
	spec, ok := w.options.Namespaces[m.PackageID+"/"+namespace]
	if !ok {
		return zero, w.reject(profile.ErrCapability)
	}
	if op == "list" {
		if len(args) != 4 || args[1].Kind != "string" {
			return zero, w.reject(profile.ErrValue)
		}
		index, ok := spec.Indices[args[1].String]
		limit, valid := integer(args[3])
		if !ok || !valid || limit < 1 || limit > int64(spec.MaxRows) {
			return zero, w.reject(profile.ErrCapability)
		}
		rows := []data.Row{}
		for _, r := range w.rows {
			if r.PackageID == m.PackageID && r.Namespace == namespace && !r.Deleted {
				v, exists := lookup(r.Value, index)
				if exists && digest(v) == digest(args[2]) {
					rows = append(rows, r)
				}
			}
		}
		sort.Slice(rows, func(i, j int) bool { return rows[i].Key < rows[j].Key })
		values := []checkpoint.Value{}
		for _, r := range rows {
			if len(values) >= int(limit) {
				break
			}
			v := checkpoint.Object(map[string]checkpoint.Value{"key": checkpoint.Text(r.Key), "value": r.Value})
			if err := w.measure(v, 1); err != nil {
				return zero, err
			}
			values = append(values, v)
		}
		return checkpoint.Array(values...), nil
	}
	if len(args) < 2 || args[1].Kind != "string" || !store.ValidID(args[1].String) {
		return zero, w.reject(profile.ErrCapability)
	}
	key := rowKey(m.PackageID, namespace, args[1].String)
	row, exists := w.rows[key]
	exists = exists && !row.Deleted
	switch op {
	case "get":
		if len(args) != 2 {
			return zero, w.reject(profile.ErrValue)
		}
		if !exists {
			return zero, nil
		}
		return row.Value, w.measure(row.Value, 1)
	case "delete":
		if len(args) != 2 {
			return zero, w.reject(profile.ErrValue)
		}
		row = data.Row{PackageID: m.PackageID, Namespace: namespace, Key: args[1].String, SchemaHash: spec.Schema.digest, Deleted: true, Value: zero}
	case "put", "compare_and_set":
		n := 3
		if op == "compare_and_set" {
			n = 4
		}
		if len(args) != n {
			return zero, w.reject(profile.ErrValue)
		}
		if op == "compare_and_set" {
			old := zero
			if exists {
				old = row.Value
			}
			if digest(old) != digest(args[2]) {
				return checkpoint.Bool(false), nil
			}
		}
		v := args[n-1]
		if spec.Schema.Validate(v) != nil {
			return zero, w.reject("SCHEMA_REJECTED")
		}
		row = data.Row{PackageID: m.PackageID, Namespace: namespace, Key: args[1].String, SchemaHash: spec.Schema.digest, Value: v}
	default:
		return zero, w.reject(profile.ErrCapability)
	}
	w.rows[key] = row
	w.changed[key] = row
	count, size := 0, 0
	for _, r := range w.rows {
		if r.PackageID == m.PackageID && r.Namespace == namespace && !r.Deleted {
			count++
			raw, _ := nativeJSON(r.Value)
			size += len(raw)
		}
	}
	if count > spec.MaxRows || size > spec.MaxBytes {
		return zero, w.reject(profile.ErrBudget)
	}
	if err := w.measure(row.Value, 1); err != nil {
		return zero, err
	}
	if op == "compare_and_set" {
		return checkpoint.Bool(true), nil
	}
	return zero, nil
}
func (w *workspace) named(m vm.ModuleIdentity, args []checkpoint.Value, phase string) (checkpoint.Value, []string, error) {
	zero := checkpoint.Value{Kind: "nil"}
	tables := []string{m.PackageID + "/quantity"}
	if m.Trust != capability.TrustOfficial && m.Trust != capability.TrustSigned {
		return zero, tables, w.reject(profile.ErrCapability)
	}
	if len(args) != 2 || args[0].Kind != "string" {
		return zero, tables, w.reject(profile.ErrValue)
	}
	spec, ok := w.options.NamedOperations[m.PackageID+"/"+args[0].String]
	if !ok || spec.Input.Validate(args[1]) != nil {
		return zero, tables, w.reject("SCHEMA_REJECTED")
	}
	key, ok := args[1].Table["key"]
	if !ok || key.Kind != "string" || !store.ValidID(key.String) {
		return zero, tables, w.reject(profile.ErrValue)
	}
	id := rowKey(m.PackageID, "quantity", key.String)
	q := w.quantities[id]
	q.PackageID = m.PackageID
	q.Table = "quantity"
	q.Key = key.String
	switch spec.Plan {
	case "quantity-get":
	case "quantity-add":
		if w.readonly || phase != "execute" {
			return zero, tables, w.reject("VALIDATION_EFFECT_DENIED")
		}
		delta, ok := integer(args[1].Table["delta"])
		if !ok || delta > 0 && q.Value > math.MaxInt64-delta || delta < 0 && q.Value < math.MinInt64-delta {
			return zero, tables, w.reject(profile.ErrValue)
		}
		q.Value += delta
		w.quantities[id] = q
		w.quantityChanges[id] = q
	default:
		return zero, tables, w.reject(profile.ErrCapability)
	}
	value := checkpoint.Int(q.Value)
	if spec.Output.Validate(value) != nil {
		return zero, tables, w.reject("SCHEMA_REJECTED")
	}
	return value, tables, w.measure(value, 1)
}
