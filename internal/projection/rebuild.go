// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// Package projection deterministically reduces recorded Go-approved effects.
// It cannot execute Lua, dispatch an intent, or obtain an external input.
package projection

import (
	"encoding/json"
	"reflect"
	"sort"

	"github.com/zyc14588/TRPG_PLATFORM/internal/eventstore"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
)

type Image struct {
	Binding     data.Binding     `json:"binding"`
	Version     uint64           `json:"version"`
	Cursor      uint64           `json:"cursor"`
	StateSchema string           `json:"state_schema"`
	State       checkpoint.Value `json:"state"`
	Rows        []data.Row       `json:"rows"`
	Quantities  []data.Quantity  `json:"quantities"`
	HistoryHash string           `json:"history_hash"`
	Ended       bool             `json:"ended,omitempty"`
}
type Cache struct {
	Metadata checkpoint.RecoveryBinding `json:"metadata"`
	Image    Image                      `json:"image"`
	Hash     string                     `json:"hash"`
}

func Genesis(c data.Creation) (Image, error) {
	if c.Version != 1 || !checkpoint.IsDigest(c.Binding.GraphHash) || checkpoint.Validate(c.Seed) != nil || c.SeedHash != eventstore.Digest(c.Seed) || !checkpoint.IsDigest(c.SchemaHash) || !checkpoint.IsDigest(c.ArtifactsHash) {
		return Image{}, eventstore.ErrHistory
	}
	return Image{Binding: c.Binding, Version: c.Version, StateSchema: c.SchemaHash, State: eventstore.Copy(c.Seed), HistoryHash: eventstore.Digest(c)}, nil
}
func Apply(i Image, r data.EffectRecord) (Image, error) {
	if eventstore.Validate(r) != nil || i.Ended || r.Header.Binding != i.Binding || r.Header.ExpectedVersion != i.Version || r.BeforeCursor != i.Cursor || r.SchemaHash != i.StateSchema {
		return Image{}, eventstore.ErrHistory
	}
	state, err := eventstore.ApplyPatches(i.State, r)
	if err != nil {
		return Image{}, err
	}
	rows := map[string]data.Row{}
	qs := map[string]data.Quantity{}
	for _, v := range i.Rows {
		rows[rowKey(v)] = v
	}
	for _, v := range i.Quantities {
		qs[quantityKey(v)] = v
	}
	for _, v := range r.Rows {
		if v.Deleted {
			delete(rows, rowKey(v))
		} else {
			rows[rowKey(v)] = eventstore.Copy(v)
		}
	}
	for _, v := range r.Quantities {
		qs[quantityKey(v)] = v
	}
	out := Image{Binding: i.Binding, Version: r.Version, Cursor: r.Cursor, StateSchema: i.StateSchema, State: state, HistoryHash: eventstore.Digest([]string{i.HistoryHash, r.Hash}), Ended: r.Ended}
	keys := make([]string, 0, len(rows))
	for k := range rows {
		keys = append(keys, k)
	}
	sort.Strings(keys)
	size := 0
	for _, k := range keys {
		v := rows[k]
		raw, _ := json.Marshal(v)
		size += len(raw)
		out.Rows = append(out.Rows, v)
	}
	keys = keys[:0]
	for k := range qs {
		keys = append(keys, k)
	}
	sort.Strings(keys)
	for _, k := range keys {
		out.Quantities = append(out.Quantities, qs[k])
	}
	if len(out.Rows) > 128 || len(out.Quantities) > 128 || size > 256<<10 {
		return Image{}, eventstore.ErrHistory
	}
	return out, nil
}
func rowKey(v data.Row) string           { return v.PackageID + "/" + v.Namespace + "/" + v.Key }
func quantityKey(v data.Quantity) string { return v.PackageID + "/" + v.Table + "/" + v.Key }
func Seal(m checkpoint.RecoveryBinding, i Image) (Cache, error) {
	m = eventstore.Copy(m)
	m.Session.StateVersion = i.Version
	if m.Validate() != nil || m.Session.SessionID != i.Binding.Session || m.Workspace != i.Binding.Workspace || m.Session.DependencyLock != i.Binding.GraphHash || m.StateSchema != i.StateSchema {
		return Cache{}, checkpoint.ErrRejected
	}
	c := Cache{Metadata: m, Image: eventstore.Copy(i)}
	c.Hash = eventstore.Digest(c)
	return c, nil
}
func Compatible(c Cache, m checkpoint.RecoveryBinding, prefix Image) bool {
	h := c.Hash
	c.Hash = ""
	m = eventstore.Copy(m)
	m.Session.StateVersion = prefix.Version
	return checkpoint.IsDigest(h) && eventstore.Digest(c) == h && c.Metadata.Validate() == nil && reflect.DeepEqual(c.Metadata, m) && reflect.DeepEqual(c.Image, prefix)
}

// Every supported cache boundary is verified against its immutable prefix. A
// cache's own hash cannot make it authoritative. Rejected caches fall back to
// genesis; the original event effects are always validated in order.
func Rebuild(h data.ReplayHistory, m checkpoint.RecoveryBinding, c *Cache, validate func(data.EffectRecord) error) (Image, bool, error) {
	i, err := Genesis(h.Creation)
	if err != nil || m.Validate() != nil || m.Session.SessionID != i.Binding.Session || m.Workspace != i.Binding.Workspace || m.Session.DependencyLock != i.Binding.GraphHash || h.Creation.ArtifactsHash != m.ArtifactsHash || h.Creation.SchemaHash != m.StateSchema {
		return Image{}, false, eventstore.ErrHistory
	}
	accepted := c != nil && Compatible(*c, m, i)
	seen := map[string]bool{}
	bytes := 0
	if len(h.Records) > eventstore.MaxRecords {
		return Image{}, false, eventstore.ErrHistory
	}
	for _, r := range h.Records {
		raw, _ := json.Marshal(r)
		bytes += len(raw)
		if bytes > eventstore.MaxHistoryBytes {
			return Image{}, false, eventstore.ErrHistory
		}
		if seen[r.Header.CommandID] {
			return Image{}, false, eventstore.ErrHistory
		}
		seen[r.Header.CommandID] = true
		if validate != nil {
			if err = validate(r); err != nil {
				return Image{}, false, err
			}
		}
		i, err = Apply(i, r)
		if err != nil {
			return Image{}, false, err
		}
		if c != nil && c.Image.Version == i.Version && Compatible(*c, m, i) {
			accepted = true
			i = eventstore.Copy(c.Image)
		}
	}
	if i.Version != h.Version || i.Cursor != h.Cursor || i.Ended != h.Ended {
		return Image{}, false, eventstore.ErrHistory
	}
	return i, accepted, nil
}
