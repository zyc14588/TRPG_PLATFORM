// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package model

import (
	"context"
	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/credential"
	store "github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/launch"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/room"
	"slices"
)

type frame struct {
	room room.RoomData
	prep launch.PreparationData
	slot launch.Slot
	rt   room.Transaction
	mt   Transaction
	lt   launch.Transaction
}

func (s *Service) frame(ctx context.Context, tx core.Transaction, sc core.Scope, seat string) (frame, error) {
	if !scope(sc) || !store.ValidID(seat) || tx == nil {
		return frame{}, auth.ErrInvalid
	}
	d := s.state()
	if d == nil {
		return frame{}, auth.ErrUnavailable
	}
	if _, e := auth.RoomAdmissionSession(tx); e != nil {
		return frame{}, auth.ErrDenied
	}
	rt, e := d.rooms.Bind(tx)
	if e != nil {
		return frame{}, auth.SafeError(e)
	}
	lt, e := d.launches.Bind(tx)
	if e != nil {
		return frame{}, auth.SafeError(e)
	}
	mt, e := d.storage.Bind(tx)
	if e != nil {
		return frame{}, auth.SafeError(e)
	}
	rm, e := rt.Room(ctx, sc.WorkspaceID, sc.RoomID)
	if e != nil {
		return frame{}, auth.SafeError(e)
	}
	r := rm.StorageValue()
	if r.Scope != sc || (r.State != "lobby" && r.State != "launched") {
		return frame{}, auth.ErrDenied
	}
	prep, e := lt.Preparation(ctx, sc)
	if e != nil {
		return frame{}, auth.SafeError(e)
	}
	p := prep.StorageValue()
	if p.Scope != sc || p.Revision < 1 || !graphHash(p.ConfigurationHash) || !graphHash(p.GraphHash) || len(p.Slots) < 1 || len(p.Slots) > 64 {
		return frame{}, auth.ErrDenied
	}
	var selected launch.Slot
	found := 0
	for _, slot := range p.Slots {
		if slot.ID == seat {
			selected = slot
			found++
		}
	}
	if found != 1 || (selected.Mode != "ai" && selected.Mode != "human") {
		return frame{}, auth.ErrDenied
	}
	return frame{room: r, prep: p, slot: selected, rt: rt, mt: mt, lt: lt}, nil
}
func identity(v auth.SessionData) (string, string) {
	if v.Kind == "account" && store.ValidID(v.AccountID) {
		return "account", v.AccountID
	}
	if v.Kind == "guest" && store.ValidID(v.GuestID) {
		return "guest", v.GuestID
	}
	return "", ""
}
func (f frame) access(ctx context.Context, tx core.Transaction, v auth.SessionData) (room.ParticipantData, error) {
	var p room.Participant
	var e error
	if v.Kind == "account" {
		a, e := tx.Account(ctx, v.AccountID)
		if e != nil || a.Disabled {
			return room.ParticipantData{}, auth.ErrDenied
		}
		p, e = f.rt.AccountParticipant(ctx, f.room.Scope, v.AccountID)
	} else if v.Kind == "guest" {
		if v.Scope != f.room.Scope {
			return room.ParticipantData{}, auth.ErrDenied
		}
		g, err := tx.Guest(ctx, v.Scope, v.GuestID)
		if err != nil {
			return room.ParticipantData{}, auth.ErrDenied
		}
		now, err := tx.Now(ctx)
		if err != nil || g.Disabled || g.ClaimedAccountID != "" || !now.Before(g.ExpiresAt) {
			return room.ParticipantData{}, auth.ErrDenied
		}
		p, e = f.rt.GuestParticipant(ctx, v.Scope, v.GuestID)
	} else {
		return room.ParticipantData{}, auth.ErrDenied
	}
	if e != nil {
		return room.ParticipantData{}, auth.SafeError(e)
	}
	d := p.StorageValue()
	if !d.Active || d.Scope != f.room.Scope || v.Kind == "account" && d.AccountID != v.AccountID || v.Kind == "guest" && d.GuestID != v.GuestID {
		return room.ParticipantData{}, auth.ErrDenied
	}
	if f.slot.Mode == "human" {
		if f.slot.ParticipantID != d.ID {
			return room.ParticipantData{}, auth.ErrDenied
		}
		return d, nil
	}
	// Workspace membership alone never grants AI-seat control. A currently
	// admitted host or delegated room manager must also be an active account.
	if v.Kind != "account" {
		return room.ParticipantData{}, auth.ErrDenied
	}
	if d.Host {
		return d, nil
	}
	ok, e := f.rt.Manager(ctx, f.room.Scope, v.AccountID)
	if e != nil || !ok {
		return room.ParticipantData{}, auth.ErrDenied
	}
	return d, nil
}
func retained(ctx context.Context, tx core.Transaction, sc core.Scope, kind, id string) error {
	// Same current-row owner/admin rule as core.RetainCredential; no permission
	// is added to a Member, guest, participant or room manager by this service.
	if kind != "account" {
		return auth.ErrDenied
	}
	a, e := tx.Account(ctx, id)
	if e != nil || a.Disabled {
		return auth.ErrDenied
	}
	m, e := tx.Membership(ctx, sc.WorkspaceID, id)
	if e != nil || m.WorkspaceID != sc.WorkspaceID || m.AccountID != id || (m.Role != core.Owner && m.Role != core.Admin) {
		return auth.ErrDenied
	}
	return nil
}
func (s *Service) credentialCurrent(ctx context.Context, tx core.Transaction, f frame, record credential.Record, kind, id string) error {
	x := record.StorageValue()
	b := x.Binding
	if b.Scope != f.room.Scope || b.SeatID != f.slot.ID || b.OwnerKind != kind || b.OwnerID != id || x.Revoked {
		return auth.ErrDenied
	}
	owner := auth.SessionData{Kind: kind, AccountID: id}
	if kind == "guest" {
		owner.AccountID = ""
		owner.GuestID = id
		owner.Scope = b.Scope
	}
	if _, e := f.access(ctx, tx, owner); e != nil {
		return e
	}
	if b.Lifetime == credential.Retained {
		if e := retained(ctx, tx, b.Scope, kind, id); e != nil {
			return e
		}
	}
	now, e := tx.Now(ctx)
	if e != nil {
		return auth.SafeError(e)
	}
	key, e := s.state().vault.Open(ctx, record, b, now)
	if e != nil {
		return e
	}
	key.Close()
	return nil
}
func (s *Service) configurationCurrent(ctx context.Context, tx core.Transaction, f frame, c ConfigurationData, caps []string) error {
	if c.Scope != f.room.Scope || c.SeatID != f.slot.ID || c.Selection != f.slot.ModelSelection || f.slot.Mode != "ai" || c.Revoked || c.Version < 1 || c.ConfigurationID != f.prep.ConfigurationID || c.ConfigurationHash != f.prep.ConfigurationHash || c.GraphHash != f.prep.GraphHash || c.PreparationRevision != f.prep.Revision || len(c.Fallbacks) > 4 || !validBudget(c.Budget) {
		return auth.ErrDenied
	}
	budget, ok := s.state().caps[c.Scope.WorkspaceID]
	if !ok || !fits(c.Budget, budget) {
		return auth.ErrDenied
	}
	now, e := tx.Now(ctx)
	if e != nil {
		return auth.SafeError(e)
	}
	primary, e := s.certificate(c.Scope.WorkspaceID, c.Primary)
	if e != nil || primary.Tuple != c.Tuple || primary.Level < 2 || !certified(primary, c.GraphHash, caps, now) || primary.Tuple.ToolMode == "structured" && c.Budget.Tools < 1 {
		return auth.ErrDenied
	}
	if slices.Contains(caps, "ai-player") && primary.Level < 3 || slices.Contains(caps, "ai-host") && primary.Level < 4 {
		return auth.ErrDenied
	}
	seen := map[string]bool{primary.ID: true}
	for _, bound := range c.Fallbacks {
		cert, e := s.certificate(c.Scope.WorkspaceID, bound)
		if e != nil || seen[cert.ID] || cert.Level < primary.Level || cert.Tuple.ToolMode != primary.Tuple.ToolMode || cert.Tuple.PromptTemplate != primary.Tuple.PromptTemplate || !certified(cert, c.GraphHash, primary.Capabilities, now) || !certified(cert, c.GraphHash, caps, now) {
			return auth.ErrDenied
		}
		seen[cert.ID] = true
	}
	record, e := f.mt.Credential(ctx, c.Scope, c.SeatID, c.CredentialID)
	if e != nil {
		return auth.SafeError(e)
	}
	if record.StorageValue().Binding.Version != c.CredentialVersion {
		return auth.ErrDenied
	}
	return s.credentialCurrent(ctx, tx, f, record, c.OwnerKind, c.OwnerID)
}
