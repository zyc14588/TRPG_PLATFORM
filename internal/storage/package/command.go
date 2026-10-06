// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// Package packagedata defines data-only command persistence. PostgreSQL owns
// transaction and SQL implementation; neither Lua nor Host callbacks see a DB.
package packagedata

import (
	"context"
	"errors"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
)

var ErrConflict = errors.New("COMMAND_CONFLICT")
var ErrDenied = errors.New("COMMAND_STORAGE_DENIED")
var ErrNotFound = errors.New("COMMAND_NOT_FOUND")
var ErrUnknownCommit = errors.New("COMMAND_COMMIT_OUTCOME_UNKNOWN")

type Binding struct {
	Workspace string `json:"workspace"`
	Session   string `json:"session"`
	GraphHash string `json:"graph_hash"`
}
type Header struct {
	Binding         Binding `json:"binding"`
	Principal       string  `json:"principal"`
	CommandID       string  `json:"command_id"`
	Fingerprint     string  `json:"fingerprint"`
	ExpectedVersion uint64  `json:"expected_version"`
}
type Inputs struct {
	Callback string           `json:"callback"`
	Time     int64            `json:"time"`
	Random   []int64          `json:"random"`
	Command  checkpoint.Value `json:"command"`
}
type Row struct {
	PackageID  string           `json:"package_id"`
	Namespace  string           `json:"namespace"`
	Key        string           `json:"key"`
	SchemaHash string           `json:"schema_hash"`
	Value      checkpoint.Value `json:"value"`
	Deleted    bool             `json:"deleted,omitempty"`
}
type Quantity struct {
	PackageID string `json:"package_id"`
	Table     string `json:"table"`
	Key       string `json:"key"`
	Value     int64  `json:"value"`
}
type Event struct {
	ID      string           `json:"id"`
	Type    string           `json:"type"`
	Payload checkpoint.Value `json:"payload"`
}
type Intent struct {
	ID        string           `json:"id"`
	PackageID string           `json:"package_id"`
	Kind      string           `json:"kind"`
	Payload   checkpoint.Value `json:"payload"`
}
type Patch struct {
	Path       []string `json:"path"`
	BeforeHash string   `json:"before_hash"`
	AfterHash  string   `json:"after_hash"`
	Module     string   `json:"module"`
	Line       int      `json:"line"`
	CommandID  string   `json:"command_id"`
	EventID    string   `json:"event_id"`
}
type Audit struct {
	Workspace     string   `json:"workspace"`
	Session       string   `json:"session"`
	CommandID     string   `json:"command_id"`
	Level         string   `json:"level"`
	Module        string   `json:"module"`
	PackageID     string   `json:"package_id"`
	Operation     string   `json:"operation"`
	Phase         string   `json:"phase"`
	ArgumentsHash string   `json:"arguments_hash"`
	ResultHash    string   `json:"result_hash"`
	Outcome       string   `json:"outcome"`
	DurationNanos int64    `json:"duration_nanos"`
	StateVersion  uint64   `json:"state_version"`
	CallbackCount int      `json:"callback_count"`
	Tables        []string `json:"tables,omitempty"`
	EventID       string   `json:"event_id,omitempty"`
}
type Snapshot struct {
	Binding    Binding          `json:"binding"`
	Version    uint64           `json:"version"`
	State      checkpoint.Value `json:"state"`
	SchemaHash string           `json:"schema_hash"`
	Rows       []Row            `json:"rows"`
	Quantities []Quantity       `json:"quantities"`
	Existing   *Receipt         `json:"existing,omitempty"`
}
type Commit struct {
	Header        Header           `json:"header"`
	State         checkpoint.Value `json:"state"`
	SchemaHash    string           `json:"schema_hash"`
	Patches       []Patch          `json:"patches"`
	Rows          []Row            `json:"rows"`
	Quantities    []Quantity       `json:"quantities"`
	Events        []Event          `json:"events"`
	Tasks         []Intent         `json:"tasks"`
	Continuations []Intent         `json:"continuations"`
	Outbox        []Intent         `json:"outbox"`
	Result        checkpoint.Value `json:"result"`
	Inputs        Inputs           `json:"inputs"`
	Audit         []Audit          `json:"audit"`
}
type Receipt struct {
	Header  Header           `json:"header"`
	Version uint64           `json:"version"`
	Result  checkpoint.Value `json:"result"`
	Events  []Event          `json:"events"`
	Inputs  Inputs           `json:"inputs"`
}
type Transaction interface {
	Snapshot() Snapshot
	Commit(context.Context, Commit) (Receipt, error)
	Rollback() error
}
type Repository interface {
	Begin(context.Context, Header) (Transaction, error)
}
