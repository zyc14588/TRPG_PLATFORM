// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package packagedata

import "github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"

// Creation is immutable approved graph/seed evidence, written in the original
// installation transaction. The mutable current state is never used as genesis.
type Creation struct {
	Binding       Binding          `json:"binding"`
	Version       uint64           `json:"version"`
	SchemaHash    string           `json:"schema_hash"`
	Seed          checkpoint.Value `json:"seed"`
	SeedHash      string           `json:"seed_hash"`
	ArtifactsHash string           `json:"artifacts_hash"`
}
type JournalEvent struct {
	Sequence  uint64 `json:"sequence"`
	Version   uint64 `json:"version"`
	CommandID string `json:"command_id"`
	Event     Event  `json:"event"`
}
type JournalPage struct {
	Binding   Binding        `json:"binding"`
	Version   uint64         `json:"version"`
	Cursor    uint64         `json:"cursor"`
	Events    []JournalEvent `json:"events"`
	Truncated bool           `json:"truncated"`
	Ended     bool           `json:"ended,omitempty"`
}

// CheckpointCache is derived recovery data. Its digest is integrity evidence,
// not authority to replace the immutable creation/event history.
type CheckpointCache struct {
	Binding          Binding          `json:"binding"`
	Version          uint64           `json:"version"`
	Cursor           uint64           `json:"cursor"`
	StateSchema      string           `json:"state_schema"`
	CheckpointSchema string           `json:"checkpoint_schema"`
	Value            checkpoint.Value `json:"value"`
	Hash             string           `json:"hash"`
}
