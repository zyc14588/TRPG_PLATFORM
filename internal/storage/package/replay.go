// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package packagedata

import "github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"

// EffectRecord is appended in the original command transaction. Patches carry
// validated before/after facts, not just hashes. Current state/data tables are
// derived; replay never treats them as an initial state. Zero Complete identifies
// old/incomplete evidence and must fail closed in reconstruction.
type EffectRecord struct {
	Format          uint64           `json:"format"`
	Complete        bool             `json:"complete"`
	Header          Header           `json:"header"`
	Version         uint64           `json:"version"`
	BeforeCursor    uint64           `json:"before_cursor"`
	Cursor          uint64           `json:"cursor"`
	BeforeStateHash string           `json:"before_state_hash"`
	StateHash       string           `json:"state_hash"`
	SchemaHash      string           `json:"schema_hash"`
	Patches         []Patch          `json:"patches"`
	Rows            []Row            `json:"rows"`
	Quantities      []Quantity       `json:"quantities"`
	Events          []Event          `json:"events"`
	Tasks           []Intent         `json:"tasks"`
	Continuations   []Intent         `json:"continuations"`
	Outbox          []Intent         `json:"outbox"`
	Inputs          Inputs           `json:"inputs"`
	Result          checkpoint.Value `json:"result"`
	Ended           bool             `json:"ended,omitempty"`
	Hash            string           `json:"hash"`
}

type ReplayHistory struct {
	Creation Creation       `json:"creation"`
	Version  uint64         `json:"version"`
	Cursor   uint64         `json:"cursor"`
	Ended    bool           `json:"ended"`
	Records  []EffectRecord `json:"records"`
}
