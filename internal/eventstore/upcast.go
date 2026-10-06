// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package eventstore

import (
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
)

// Upcast is a bounded declarative reader adaptation, never a history update or
// callback. The original ID/type/payload/version/hash are returned untouched.
type Upcast struct {
	Type       string
	From, To   uint64
	SchemaHash string
	Rename     map[string]string
}
type ReadEvent struct {
	Original   data.Event
	Value      checkpoint.Value
	Version    uint64
	SchemaHash string
}

func Read(e data.Event, target uint64, rules []Upcast) (ReadEvent, error) {
	if e.SchemaVersion == 0 || target < e.SchemaVersion || target > 1024 || len(rules) > 16 || checkpoint.Validate(e.Payload) != nil || !checkpoint.IsDigest(e.SchemaHash) {
		return ReadEvent{}, ErrHistory
	}
	out := ReadEvent{Original: Copy(e), Value: Copy(e.Payload), Version: e.SchemaVersion, SchemaHash: e.SchemaHash}
	for steps := 0; out.Version < target; steps++ {
		if steps >= 16 {
			return ReadEvent{}, ErrHistory
		}
		var selected *Upcast
		for n := range rules {
			r := &rules[n]
			if r.Type == e.Type && r.From == out.Version {
				if selected != nil {
					return ReadEvent{}, ErrHistory
				}
				selected = r
			}
		}
		if selected == nil || selected.To != out.Version+1 || selected.To > target || !checkpoint.IsDigest(selected.SchemaHash) || len(selected.Rename) > 32 || out.Value.Kind != "table" {
			return ReadEvent{}, ErrHistory
		}
		before := Copy(out.Value.Table)
		next := Copy(before)
		targets := map[string]bool{}
		for old, newKey := range selected.Rename {
			value, ok := before[old]
			if !ok || old == newKey || old == "" || newKey == "" || len(old) > 256 || len(newKey) > 256 || targets[newKey] {
				return ReadEvent{}, ErrHistory
			}
			if _, exists := before[newKey]; exists {
				return ReadEvent{}, ErrHistory
			}
			targets[newKey] = true
			delete(next, old)
			next[newKey] = value
		}
		out.Value = checkpoint.Object(next)
		out.Version = selected.To
		out.SchemaHash = selected.SchemaHash
		if checkpoint.Validate(out.Value) != nil {
			return ReadEvent{}, ErrHistory
		}
	}
	return out, nil
}
