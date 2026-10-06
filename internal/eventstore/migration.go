// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package eventstore

import (
	"encoding/json"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
	"sort"
)

// RecordMigration is a private Go operator seam. Ordinary Commit and Lua Host
// callbacks have no transition field and cannot invoke it.
func RecordMigration(before, after data.Snapshot, header data.Header, cursor uint64, m data.MigrationTransition) (data.EffectRecord, error) {
	if header.Binding != before.Binding || header.ExpectedVersion != before.Version || after.Binding.Workspace != before.Binding.Workspace || after.Binding.Session != before.Binding.Session || after.Binding.GraphHash != m.To.GraphHash || after.SchemaHash != m.After.StateSchema || after.Version != before.Version+1 {
		return data.EffectRecord{}, ErrHistory
	}
	r := data.EffectRecord{Format: 1, Header: header, Version: after.Version, BeforeCursor: cursor, Cursor: cursor, BeforeStateHash: Digest(before.State), StateHash: Digest(after.State), SchemaHash: before.SchemaHash, Inputs: data.Inputs{Callback: "platform-migration", Command: after.State}, Result: after.State, Migration: &m, Ended: m.WasEnded}
	first, last := Copy(before.State), Copy(after.State)
	r.Patches = []data.Patch{{Before: &first, After: &last, BeforeHash: r.BeforeStateHash, AfterHash: r.StateHash, CommandID: header.CommandID, EventID: header.CommandID, Module: "platform-migration"}}
	rows := map[string]data.Row{}
	for _, v := range before.Rows {
		rows[v.PackageID+"/"+v.Namespace+"/"+v.Key] = v
	}
	for _, v := range after.Rows {
		k := v.PackageID + "/" + v.Namespace + "/" + v.Key
		if _, ok := rows[k]; ok {
			delete(rows, k)
		}
		r.Rows = append(r.Rows, v)
	}
	// Sort deletion keys: map iteration must never influence an immutable record.
	for _, k := range sortedKeys(rows) {
		v := rows[k]
		v.Deleted = true
		r.Rows = append(r.Rows, v)
	}
	qs := map[string]data.Quantity{}
	for _, v := range before.Quantities {
		qs[v.PackageID+"/"+v.Table+"/"+v.Key] = v
	}
	for _, v := range after.Quantities {
		k := v.PackageID + "/" + v.Table + "/" + v.Key
		delete(qs, k)
		r.Quantities = append(r.Quantities, v)
	}
	for _, k := range sortedKeys(qs) {
		v := qs[k]
		v.Deleted = true
		r.Quantities = append(r.Quantities, v)
	}
	r.Complete = true
	r.Hash = Digest(r)
	raw, err := json.Marshal(r)
	if err != nil || len(raw) > MaxRecordBytes || Validate(r) != nil {
		return data.EffectRecord{}, ErrHistory
	}
	state, err := ApplyPatches(before.State, r)
	if err != nil || Digest(state) != r.StateHash {
		return data.EffectRecord{}, ErrHistory
	}
	return Copy(r), nil
}

func sortedKeys[T any](m map[string]T) []string {
	keys := make([]string, 0, len(m))
	for k := range m {
		keys = append(keys, k)
	}
	sort.Strings(keys)
	return keys
}
