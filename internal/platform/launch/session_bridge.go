// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package launch

import (
	"context"
	"fmt"
	"io"
	"sync/atomic"

	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/actor"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/command"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/realtime"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
)

// SessionAccess is issued only from the actual authenticated SQL transaction.
// Workspace membership, room management and admission alone confer no seat.
type SessionAccess struct{ data **sessionAccessData }
type sessionAccessData struct {
	issuer *Service
	value  SessionAccessData
}
type SessionAccessData struct {
	Scope                                               core.Scope
	Binding                                             data.Binding
	ConfigurationID, ConfigurationHash, Principal, Seat string
	Host, Administrator                                 bool
}

func (SessionAccess) Format(f fmt.State, _ rune)   { _, _ = io.WriteString(f, "<verified game access>") }
func (SessionAccess) MarshalJSON() ([]byte, error) { return nil, auth.ErrDenied }
func (v SessionAccess) state() *sessionAccessData {
	if v.data == nil {
		return nil
	}
	return *v.data
}

// StorageValue is for trusted policy/storage composition, never a wire identity.
func (v SessionAccess) StorageValue() SessionAccessData {
	if v.state() == nil {
		return SessionAccessData{}
	}
	return v.state().value
}
func (s *Service) sessionAccess(ctx context.Context, tx auth.Transaction, v auth.SessionData, w, r string) (SessionAccess, error) {
	rt, lt, rm, e := s.room(ctx, tx, w, r)
	if e != nil {
		return SessionAccess{}, e
	}
	if rm.State != "launched" {
		return SessionAccess{}, auth.ErrDenied
	}
	part, e := participant(ctx, tx.Core(), rt, rm, v)
	if e != nil {
		return SessionAccess{}, e
	}
	saved, e := lt.Session(ctx, rm.Scope)
	if e != nil {
		return SessionAccess{}, auth.SafeError(e)
	}
	bound := saved.StorageValue()
	prep, e := lt.Preparation(ctx, rm.Scope)
	if e != nil {
		return SessionAccess{}, auth.SafeError(e)
	}
	p := prep.StorageValue()
	cfg, e := s.config(w, p.ConfigurationID)
	if e != nil {
		return SessionAccess{}, e
	}
	hash, e := sessionRequest(cfg, rm.Scope).authenticate(ctx)
	if e != nil || hash != p.GraphHash || !validPreparation(p, cfg) || bound.Scope != rm.Scope || bound.Binding.Workspace != w || bound.Binding.GraphHash != hash || bound.ConfigurationID != p.ConfigurationID || bound.ConfigurationHash != p.ConfigurationHash || bound.Revision != p.Revision {
		return SessionAccess{}, auth.ErrDenied
	}
	seat := ""
	for _, slot := range p.Slots {
		if slot.ParticipantID == part.ID {
			if slot.Mode != "human" || seat != "" {
				return SessionAccess{}, auth.ErrDenied
			}
			seat = slot.ID
		}
	}
	if seat == "" {
		return SessionAccess{}, auth.ErrDenied
	}
	d := &sessionAccessData{issuer: s, value: SessionAccessData{Scope: rm.Scope, Binding: bound.Binding, ConfigurationID: p.ConfigurationID, ConfigurationHash: p.ConfigurationHash, Principal: part.ID, Seat: seat, Host: part.Host, Administrator: manager(ctx, tx.Core(), rt, rm, v) == nil}}
	return SessionAccess{data: &d}, nil
}
func (s *Service) liveService(ctx context.Context, w, r string) error {
	if s.state() == nil || s.state().actors.state() == nil || s.state().actors.state().ctx.Err() != nil || ctx == nil || ctx.Err() != nil {
		return auth.ErrUnavailable
	}
	if !store.ValidID(w) || !store.ValidID(r) {
		return auth.ErrInvalid
	}
	return nil
}

// Admission charges the existing user-operation limiter once. Subsequent live
// checks share that authority through Inspect rather than duplicating its policy.
func (s *Service) AdmitSession(ctx context.Context, c Caller, w, r string) (SessionAccess, error) {
	if e := s.liveService(ctx, w, r); e != nil {
		return SessionAccess{}, e
	}
	var out SessionAccess
	v := c.StorageValue()
	cmd := auth.RoomSecret(auth.RoomCommandData{Action: "session.admit", TargetKey: w + "/" + r, Write: false})
	_, e := s.state().authority.Do(ctx, v.Credential, "", "", v.Network, cmd, auth.RoomCallbacks{Apply: func(ctx context.Context, tx auth.Transaction, v auth.SessionData) (auth.Outcome, error) {
		var e error
		out, e = s.sessionAccess(ctx, tx, v, w, r)
		if e != nil {
			return auth.Outcome{}, e
		}
		return auth.RoomOutcome([]byte(`{"ok":true}`)), nil
	}, Replay: func(context.Context, auth.Transaction, auth.SessionData, auth.Outcome) error { return auth.ErrDenied }})
	if e != nil {
		return SessionAccess{}, e
	}
	return out, nil
}
func (s *Service) CurrentSessionAccess(ctx context.Context, c Caller, w, r string, mutation bool) (SessionAccess, error) {
	if e := s.liveService(ctx, w, r); e != nil {
		return SessionAccess{}, e
	}
	var out SessionAccess
	v := c.StorageValue()
	e := s.state().authority.Inspect(ctx, v.Credential, v.CSRF, mutation, func(ctx context.Context, tx auth.Transaction, v auth.SessionData) error {
		var e error
		out, e = s.sessionAccess(ctx, tx, v, w, r)
		return e
	})
	if e != nil {
		return SessionAccess{}, e
	}
	return out, nil
}

// SessionTransport references the existing accepted registry and hub. It never
// creates a second writer or exposes its authority/identity to a client.
type SessionTransport struct{ data **sessionTransportData }
type sessionTransportData struct {
	authority  *command.Authority
	identity   command.Identity
	registry   *actor.Registry
	hub        *realtime.Hub
	connection *realtime.Connection
	closed     atomic.Bool
}

func (SessionTransport) Format(f fmt.State, _ rune) {
	_, _ = io.WriteString(f, "<private game transport>")
}
func (SessionTransport) MarshalJSON() ([]byte, error) { return nil, auth.ErrDenied }
func (t *SessionTransport) state() *sessionTransportData {
	if t == nil || t.data == nil {
		return nil
	}
	return *t.data
}
func (s *Service) ConnectSession(ctx context.Context, access SessionAccess, current command.NativeResolver) (*SessionTransport, error) {
	a := access.state()
	if a == nil || a.issuer != s || current == nil {
		return nil, auth.ErrDenied
	}
	v := a.value
	if e := s.liveService(ctx, v.Scope.WorkspaceID, v.Scope.RoomID); e != nil {
		return nil, e
	}
	cfg, e := s.config(v.Scope.WorkspaceID, v.ConfigurationID)
	if e != nil || configurationHash(cfg) != v.ConfigurationHash {
		return nil, auth.ErrDenied
	}
	// On a restart the committed native binding is recovered, never provisioned.
	if e = s.state().actors.Activate(ctx, nil, v.Binding, auth.RoomSecret(cfg), v.Seat); e != nil {
		return nil, e
	}
	d := s.state().actors.state()
	d.mu.Lock()
	entry := d.entries[bindingKey(v.Binding)]
	if d.closed || entry == nil || !entry.active || entry.binding != v.Binding || entry.authority == nil || entry.hub == nil {
		d.mu.Unlock()
		return nil, auth.ErrUnavailable
	}
	authority, registry, hub := entry.authority, entry.registry, entry.hub
	d.mu.Unlock()
	identity, e := authority.IssueNative(ctx, current)
	if e != nil {
		return nil, e
	}
	if identity.Binding() != v.Binding || identity.Principal() != v.Principal || identity.Seat() != v.Seat {
		_ = authority.ReleaseNative(identity)
		return nil, auth.ErrDenied
	}
	connection, e := hub.SubscribeContext(ctx, identity)
	if e != nil {
		_ = authority.ReleaseNative(identity)
		return nil, e
	}
	td := &sessionTransportData{authority: authority, identity: identity, registry: registry, hub: hub, connection: connection}
	return &SessionTransport{data: &td}, nil
}
func (t *SessionTransport) usable(ctx context.Context) error {
	if t.state() == nil || t.state().closed.Load() || ctx == nil || ctx.Err() != nil {
		return command.ErrDenied
	}
	return t.state().authority.VerifyContext(ctx, t.state().identity)
}
func (t *SessionTransport) Submit(ctx context.Context, e command.Envelope) (data.Receipt, error) {
	if err := t.usable(ctx); err != nil {
		return data.Receipt{}, err
	}
	return t.state().registry.Submit(ctx, t.state().identity, e)
}
func (t *SessionTransport) Snapshot(ctx context.Context, after uint64) (realtime.Frame, error) {
	if e := t.usable(ctx); e != nil {
		return realtime.Frame{}, e
	}
	d := t.state()
	f, e := d.registry.Reconnect(ctx, d.identity, after, nil)
	if e != nil {
		return realtime.Frame{}, e
	}
	p, e := d.authority.PolicyContext(ctx, d.identity)
	if e != nil {
		return realtime.Frame{}, e
	}
	f.View = realtime.Filter(f.View, p.ViewFields)
	f.Events = realtime.FilterEvents(f.Events, p)
	return f, nil
}
func (t *SessionTransport) Reconnect(ctx context.Context, after uint64) error {
	if e := t.usable(ctx); e != nil {
		return e
	}
	d := t.state()
	_, e := d.registry.Reconnect(ctx, d.identity, after, d.connection)
	return e
}
func (t *SessionTransport) Next(ctx context.Context) (realtime.Frame, error) {
	if t.state() == nil || t.state().closed.Load() {
		return realtime.Frame{}, command.ErrDenied
	}
	return t.state().connection.Next(ctx)
}
func (t *SessionTransport) CreateRecoveryPoint(ctx context.Context) (actor.RecoveryPoint, error) {
	if e := t.usable(ctx); e != nil {
		return actor.RecoveryPoint{}, e
	}
	d := t.state()
	return d.registry.CreateRecoveryPoint(ctx, d.identity)
}
func (t *SessionTransport) Close() {
	if t.state() == nil || t.state().closed.Swap(true) {
		return
	}
	d := t.state()
	d.connection.Close()
	_ = d.authority.ReleaseNative(d.identity)
}
