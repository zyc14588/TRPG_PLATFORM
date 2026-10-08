// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package launch

import (
	"context"
	"errors"
	"slices"
	"strings"

	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
)

var ErrPlayerPaused = errors.New("PLAYER_PAUSED")

// PlayerControl is trusted composition. The native SQL repository also fences
// each write through the same durable control row, including cancelled callers.
// A permit never confers a seat or bypasses the native resolver.
type PlayerControl interface {
	BeginMutation(context.Context, core.Scope, data.Binding) (context.Context, func(), error)
}

type PlayerConfigurationData struct {
	WorkspaceID, ID         string
	ContentTags, SafetyTags []string
	Seats                   []SeatRule
}
type PlayerConfiguration = auth.Secret[PlayerConfigurationData]
type PlayerLobbyData struct {
	Scope                        core.Scope
	Preparation                  PreparationData
	Own                          AcknowledgmentData
	Configured, Ready, CanManage bool
}
type PlayerLobby = auth.Secret[PlayerLobbyData]
type PlayerHuman struct {
	Principal, Seat string
	Required        bool
}
type PlayerReferenceData struct {
	Access              SessionAccessData
	PreparationRevision uint64
	Humans              []PlayerHuman
	OwnConfirmed, Ready bool
}
type PlayerReference = auth.Secret[PlayerReferenceData]

func publicConfiguration(c ConfigurationData) PlayerConfiguration {
	v := PlayerConfigurationData{WorkspaceID: c.WorkspaceID, ID: c.ID, ContentTags: slices.Clone(c.ContentTags), SafetyTags: slices.Clone(c.SafetyTags), Seats: slices.Clone(c.Seats)}
	for i := range v.Seats {
		v.Seats[i].Modes = slices.Clone(v.Seats[i].Modes)
		v.Seats[i].ModelCapabilities = nil
	}
	return auth.RoomSecret(v)
}

// PlayerCatalog admits only workspace accounts or the guest's exact current
// room configuration. Configuration factories, grants and evidence never leave.
func (s *Service) PlayerCatalog(ctx context.Context, c Caller, w string) ([]PlayerConfiguration, error) {
	if s.state() == nil || ctx == nil || ctx.Err() != nil || !store.ValidID(w) {
		return nil, auth.ErrInvalid
	}
	out := []PlayerConfiguration{}
	v := c.StorageValue()
	cmd := auth.RoomSecret(auth.RoomCommandData{Action: "player.catalog", TargetKey: w})
	_, e := s.state().authority.Do(ctx, v.Credential, "", "", v.Network, cmd, auth.RoomCallbacks{Apply: func(ctx context.Context, tx auth.Transaction, v auth.SessionData) (auth.Outcome, error) {
		switch v.Kind {
		case "account":
			a, e := tx.Core().Account(ctx, v.AccountID)
			if e != nil || a.Disabled {
				return auth.Outcome{}, auth.ErrDenied
			}
			if _, e = tx.Core().Workspace(ctx, w); e != nil {
				return auth.Outcome{}, auth.ErrDenied
			}
			m, e := tx.Core().Membership(ctx, w, v.AccountID)
			if e != nil || m.WorkspaceID != w || m.AccountID != v.AccountID || (m.Role != core.Owner && m.Role != core.Admin && m.Role != core.Member) {
				return auth.Outcome{}, auth.ErrDenied
			}
			keys := make([]string, 0, len(s.state().configs))
			for k, x := range s.state().configs {
				if x.StorageValue().WorkspaceID == w {
					keys = append(keys, k)
				}
			}
			slices.Sort(keys)
			for _, k := range keys {
				out = append(out, publicConfiguration(s.state().configs[k].StorageValue()))
			}
		case "guest":
			if v.Scope.WorkspaceID != w {
				return auth.Outcome{}, auth.ErrDenied
			}
			rt, lt, rm, e := s.room(ctx, tx, w, v.Scope.RoomID)
			if e != nil || rm.Scope != v.Scope {
				return auth.Outcome{}, auth.ErrDenied
			}
			if _, e = participant(ctx, tx.Core(), rt, rm, v); e != nil {
				return auth.Outcome{}, e
			}
			p, e := lt.Preparation(ctx, rm.Scope)
			if e == auth.ErrDenied {
				return auth.RoomOutcome([]byte(`{"ok":true}`)), nil
			}
			if e != nil {
				return auth.Outcome{}, auth.SafeError(e)
			}
			cfg, e := s.config(w, p.StorageValue().ConfigurationID)
			if e != nil {
				return auth.Outcome{}, e
			}
			out = append(out, publicConfiguration(cfg))
		default:
			return auth.Outcome{}, auth.ErrUnauthenticated
		}
		return auth.RoomOutcome([]byte(`{"ok":true}`)), nil
	}, Replay: func(context.Context, auth.Transaction, auth.SessionData, auth.Outcome) error { return auth.ErrDenied }})
	if e != nil {
		return nil, e
	}
	return out, nil
}

func (s *Service) playerLobby(ctx context.Context, tx auth.Transaction, v auth.SessionData, w, r string) (PlayerLobby, error) {
	rt, lt, rm, e := s.room(ctx, tx, w, r)
	if e != nil {
		return PlayerLobby{}, e
	}
	canManage := manager(ctx, tx.Core(), rt, rm, v) == nil
	part, pe := participant(ctx, tx.Core(), rt, rm, v)
	if !canManage && pe != nil {
		return PlayerLobby{}, auth.ErrDenied
	}
	if rm.State != "lobby" && rm.State != "launched" {
		return PlayerLobby{}, auth.ErrDenied
	}
	out := PlayerLobbyData{Scope: rm.Scope}
	out.CanManage = canManage
	p, e := lt.Preparation(ctx, rm.Scope)
	if e == auth.ErrDenied {
		return auth.RoomSecret(out), nil
	}
	if e != nil {
		return PlayerLobby{}, auth.SafeError(e)
	}
	out.Preparation = p.StorageValue()
	cfg, e := s.config(w, out.Preparation.ConfigurationID)
	if e != nil {
		return PlayerLobby{}, e
	}
	hash, e := sessionRequest(cfg, rm.Scope).authenticate(ctx)
	if e != nil || hash != out.Preparation.GraphHash || !validPreparation(out.Preparation, cfg) {
		return PlayerLobby{}, auth.ErrDenied
	}
	out.Preparation.Slots = slices.Clone(out.Preparation.Slots)
	acks, e := lt.Acknowledgments(ctx, rm.Scope)
	if e != nil {
		return PlayerLobby{}, auth.SafeError(e)
	}
	for _, a := range acks {
		x := a.StorageValue()
		if pe == nil && x.ParticipantID == part.ID && x.Scope == rm.Scope && x.Revision == out.Preparation.Revision && x.ConfigurationHash == out.Preparation.ConfigurationHash && x.GraphHash == hash {
			out.Own = x
			out.Own.Boundaries = slices.Clone(x.Boundaries)
		}
	}
	out.Configured = true
	_, e = checkReadiness(ctx, tx.Core(), rt, out.Preparation, cfg, acks, s.state().models)
	out.Ready = e == nil
	// Detailed participant/model/boundary errors are intentionally redacted.
	return auth.RoomSecret(out), nil
}

func (s *Service) PlayerLobby(ctx context.Context, c Caller, w, r string) (PlayerLobby, error) {
	if e := s.liveService(ctx, w, r); e != nil {
		return PlayerLobby{}, e
	}
	var out PlayerLobby
	v := c.StorageValue()
	cmd := auth.RoomSecret(auth.RoomCommandData{Action: "player.preparation", TargetKey: w + "/" + r})
	_, e := s.state().authority.Do(ctx, v.Credential, "", "", v.Network, cmd, auth.RoomCallbacks{Apply: func(ctx context.Context, tx auth.Transaction, v auth.SessionData) (auth.Outcome, error) {
		var e error
		out, e = s.playerLobby(ctx, tx, v, w, r)
		if e != nil {
			return auth.Outcome{}, e
		}
		return auth.RoomOutcome([]byte(`{"ok":true}`)), nil
	}, Replay: func(context.Context, auth.Transaction, auth.SessionData, auth.Outcome) error { return auth.ErrDenied }})
	return out, e
}

// PlayerReferenceWithin can be used only inside the current authenticated
// admission transaction. No caller-provided subject is admitted here.
func (s *Service) PlayerReferenceWithin(ctx context.Context, tx auth.Transaction, v auth.SessionData, w, r string) (PlayerReference, error) {
	if e := s.liveService(ctx, w, r); e != nil {
		return PlayerReference{}, e
	}
	actual, e := auth.RoomAdmissionSession(tx.Core())
	if e != nil || actual.StorageValue().Hash != v.Hash || v.Hash == "" {
		return PlayerReference{}, auth.ErrDenied
	}
	a, e := s.sessionAccess(ctx, tx, v, w, r)
	if e != nil {
		return PlayerReference{}, e
	}
	lobby, e := s.playerLobby(ctx, tx, v, w, r)
	if e != nil {
		return PlayerReference{}, e
	}
	l := lobby.StorageValue()
	cfg, e := s.config(w, l.Preparation.ConfigurationID)
	if e != nil {
		return PlayerReference{}, e
	}
	out := PlayerReferenceData{Access: a.StorageValue(), PreparationRevision: l.Preparation.Revision, Humans: []PlayerHuman{}, Ready: l.Ready, OwnConfirmed: l.Own.Revision == l.Preparation.Revision && l.Own.Consent && l.Own.Ready && l.Own.SafetyConfirmed}
	for _, slot := range l.Preparation.Slots {
		if slot.Mode != "human" {
			continue
		}
		required := false
		for _, rule := range cfg.Seats {
			if rule.ID == slot.ID {
				required = rule.Required
			}
		}
		out.Humans = append(out.Humans, PlayerHuman{Principal: slot.ParticipantID, Seat: slot.ID, Required: required})
	}
	slices.SortFunc(out.Humans, func(a, b PlayerHuman) int { return strings.Compare(a.Principal, b.Principal) })
	return auth.RoomSecret(out), nil
}

func (s *Service) PlayerControlled() bool { return s.state() != nil && s.state().player != nil }
func (s *Service) PlayerAuthority(a *auth.RoomAuthority) bool {
	return s.state() != nil && a != nil && s.state().authority == a
}
func (s *Service) PlayerControlledBy(c PlayerControl) bool {
	return s.state() != nil && c != nil && s.state().player == c
}
func (s *Service) PlayerConfigurations() []PlayerConfiguration {
	if s.state() == nil {
		return nil
	}
	keys := make([]string, 0, len(s.state().configs))
	for k := range s.state().configs {
		keys = append(keys, k)
	}
	slices.Sort(keys)
	out := []PlayerConfiguration{}
	for _, k := range keys {
		out = append(out, publicConfiguration(s.state().configs[k].StorageValue()))
	}
	return out
}
