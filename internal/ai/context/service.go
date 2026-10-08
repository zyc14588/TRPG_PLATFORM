// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package context

import (
	stdcontext "context"
	"encoding/json"
	"fmt"
	"io"
	"slices"

	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/model"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/capability"
	store "github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/launch"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/room"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/command"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/realtime"
)

type Options struct {
	Authority    *auth.RoomAuthority
	Rooms        room.Storage
	Launches     launch.Storage
	Models       launch.ModelChecker
	ModelStorage model.Storage
	Storage      Storage
	Policies     PolicyProvider
}
type Service struct{ data **serviceData }
type serviceData struct{ options Options }

func (Service) Format(f fmt.State, _ rune)   { _, _ = io.WriteString(f, "<private AI context service>") }
func (Service) MarshalJSON() ([]byte, error) { return nil, auth.ErrDenied }
func (s *Service) state() *serviceData {
	if s == nil || s.data == nil {
		return nil
	}
	return *s.data
}
func New(o Options) (*Service, error) {
	if o.Authority == nil || o.Rooms == nil || o.Launches == nil || o.Models == nil || o.ModelStorage == nil || o.Storage == nil || o.Policies == nil {
		return nil, auth.ErrInvalid
	}
	d := &serviceData{options: o}
	return &Service{data: &d}, nil
}
func validTarget(t TargetData) bool {
	return store.ValidID(t.Scope.WorkspaceID) && store.ValidID(t.Scope.RoomID) && store.ValidID(t.Scope.GameID) && store.ValidID(t.SeatID)
}

func (s *Service) resolve(ctx stdcontext.Context, tx core.Transaction, target Target) (SubjectData, launch.PreparationData, launch.Transaction, Transaction, error) {
	t := target.StorageValue()
	if s.state() == nil || ctx == nil || ctx.Err() != nil || tx == nil || !validTarget(t) {
		return SubjectData{}, launch.PreparationData{}, nil, nil, auth.ErrDenied
	}
	v, e := auth.RoomAdmissionSession(tx)
	if e != nil {
		return SubjectData{}, launch.PreparationData{}, nil, nil, auth.ErrDenied
	}
	caller := v.StorageValue()
	if caller.Kind != "account" {
		return SubjectData{}, launch.PreparationData{}, nil, nil, auth.ErrDenied
	}
	a, e := tx.Account(ctx, caller.AccountID)
	if e != nil || a.Disabled {
		return SubjectData{}, launch.PreparationData{}, nil, nil, auth.ErrDenied
	}
	o := s.state().options
	rt, e := o.Rooms.Bind(tx)
	if e != nil {
		return SubjectData{}, launch.PreparationData{}, nil, nil, auth.SafeError(e)
	}
	rm, e := rt.Room(ctx, t.Scope.WorkspaceID, t.Scope.RoomID)
	if e != nil || rm.StorageValue().Scope != t.Scope || rm.StorageValue().State != "launched" {
		return SubjectData{}, launch.PreparationData{}, nil, nil, auth.ErrDenied
	}
	part, e := rt.AccountParticipant(ctx, t.Scope, caller.AccountID)
	if e != nil {
		return SubjectData{}, launch.PreparationData{}, nil, nil, auth.ErrDenied
	}
	p := part.StorageValue()
	if !p.Active || p.Scope != t.Scope || p.AccountID != caller.AccountID {
		return SubjectData{}, launch.PreparationData{}, nil, nil, auth.ErrDenied
	}
	if !p.Host {
		ok, e := rt.Manager(ctx, t.Scope, caller.AccountID)
		if e != nil || !ok {
			return SubjectData{}, launch.PreparationData{}, nil, nil, auth.ErrDenied
		}
	}
	lt, e := o.Launches.Bind(tx)
	if e != nil {
		return SubjectData{}, launch.PreparationData{}, nil, nil, auth.SafeError(e)
	}
	pre, e := lt.Preparation(ctx, t.Scope)
	if e != nil {
		return SubjectData{}, launch.PreparationData{}, nil, nil, auth.SafeError(e)
	}
	prep := pre.StorageValue()
	selected := launch.Slot{}
	count := 0
	for _, slot := range prep.Slots {
		if slot.ID == t.SeatID {
			selected = slot
			count++
		}
	}
	if count != 1 || selected.Mode != "ai" || !store.ValidID(selected.ModelSelection) || prep.Scope != t.Scope || prep.Revision < 1 {
		return SubjectData{}, launch.PreparationData{}, nil, nil, auth.ErrDenied
	}
	saved, e := lt.Session(ctx, t.Scope)
	if e != nil {
		return SubjectData{}, launch.PreparationData{}, nil, nil, auth.SafeError(e)
	}
	session := saved.StorageValue()
	if session.Scope != t.Scope || session.Binding.Workspace != t.Scope.WorkspaceID || !store.ValidID(session.Binding.Session) || session.Binding.GraphHash != prep.GraphHash || session.ConfigurationID != prep.ConfigurationID || session.ConfigurationHash != prep.ConfigurationHash || session.Revision != prep.Revision {
		return SubjectData{}, launch.PreparationData{}, nil, nil, auth.ErrDenied
	}
	mt, e := o.ModelStorage.Bind(tx)
	if e != nil {
		return SubjectData{}, launch.PreparationData{}, nil, nil, auth.SafeError(e)
	}
	configuration, e := mt.Configuration(ctx, t.Scope, t.SeatID, selected.ModelSelection)
	if e != nil {
		return SubjectData{}, launch.PreparationData{}, nil, nil, auth.SafeError(e)
	}
	m := configuration.StorageValue()
	if m.Scope != t.Scope || m.SeatID != t.SeatID || m.Revoked || m.Version < 1 || m.PreparationRevision != prep.Revision || m.GraphHash != prep.GraphHash || m.ConfigurationID != prep.ConfigurationID || m.ConfigurationHash != prep.ConfigurationHash {
		return SubjectData{}, launch.PreparationData{}, nil, nil, auth.ErrDenied
	}
	st, e := o.Storage.Bind(tx)
	if e != nil {
		return SubjectData{}, launch.PreparationData{}, nil, nil, auth.SafeError(e)
	}
	subject := SubjectData{Scope: t.Scope, Binding: session.Binding, SeatID: t.SeatID, Controller: caller.AccountID, ConfigurationID: prep.ConfigurationID, ConfigurationHash: prep.ConfigurationHash, PreparationRevision: prep.Revision, ModelVersion: m.Version, Tuple: m.Tuple, Budget: m.Budget}
	return subject, prep, lt, st, nil
}
func ownPolicy(p Policy, v SubjectData) (Policy, []string, error) {
	if p.Binding != v.Binding || p.SeatID != v.SeatID || p.Tuple != v.Tuple || (p.Role != "player" && p.Role != "host") || !store.ValidID(p.GeneratorVersion) || len(p.Tools) > 32 {
		return Policy{}, nil, auth.ErrDenied
	}
	views, e := command.OwnPolicy(p.Views)
	if e != nil {
		return Policy{}, nil, auth.ErrDenied
	}
	p.Views = views
	resolved, e := capability.Resolve(p.Declaration, p.TrustLevel, p.Trust, p.Execution)
	if e != nil {
		return Policy{}, nil, auth.ErrDenied
	}
	tools := []Tool{}
	seen := map[string]bool{}
	for _, tool := range p.Tools {
		if !store.ValidID(tool.ID) || seen[tool.ID] || (tool.Mode != "read" && tool.Mode != "propose" && tool.Mode != "advise") {
			return Policy{}, nil, auth.ErrDenied
		}
		seen[tool.ID] = true
		if _, e := capability.ParseName(string(tool.Capability)); e != nil {
			return Policy{}, nil, auth.ErrDenied
		}
		if resolved.Effective.Contains(tool.Capability) {
			tools = append(tools, tool)
		}
	}
	if v.Tuple.ToolMode != "structured" {
		tools = nil
	}
	p.Tools = tools
	caps := []string{"structured-actions", "ai-player"}
	if p.Role == "host" {
		caps = []string{"structured-actions", "ai-host"}
	}
	return p, caps, nil
}

// BuildWithin is called only in the existing live cookie-bound transaction.
// Full state never leaves the protected storage handle before leaf filtering.
func (s *Service) BuildWithin(ctx stdcontext.Context, tx core.Transaction, target Target) (prompt Prompt, err error) {
	defer func() {
		if recover() != nil {
			prompt = Prompt{}
			err = auth.ErrUnavailable
		}
	}()
	v, prep, lt, st, e := s.resolve(ctx, tx, target)
	if e != nil {
		return Prompt{}, e
	}
	p, e := s.state().options.Policies.Current(ctx, tx, auth.RoomSecret(v))
	if e != nil {
		return Prompt{}, auth.SafeError(e)
	}
	p, caps, e := ownPolicy(p, v)
	if e != nil {
		return Prompt{}, e
	}
	acks, e := lt.Acknowledgments(ctx, v.Scope)
	if e != nil {
		return Prompt{}, auth.SafeError(e)
	}
	proof := auth.RoomSecret(launch.ModelRequirementData{Scope: v.Scope, ConfigurationID: v.ConfigurationID, ConfigurationHash: v.ConfigurationHash, GraphHash: v.Binding.GraphHash, SeatID: v.SeatID, Selection: findSelection(prep, v.SeatID), Revision: v.PreparationRevision, Capabilities: caps})
	if e = s.state().options.Models.Check(ctx, tx, proof, acks); e != nil {
		return Prompt{}, auth.SafeError(e)
	}
	snapshot, e := st.Snapshot(ctx, v.Scope, v.Binding)
	if e != nil {
		return Prompt{}, auth.SafeError(e)
	}
	snap := snapshot.StorageValue()
	if snap.Binding != v.Binding || snap.Version < 1 || len(snap.Events) > 128 || checkpoint.Validate(snap.State) != nil {
		return Prompt{}, auth.ErrDenied
	}
	v.StateVersion = snap.Version
	v.EventCursor = snap.Cursor
	events := realtime.FilterEvents(snap.Events, p.Views)
	visible := map[uint64]bool{}
	for _, event := range events {
		if event.Sequence < 1 || event.Sequence > snap.Cursor || event.Version > snap.Version {
			return Prompt{}, auth.ErrDenied
		}
		visible[event.Sequence] = true
	}
	memory, e := st.Memories(ctx, auth.RoomSecret(v))
	if e != nil || len(memory) > 64 {
		return Prompt{}, auth.SafeError(eOrDenied(e))
	}
	memories := []MemoryData{}
	for _, m := range memory {
		x := m.StorageValue()
		if !validMemory(x, p.GeneratorVersion) {
			return Prompt{}, auth.ErrDenied
		}
		allowed := true
		for _, seq := range x.Sources {
			if !visible[seq] {
				allowed = false
			}
		}
		if allowed {
			x.Sources = slices.Clone(x.Sources)
			memories = append(memories, x)
		}
	}
	return ownPrompt(v, PayloadData{Scope: v.Scope, SessionID: v.Binding.Session, SeatID: v.SeatID, Role: p.Role, Version: snap.Version, Cursor: snap.Cursor, Model: v.Tuple, View: realtime.Filter(snap.State, p.Views.ViewFields), Events: events, Memories: memories, Tools: p.Tools})
}
func eOrDenied(e error) error {
	if e == nil {
		return auth.ErrDenied
	}
	return e
}
func findSelection(p launch.PreparationData, seat string) string {
	for _, s := range p.Slots {
		if s.ID == seat {
			return s.ModelSelection
		}
	}
	return ""
}
func validMemory(m MemoryData, generator string) bool {
	if !store.ValidID(m.ID) || len(m.Text) < 1 || len(m.Text) > MaxMemoryBytes || m.GeneratorVersion != generator || (m.FactLevel != "summary" && m.FactLevel != "hypothesis") || len(m.Sources) < 1 || len(m.Sources) > 32 {
		return false
	}
	seen := map[uint64]bool{}
	for _, n := range m.Sources {
		if n < 1 || n >= 1<<53 || seen[n] {
			return false
		}
		seen[n] = true
	}
	return true
}
func (s *Service) Build(ctx stdcontext.Context, caller model.Caller, target Target) (Prompt, error) {
	if s.state() == nil {
		return Prompt{}, auth.ErrUnavailable
	}
	c := caller.StorageValue()
	var p Prompt
	e := s.state().options.Authority.Inspect(ctx, c.Credential, c.CSRF, false, func(ctx stdcontext.Context, tx auth.Transaction, _ auth.SessionData) error {
		var e error
		p, e = s.BuildWithin(ctx, tx.Core(), target)
		return e
	})
	if e != nil {
		return Prompt{}, auth.SafeError(e)
	}
	return p, nil
}
func (s *Service) Remember(ctx stdcontext.Context, caller model.Caller, target Target, memory Memory) error {
	if s.state() == nil {
		return auth.ErrUnavailable
	}
	c := caller.StorageValue()
	m := memory.StorageValue()
	m.Sources = slices.Clone(m.Sources)
	t := target.StorageValue()
	raw, e := json.Marshal(struct {
		Target TargetData
		Memory MemoryData
	}{t, m})
	if e != nil || len(raw) > 16<<10 {
		return auth.ErrInvalid
	}
	work := func(ctx stdcontext.Context, tx auth.Transaction, saved *auth.Outcome) (auth.Outcome, error) {
		prompt, e := s.BuildWithin(ctx, tx.Core(), target)
		if e != nil {
			return auth.Outcome{}, e
		}
		v := prompt.Subject()
		p, e := s.state().options.Policies.Current(ctx, tx.Core(), v)
		if e != nil || !validMemory(m, p.GeneratorVersion) {
			return auth.Outcome{}, auth.ErrDenied
		}
		visible := map[uint64]bool{}
		for _, event := range prompt.state().payload.Events {
			visible[event.Sequence] = true
		}
		for _, n := range m.Sources {
			if !visible[n] {
				return auth.Outcome{}, auth.ErrDenied
			}
		}
		st, e := s.state().options.Storage.Bind(tx.Core())
		if e != nil {
			return auth.Outcome{}, auth.SafeError(e)
		}
		if e = st.PutMemory(ctx, v, auth.RoomSecret(m)); e != nil {
			return auth.Outcome{}, auth.SafeError(e)
		}
		out := auth.RoomOutcome([]byte(`{"remembered":true}`))
		if saved != nil && !slices.Equal(saved.StorageValue().Body, out.StorageValue().Body) {
			return auth.Outcome{}, auth.ErrConflict
		}
		return out, nil
	}
	_, e = s.state().options.Authority.Do(ctx, c.Credential, c.CSRF, c.IdempotencyKey, c.Network, auth.RoomSecret(auth.RoomCommandData{Action: "ai.memory", TargetKey: t.Scope.WorkspaceID + "/" + t.Scope.RoomID + "/" + t.Scope.GameID + "/" + t.SeatID + "/" + m.ID, Canonical: raw, Write: true}), auth.RoomCallbacks{Apply: func(c stdcontext.Context, t auth.Transaction, _ auth.SessionData) (auth.Outcome, error) {
		return work(c, t, nil)
	}, Replay: func(c stdcontext.Context, t auth.Transaction, _ auth.SessionData, o auth.Outcome) error {
		_, e := work(c, t, &o)
		return e
	}})
	return auth.SafeError(e)
}
