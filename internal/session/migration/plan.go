// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// Package migration provides the trusted, data-only M1 upgrade operator. Plans
// cannot supply scripts, SQL, DDL, dependency ranges or an inverse migration.
package migration

import (
	"github.com/zyc14588/TRPG_PLATFORM/internal/eventstore"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/install"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/model"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
	"strings"
)

type FieldMove struct{ From, To string }
type NamespaceMove struct {
	PackageID, From, To string
	Fields              []FieldMove
	Defaults            map[string]checkpoint.Value
}
type Plan struct {
	ID               string
	FromLock, ToLock string
	StateFields      []FieldMove
	StateDefaults    map[string]checkpoint.Value
	Namespaces       []NamespaceMove
}

func validField(s string) bool { return s != "" && len(s) <= 256 && !strings.HasPrefix(s, "cap:") }
func transform(v checkpoint.Value, moves []FieldMove, defaults map[string]checkpoint.Value) (checkpoint.Value, error) {
	if v.Kind != "table" || len(moves) > 64 || len(defaults) > 64 {
		return checkpoint.Value{}, data.ErrDenied
	}
	out := eventstore.Copy(v)
	seen := map[string]bool{}
	for _, m := range moves {
		if !validField(m.From) || (m.To != "" && !validField(m.To)) || seen[m.From] {
			return checkpoint.Value{}, data.ErrDenied
		}
		seen[m.From] = true
		value, ok := out.Table[m.From]
		if !ok {
			return checkpoint.Value{}, data.ErrDenied
		}
		if m.To != m.From {
			if _, exists := out.Table[m.To]; m.To != "" && exists {
				return checkpoint.Value{}, data.ErrDenied
			}
			delete(out.Table, m.From)
			if m.To != "" {
				out.Table[m.To] = value
			}
		}
	}
	for k, value := range defaults {
		if !validField(k) || checkpoint.Validate(value) != nil {
			return checkpoint.Value{}, data.ErrDenied
		}
		if _, exists := out.Table[k]; exists {
			return checkpoint.Value{}, data.ErrDenied
		}
		out.Table[k] = eventstore.Copy(value)
	}
	if checkpoint.Validate(out) != nil {
		return checkpoint.Value{}, data.ErrDenied
	}
	return out, nil
}
func (p Plan) Apply(before data.Snapshot, target install.RecoveryContext) (data.Snapshot, error) {
	if !store.ValidID(p.ID) || p.FromLock != before.Binding.GraphHash || p.ToLock != target.Binding.GraphHash || p.FromLock == p.ToLock || len(p.Namespaces) > 128 {
		return data.Snapshot{}, data.ErrDenied
	}
	out := eventstore.Copy(before)
	out.Binding = target.Binding
	out.Version++
	out.SchemaHash = target.StateSchema.Digest()
	var err error
	if out.State, err = transform(out.State, p.StateFields, p.StateDefaults); err != nil {
		return out, err
	}
	rules := map[string]NamespaceMove{}
	for _, n := range p.Namespaces {
		if _, err = model.ParsePackageID(n.PackageID); err != nil || !store.ValidID(n.From) || n.To != "" && !store.ValidID(n.To) {
			return out, data.ErrDenied
		}
		key := n.PackageID + "/" + n.From
		if _, ok := rules[key]; ok {
			return out, data.ErrDenied
		}
		rules[key] = n
	}
	out.Rows = nil
	for _, v := range before.Rows {
		if n, ok := rules[v.PackageID+"/"+v.Namespace]; ok {
			if n.To == "" {
				continue
			}
			v.Namespace = n.To
			if v.Value, err = transform(v.Value, n.Fields, n.Defaults); err != nil {
				return out, err
			}
		}
		s, ok := target.Namespaces[v.PackageID+"/"+v.Namespace]
		if !ok {
			return out, data.ErrDenied
		}
		v.SchemaHash = s.Digest()
		out.Rows = append(out.Rows, v)
	}
	if target.ValidateSnapshot(out) != nil {
		return out, eventstore.ErrHistory
	}
	// The common reducer enforces whole-image row/quantity and byte limits.
	return out, nil
}
