// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// Package eventstore reads immutable, bounded event effects. It has no runtime,
// clock, random source, model client, dispatcher, or network dependency.
package eventstore

import (
	"encoding/json"
	"errors"
	"math"
	"strings"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/model"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
)

const MaxRecordBytes = 1 << 20
const MaxRecords = 256
const MaxHistoryBytes = 4 << 20

var ErrHistory = errors.New("REPLAY_HISTORY_REJECTED")

func Copy[T any](v T) T {
	raw, _ := json.Marshal(v)
	var out T
	_ = json.Unmarshal(raw, &out)
	return out
}
func Digest(v any) string { raw, _ := json.Marshal(v); return checkpoint.Hash(raw) }

func Record(s data.Snapshot, c data.Commit, cursor uint64) (data.EffectRecord, error) {
	r := data.EffectRecord{Format: 1, Header: c.Header, Version: c.Header.ExpectedVersion + 1, BeforeCursor: cursor, Cursor: cursor + uint64(len(c.Events)), BeforeStateHash: Digest(s.State), StateHash: Digest(c.State), SchemaHash: c.SchemaHash, Patches: c.Patches, Rows: c.Rows, Quantities: c.Quantities, Events: c.Events, Tasks: c.Tasks, Continuations: c.Continuations, Outbox: c.Outbox, Inputs: c.Inputs, Result: c.Result}
	r.Ended = c.Inputs.Envelope != nil && c.Inputs.Envelope.Type == "end" && c.Inputs.Callback == "on_session_end"
	// Old direct repository clients may have incomplete patch/schema evidence.
	// Persist its actual qualification; never fill missing facts from a cache.
	r.Complete = validate(r) == nil
	if r.Complete {
		state, err := ApplyPatches(s.State, r)
		r.Complete = err == nil && Digest(state) == r.StateHash
	}
	r.Hash = ""
	r.Hash = Digest(r)
	r = Copy(r)
	raw, err := json.Marshal(r)
	if err != nil || len(raw) > MaxRecordBytes {
		return data.EffectRecord{}, ErrHistory
	}
	return r, nil
}
func Validate(r data.EffectRecord) error {
	if !r.Complete || !checkpoint.IsDigest(r.Hash) {
		return ErrHistory
	}
	h := r.Hash
	r.Hash = ""
	if Digest(r) != h || validate(r) != nil {
		return ErrHistory
	}
	return nil
}
func Decode(raw []byte) (data.EffectRecord, error) {
	var r data.EffectRecord
	if checkpoint.StrictDecode(raw, &r, MaxRecordBytes) != nil || Validate(r) != nil {
		return data.EffectRecord{}, ErrHistory
	}
	return r, nil
}
func validate(r data.EffectRecord) error {
	h := r.Header
	if r.Format != 1 || h.ReadOnly || !store.ValidID(h.Binding.Workspace) || !store.ValidID(h.Binding.Session) || !checkpoint.IsDigest(h.Binding.GraphHash) || !store.ValidID(h.Principal) || !store.ValidID(h.CommandID) || !checkpoint.IsDigest(h.Fingerprint) || h.ExpectedVersion == 0 || h.ExpectedVersion >= math.MaxInt64-1 || r.Version != h.ExpectedVersion+1 || r.BeforeCursor >= math.MaxInt64 || r.Cursor >= math.MaxInt64 || r.Cursor != r.BeforeCursor+uint64(len(r.Events)) || !checkpoint.IsDigest(r.BeforeStateHash) || !checkpoint.IsDigest(r.StateHash) || !checkpoint.IsDigest(r.SchemaHash) {
		return ErrHistory
	}
	if len(r.Events) > 64 || len(r.Patches) > 128 || len(r.Rows)+len(r.Quantities) > 128 || len(r.Tasks) > 32 || len(r.Continuations) > 32 || len(r.Outbox) > 64 || len(r.Inputs.Random) > 256 || len(r.Inputs.ToolResults) > 32 || r.Inputs.Time < 0 || r.Inputs.Callback == "" || len(r.Inputs.Callback) > 128 || checkpoint.Validate(r.Inputs.Command) != nil || checkpoint.Validate(r.Result) != nil {
		return ErrHistory
	}
	for _, v := range r.Inputs.ToolResults {
		if checkpoint.Validate(v) != nil {
			return ErrHistory
		}
	}
	if m := r.Inputs.Envelope; m != nil && (!store.ValidID(m.Seat) || !store.ValidID(m.Type) || !store.ValidID(m.Correlation)) {
		return ErrHistory
	}
	eventID := ""
	seen := map[string]bool{}
	if r.Migration != nil {
		m := r.Migration
		if !store.ValidID(m.PointID) || !checkpoint.IsDigest(m.PointHash) || (m.Direction != "upgrade" && m.Direction != "restore-point") || data.ValidateSessionLock(m.From) != nil || data.ValidateSessionLock(m.To) != nil || m.From.Hash == m.To.Hash || m.Before.Validate() != nil || m.After.Validate() != nil || m.Before.Workspace != h.Binding.Workspace || m.After.Workspace != h.Binding.Workspace || m.Before.Session.SessionID != h.Binding.Session || m.After.Session.SessionID != h.Binding.Session || m.Before.Session.DependencyLock != h.Binding.GraphHash || m.From.GraphHash != h.Binding.GraphHash || m.After.Session.DependencyLock != m.To.GraphHash || m.Before.Session.StateVersion != h.ExpectedVersion || m.After.Session.StateVersion != r.Version || m.Before.StateSchema != r.SchemaHash || m.Before.ArtifactsHash != checkpoint.Hash(m.From.ArtifactSet) || m.After.ArtifactsHash != checkpoint.Hash(m.To.ArtifactSet) || len(r.Events)+len(r.Tasks)+len(r.Continuations)+len(r.Outbox) != 0 || r.Inputs.Callback != "platform-migration" || r.Inputs.Envelope != nil || r.Ended != m.WasEnded {
			return ErrHistory
		}
		// A private transition has a command-ledger identity, not a new public
		// event type or a repurposed package event. Old event cursors are preserved.
		eventID = h.CommandID
	}
	for _, e := range r.Events {
		if eventID == "" {
			eventID = e.ID
		}
		if !store.ValidID(e.ID) || seen[e.ID] || len(e.Type) > 256 || !strings.Contains(e.Type, "/") || e.SchemaVersion == 0 || e.SchemaVersion > 1024 || !checkpoint.IsDigest(e.SchemaHash) || checkpoint.Validate(e.Payload) != nil {
			return ErrHistory
		}
		seen[e.ID] = true
	}
	if len(r.Patches)+len(r.Rows)+len(r.Quantities)+len(r.Tasks)+len(r.Continuations)+len(r.Outbox) > 0 && eventID == "" {
		return ErrHistory
	}
	for _, p := range r.Patches {
		if p.CommandID != h.CommandID || p.EventID != eventID || len(p.Path) > 32 || p.Before == nil || p.After == nil || checkpoint.Validate(*p.Before) != nil || checkpoint.Validate(*p.After) != nil || Digest(*p.Before) != p.BeforeHash || Digest(*p.After) != p.AfterHash || (p.Delete && (p.After.Kind != "nil" || len(p.Path) == 0)) {
			return ErrHistory
		}
		for _, k := range p.Path {
			if k == "" || len(k) > 256 || strings.HasPrefix(k, "cap:") {
				return ErrHistory
			}
		}
	}
	rows := map[string]bool{}
	for _, v := range r.Rows {
		k := v.PackageID + "/" + v.Namespace + "/" + v.Key
		_, e := model.ParsePackageID(v.PackageID)
		if e != nil || !store.ValidID(v.Namespace) || !store.ValidID(v.Key) || rows[k] || !checkpoint.IsDigest(v.SchemaHash) || checkpoint.Validate(v.Value) != nil {
			return ErrHistory
		}
		rows[k] = true
	}
	quantities := map[string]bool{}
	for _, v := range r.Quantities {
		k := v.PackageID + "/" + v.Table + "/" + v.Key
		_, e := model.ParsePackageID(v.PackageID)
		if e != nil || v.Table != "quantity" || !store.ValidID(v.Key) || quantities[k] || (v.Deleted && r.Migration == nil) {
			return ErrHistory
		}
		quantities[k] = true
	}
	for _, list := range [][]data.Intent{r.Tasks, r.Continuations, r.Outbox} {
		ids := map[string]bool{}
		for _, v := range list {
			_, e := model.ParsePackageID(v.PackageID)
			if e != nil || !store.ValidID(v.ID) || ids[v.ID] || len(v.Kind) == 0 || len(v.Kind) > 128 || checkpoint.Validate(v.Payload) != nil {
				return ErrHistory
			}
			ids[v.ID] = true
		}
	}
	if r.Ended && r.Migration == nil && (eventID == "" || r.Inputs.Envelope == nil || r.Inputs.Envelope.Type != "end" || r.Inputs.Callback != "on_session_end") {
		return ErrHistory
	}
	return nil
}

// State patches use the same explicit string-key table boundary as Host state.
// Before facts are checked at every step; no serialized current state is used.
func ApplyPatches(initial checkpoint.Value, r data.EffectRecord) (checkpoint.Value, error) {
	state := Copy(initial)
	if checkpoint.Validate(state) != nil || Digest(state) != r.BeforeStateHash {
		return checkpoint.Value{}, ErrHistory
	}
	for _, p := range r.Patches {
		if p.Before == nil || p.After == nil || len(p.Path) > 32 {
			return checkpoint.Value{}, ErrHistory
		}
		if len(p.Path) == 0 {
			if p.Delete || Digest(state) != p.BeforeHash || Digest(*p.Before) != p.BeforeHash || Digest(*p.After) != p.AfterHash {
				return checkpoint.Value{}, ErrHistory
			}
			state = Copy(*p.After)
			continue
		}
		node := state
		for _, k := range p.Path[:len(p.Path)-1] {
			if node.Kind != "table" {
				return checkpoint.Value{}, ErrHistory
			}
			next, ok := node.Table[k]
			if !ok {
				return checkpoint.Value{}, ErrHistory
			}
			node = next
		}
		if node.Kind != "table" || node.Table == nil {
			return checkpoint.Value{}, ErrHistory
		}
		key := p.Path[len(p.Path)-1]
		before, ok := node.Table[key]
		if !ok {
			before = checkpoint.Value{Kind: "nil"}
		}
		if Digest(before) != p.BeforeHash || Digest(*p.Before) != p.BeforeHash || Digest(*p.After) != p.AfterHash {
			return checkpoint.Value{}, ErrHistory
		}
		if p.Delete {
			delete(node.Table, key)
		} else {
			node.Table[key] = Copy(*p.After)
		}
	}
	if checkpoint.Validate(state) != nil || Digest(state) != r.StateHash {
		return checkpoint.Value{}, ErrHistory
	}
	return state, nil
}
