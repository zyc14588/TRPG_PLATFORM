// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// Package context filters server-owned Session data before a model sees it.
// Its handles confer no native command or event-writing authority.
package context

import (
	stdcontext "context"
	"encoding/json"
	"fmt"
	"io"

	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/model"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/capability"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/command"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
)

const MaxPromptBytes = 64 << 10
const MaxMemoryBytes = 4096

type TargetData struct {
	Scope  core.Scope
	SeatID string
}
type Target = auth.Secret[TargetData]
type SubjectData struct {
	Scope                                                        core.Scope
	Binding                                                      data.Binding
	SeatID, Controller, ConfigurationID, ConfigurationHash       string
	PreparationRevision, ModelVersion, StateVersion, EventCursor uint64
	Tuple                                                        model.Tuple
	Budget                                                       model.Limits
}
type Subject = auth.Secret[SubjectData]
type SnapshotData struct {
	Binding         data.Binding
	Version, Cursor uint64
	State           checkpoint.Value
	Events          []data.JournalEvent
}
type Snapshot = auth.Secret[SnapshotData]
type MemoryData struct {
	ID, Text, FactLevel, GeneratorVersion string
	Sources                               []uint64
}
type Memory = auth.Secret[MemoryData]
type Tool struct {
	ID         string
	Capability capability.Name
	Mode       string
}
type Policy struct {
	Binding                data.Binding
	SeatID                 string
	Tuple                  model.Tuple
	Role, GeneratorVersion string
	Views                  command.ViewPolicy
	Declaration            capability.Declaration
	TrustLevel             capability.TrustLevel
	Trust                  capability.TrustPolicy
	Execution              capability.GrantSet
	Tools                  []Tool
}

// PolicyProvider is trusted installed-package composition. Nothing in a model
// request can select a filter, capability grant, role or qualification.
type PolicyProvider interface {
	Current(stdcontext.Context, core.Transaction, Subject) (Policy, error)
}
type Storage interface {
	Bind(core.Transaction) (Transaction, error)
}
type Transaction interface {
	Snapshot(stdcontext.Context, core.Scope, data.Binding) (Snapshot, error)
	Memories(stdcontext.Context, Subject) ([]Memory, error)
	PutMemory(stdcontext.Context, Subject, Memory) error
}

type PayloadData struct {
	Scope                   core.Scope
	SessionID, SeatID, Role string
	Version, Cursor         uint64
	Model                   model.Tuple
	View                    checkpoint.Value
	Events                  []data.JournalEvent
	Memories                []MemoryData
	Tools                   []Tool
}
type Prompt struct{ data **promptData }
type promptData struct {
	subject SubjectData
	payload PayloadData
	raw     []byte
}

func (Prompt) Format(f fmt.State, _ rune)   { _, _ = io.WriteString(f, "<private filtered AI context>") }
func (Prompt) MarshalJSON() ([]byte, error) { return nil, auth.ErrDenied }
func (p Prompt) state() *promptData {
	if p.data == nil {
		return nil
	}
	return *p.data
}
func (p Prompt) Bytes() int {
	if p.state() == nil {
		return 0
	}
	return len(p.state().raw)
}
func (p Prompt) Subject() Subject {
	if p.state() == nil {
		return Subject{}
	}
	return auth.RoomSecret(p.state().subject)
}

// Use is the explicit provider boundary. It owns and clears a copy; arbitrary
// provider errors are reduced to stable errors before they leave this boundary.
func (p Prompt) Use(f func([]byte) error) (err error) {
	if p.state() == nil || f == nil {
		return auth.ErrDenied
	}
	defer func() {
		if recover() != nil {
			err = auth.ErrUnavailable
		}
	}()
	b := append([]byte(nil), p.state().raw...)
	defer clear(b)
	return auth.SafeError(f(b))
}
func ownPrompt(s SubjectData, p PayloadData) (Prompt, error) {
	raw, e := json.Marshal(p)
	if e != nil || len(raw) > MaxPromptBytes {
		return Prompt{}, auth.ErrDenied
	}
	d := &promptData{subject: s, raw: raw}
	if json.Unmarshal(raw, &d.payload) != nil {
		return Prompt{}, auth.ErrDenied
	}
	return Prompt{data: &d}, nil
}

// Advice strips proposal tools as well as every native authority. An advisor
// receives only its parent seat's already filtered context and private memory.
func (p Prompt) Advice() (Prompt, error) {
	if p.state() == nil {
		return Prompt{}, auth.ErrDenied
	}
	v := p.state().payload
	v.Role = "advice"
	v.Tools = nil
	for _, t := range p.state().payload.Tools {
		if t.Mode == "read" || t.Mode == "advise" {
			v.Tools = append(v.Tools, t)
		}
	}
	return ownPrompt(p.state().subject, v)
}
