// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package launch

import (
	"context"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/install"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
	"slices"
)

type PresentationGraphData struct {
	WorkspaceID, ConfigurationID, ConfigurationHash, GraphHash string
	Packages                                                   []install.PresentationPackage
	Seats                                                      []SeatRule
}
type PresentationGraph = auth.Secret[PresentationGraphData]

func (s *Service) PlayerPresentationModels(checker ModelChecker) bool {
	return s.state() != nil && checker != nil && s.state().models == checker
}

// PlayerPresentationWithin runs inside the current cookie admission transaction.
// The guest can read only the exact room's current selected preparation; account
// membership permits public configuration facts, never private seat data.
func (s *Service) PlayerPresentationWithin(ctx context.Context, tx auth.Transaction, v auth.SessionData, w, id, game string) (PresentationGraph, error) {
	if s.state() == nil || ctx == nil || ctx.Err() != nil || tx == nil || !store.ValidID(w) || !store.ValidID(id) || !store.ValidID(game) {
		return PresentationGraph{}, auth.ErrInvalid
	}
	bound, e := auth.RoomAdmissionSession(tx.Core())
	if e != nil || bound.StorageValue().Hash != v.Hash {
		return PresentationGraph{}, auth.ErrDenied
	}
	cfg, e := s.config(w, id)
	if e != nil {
		return PresentationGraph{}, e
	}
	sc := core.Scope{WorkspaceID: w, GameID: game}
	var prep *PreparationData
	switch v.Kind {
	case "account":
		a, e := tx.Core().Account(ctx, v.AccountID)
		if e != nil || a.Disabled {
			return PresentationGraph{}, auth.ErrDenied
		}
		if _, e = tx.Core().Workspace(ctx, w); e != nil {
			return PresentationGraph{}, auth.ErrDenied
		}
		m, e := tx.Core().Membership(ctx, w, v.AccountID)
		if e != nil || m.WorkspaceID != w || m.AccountID != v.AccountID || m.Role != core.Owner && m.Role != core.Admin && m.Role != core.Member {
			return PresentationGraph{}, auth.ErrDenied
		}
	case "guest":
		if v.Scope.WorkspaceID != w {
			return PresentationGraph{}, auth.ErrDenied
		}
		rt, lt, rm, e := s.room(ctx, tx, w, v.Scope.RoomID)
		if e != nil || rm.Scope != v.Scope || rm.State != "lobby" && rm.State != "launched" {
			return PresentationGraph{}, auth.ErrDenied
		}
		if _, e = participant(ctx, tx.Core(), rt, rm, v); e != nil {
			return PresentationGraph{}, e
		}
		p, e := lt.Preparation(ctx, rm.Scope)
		if e != nil {
			return PresentationGraph{}, auth.SafeError(e)
		}
		q := p.StorageValue()
		if q.ConfigurationID != id || !validPreparation(q, cfg) {
			return PresentationGraph{}, auth.ErrDenied
		}
		prep = &q
		sc = rm.Scope
	default:
		return PresentationGraph{}, auth.ErrUnauthenticated
	}
	r := cfg.Request
	r.Workspace, r.Session = w, sc.GameID
	hash, packages, e := cfg.Factory.Presentation(ctx, r)
	if e != nil || ctx.Err() != nil || prep != nil && hash != prep.GraphHash {
		return PresentationGraph{}, auth.ErrDenied
	}
	seats := slices.Clone(cfg.Seats)
	for i := range seats {
		seats[i].Modes = slices.Clone(seats[i].Modes)
		seats[i].ModelCapabilities = slices.Clone(seats[i].ModelCapabilities)
	}
	return auth.RoomSecret(PresentationGraphData{w, id, configurationHash(cfg), hash, packages, seats}), nil
}
