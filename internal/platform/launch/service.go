// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package launch

import (
	"context"
	"crypto/rand"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/install"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/room"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
	"io"
	"math"
	"slices"
	"strings"
)

type Options struct {
	Context        context.Context
	Authority      *auth.RoomAuthority
	Rooms          room.Storage
	Storage        Storage
	Configurations []Configuration
	Models         ModelChecker
	MaxSessions    int
}
type Service struct{ data **serviceData }
type serviceData struct {
	authority *auth.RoomAuthority
	rooms     room.Storage
	storage   Storage
	configs   map[string]Configuration
	models    ModelChecker
	actors    *Coordinator
}

func (Service) Format(s fmt.State, _ rune)   { _, _ = io.WriteString(s, "<private launch service>") }
func (Service) MarshalJSON() ([]byte, error) { return nil, auth.ErrDenied }
func (s *Service) state() *serviceData {
	if s == nil || s.data == nil {
		return nil
	}
	return *s.data
}
func cloneEvidence(x map[string]install.Evidence) map[string]install.Evidence {
	if x == nil {
		return nil
	}
	out := map[string]install.Evidence{}
	clone := func(v *install.Attestation) *install.Attestation {
		if v == nil {
			return nil
		}
		c := *v
		c.Signature = append([]byte(nil), v.Signature...)
		return &c
	}
	for k, v := range x {
		out[k] = install.Evidence{Publisher: clone(v.Publisher), Certification: clone(v.Certification)}
	}
	return out
}
func New(o Options) (*Service, error) {
	if o.Context == nil || o.Context.Err() != nil || o.Authority == nil || o.Rooms == nil || o.Storage == nil || len(o.Configurations) < 1 || len(o.Configurations) > 128 || o.Storage.RuntimeRepository() == nil || o.Storage.SessionRepository() == nil {
		return nil, auth.ErrInvalid
	}
	d := &serviceData{authority: o.Authority, rooms: o.Rooms, storage: o.Storage, configs: map[string]Configuration{}, models: o.Models}
	for _, v := range o.Configurations {
		c := v.StorageValue()
		if !configuration(c) {
			return nil, auth.ErrInvalid
		}
		key := c.WorkspaceID + "/" + c.ID
		if _, ok := d.configs[key]; ok {
			return nil, auth.ErrInvalid
		}
		c.ContentTags = slices.Clone(c.ContentTags)
		c.SafetyTags = slices.Clone(c.SafetyTags)
		c.Seats = slices.Clone(c.Seats)
		for i := range c.Seats {
			c.Seats[i].Modes = slices.Clone(c.Seats[i].Modes)
			c.Seats[i].ModelCapabilities = slices.Clone(c.Seats[i].ModelCapabilities)
		}
		c.Request.Dependencies = slices.Clone(c.Request.Dependencies)
		c.Request.Evidence = cloneEvidence(c.Request.Evidence)
		epochs := map[string]map[string]install.Evidence{}
		for k, v := range c.Request.EpochEvidence {
			epochs[k] = cloneEvidence(v)
		}
		c.Request.EpochEvidence = epochs
		d.configs[key] = auth.RoomSecret(c)
	}
	a, e := NewCoordinator(o.Context, o.Storage, o.MaxSessions)
	if e != nil {
		return nil, e
	}
	d.actors = a
	return &Service{data: &d}, nil
}
func (s *Service) Close() error {
	if s.state() == nil {
		return nil
	}
	return s.state().actors.Close()
}
func (s *Service) config(w, id string) (ConfigurationData, error) {
	v, ok := s.state().configs[w+"/"+id]
	if !ok {
		return ConfigurationData{}, auth.ErrDenied
	}
	return v.StorageValue(), nil
}
func (s *Service) room(ctx context.Context, tx auth.Transaction, w, id string) (room.Transaction, Transaction, room.RoomData, error) {
	if !store.ValidID(w) || !store.ValidID(id) {
		return nil, nil, room.RoomData{}, auth.ErrInvalid
	}
	r, e := s.state().rooms.Bind(tx.Core())
	if e != nil {
		return nil, nil, room.RoomData{}, auth.SafeError(e)
	}
	l, e := s.state().storage.Bind(tx.Core())
	if e != nil {
		return nil, nil, room.RoomData{}, auth.SafeError(e)
	}
	v, e := r.Room(ctx, w, id)
	if e != nil {
		return nil, nil, room.RoomData{}, auth.SafeError(e)
	}
	d := v.StorageValue()
	if d.Scope.WorkspaceID != w || d.ID != id || d.Scope.RoomID != id || !store.ValidID(d.Scope.GameID) {
		return nil, nil, room.RoomData{}, auth.ErrDenied
	}
	return r, l, d, nil
}
func (s *Service) do(ctx context.Context, c Caller, action, w, id string, request any, calls auth.RoomCallbacks) (auth.Outcome, error) {
	if s.state() == nil || ctx == nil || ctx.Err() != nil || !store.ValidID(w) || !store.ValidID(id) {
		return auth.Outcome{}, auth.ErrInvalid
	}
	raw, e := canonical(request)
	if e != nil {
		return auth.Outcome{}, e
	}
	v := c.StorageValue()
	cmd := auth.RoomSecret(auth.RoomCommandData{Action: action, TargetKey: w + "/" + id, Canonical: raw, Write: true})
	return s.state().authority.Do(ctx, v.Credential, v.CSRF, v.IdempotencyKey, v.Network, cmd, calls)
}

type resultData struct {
	Kind              string
	Scope             coreScope
	Revision          uint64
	ConfigurationHash string
	Binding           data.Binding
}

// A private receipt contains no seat policy, boundary, model selection or token.
type coreScope struct{ WorkspaceID, RoomID, GameID string }

func result(kind string, p PreparationData, b data.Binding) (auth.Outcome, error) {
	raw, e := json.Marshal(resultData{Kind: kind, Scope: coreScope{p.Scope.WorkspaceID, p.Scope.RoomID, p.Scope.GameID}, Revision: p.Revision, ConfigurationHash: p.ConfigurationHash, Binding: b})
	if e != nil {
		return auth.Outcome{}, auth.ErrUnavailable
	}
	return auth.RoomOutcome(raw), nil
}
func sameResult(out auth.Outcome, kind string, p PreparationData, b data.Binding) bool {
	v, e := result(kind, p, b)
	return e == nil && string(v.StorageValue().Body) == string(out.StorageValue().Body)
}

func (s *Service) Configure(ctx context.Context, caller Caller, request ConfigureRequest) (Preparation, error) {
	r := request.StorageValue()
	r.Slots = slices.Clone(r.Slots)
	slices.SortFunc(r.Slots, func(a, b Slot) int { return strings.Compare(a.ID, b.ID) })
	var out Preparation
	apply := func(ctx context.Context, tx auth.Transaction, v auth.SessionData) (auth.Outcome, error) {
		rt, lt, rm, e := s.room(ctx, tx, r.WorkspaceID, r.RoomID)
		if e != nil {
			return auth.Outcome{}, e
		}
		if rm.State != "lobby" {
			return auth.Outcome{}, auth.ErrConflict
		}
		if e = manager(ctx, tx.Core(), rt, rm, v); e != nil {
			return auth.Outcome{}, e
		}
		c, e := s.config(r.WorkspaceID, r.ConfigurationID)
		if e != nil {
			return auth.Outcome{}, e
		}
		hash, e := sessionRequest(c, rm.Scope).authenticate(ctx)
		if e != nil {
			return auth.Outcome{}, e
		}
		revision := uint64(1)
		old, e := lt.Preparation(ctx, rm.Scope)
		if e == nil {
			revision = old.StorageValue().Revision + 1
		} else if e != auth.ErrDenied {
			return auth.Outcome{}, auth.SafeError(e)
		}
		p := PreparationData{Scope: rm.Scope, ConfigurationID: c.ID, ConfigurationHash: configurationHash(c), GraphHash: hash, Revision: revision, Slots: r.Slots}
		if !validPreparation(p, c) {
			return auth.Outcome{}, auth.ErrInvalid
		}
		out = auth.RoomSecret(p)
		if e = lt.PutPreparation(ctx, out); e != nil {
			return auth.Outcome{}, auth.SafeError(e)
		}
		return result("configured", p, data.Binding{})
	}
	replay := func(ctx context.Context, tx auth.Transaction, v auth.SessionData, saved auth.Outcome) error {
		rt, lt, rm, e := s.room(ctx, tx, r.WorkspaceID, r.RoomID)
		if e != nil {
			return e
		}
		if rm.State != "lobby" {
			return auth.ErrConflict
		}
		if e = manager(ctx, tx.Core(), rt, rm, v); e != nil {
			return e
		}
		p, e := lt.Preparation(ctx, rm.Scope)
		if e != nil {
			return auth.SafeError(e)
		}
		d := p.StorageValue()
		if d.ConfigurationID != r.ConfigurationID || !slices.Equal(d.Slots, r.Slots) || !sameResult(saved, "configured", d, data.Binding{}) {
			return auth.ErrConflict
		}
		c, e := s.config(r.WorkspaceID, d.ConfigurationID)
		if e != nil {
			return e
		}
		hash, e := sessionRequest(c, rm.Scope).authenticate(ctx)
		if e != nil || hash != d.GraphHash || !validPreparation(d, c) {
			return auth.ErrDenied
		}
		out = p
		return nil
	}
	_, e := s.do(ctx, caller, "launch.configure", r.WorkspaceID, r.RoomID, r, auth.RoomCallbacks{Apply: apply, Replay: replay})
	if e != nil {
		return Preparation{}, e
	}
	return out, nil
}
func (s *Service) Acknowledge(ctx context.Context, caller Caller, request AcknowledgeRequest) error {
	r := request.StorageValue()
	r.Boundaries = slices.Clone(r.Boundaries)
	slices.Sort(r.Boundaries)
	if r.Revision < 1 || r.Revision >= math.MaxInt64 || !labels(r.Boundaries, 64) {
		return auth.ErrInvalid
	}
	work := func(ctx context.Context, tx auth.Transaction, v auth.SessionData, saved *auth.Outcome) (auth.Outcome, error) {
		rt, lt, rm, e := s.room(ctx, tx, r.WorkspaceID, r.RoomID)
		if e != nil {
			return auth.Outcome{}, e
		}
		if rm.State != "lobby" {
			return auth.Outcome{}, auth.ErrConflict
		}
		part, e := participant(ctx, tx.Core(), rt, rm, v)
		if e != nil {
			return auth.Outcome{}, e
		}
		p, e := lt.Preparation(ctx, rm.Scope)
		if e != nil {
			return auth.Outcome{}, auth.SafeError(e)
		}
		d := p.StorageValue()
		if d.Revision != r.Revision {
			return auth.Outcome{}, auth.ErrConflict
		}
		c, e := s.config(r.WorkspaceID, d.ConfigurationID)
		if e != nil {
			return auth.Outcome{}, e
		}
		hash, e := sessionRequest(c, rm.Scope).authenticate(ctx)
		if e != nil || hash != d.GraphHash || !validPreparation(d, c) {
			return auth.Outcome{}, auth.ErrDenied
		}
		for _, boundary := range r.Boundaries {
			if !slices.Contains(c.SafetyTags, boundary) {
				return auth.Outcome{}, auth.ErrInvalid
			}
		}
		a := AcknowledgmentData{Scope: rm.Scope, ParticipantID: part.ID, ConfigurationHash: d.ConfigurationHash, GraphHash: hash, Revision: d.Revision, Consent: r.Consent, Ready: r.Ready, SafetyConfirmed: r.SafetyConfirmed, Boundaries: r.Boundaries}
		if saved != nil {
			if !sameResult(*saved, "acknowledged", d, data.Binding{}) {
				return auth.Outcome{}, auth.ErrConflict
			}
			acks, e := lt.Acknowledgments(ctx, rm.Scope)
			if e != nil {
				return auth.Outcome{}, auth.SafeError(e)
			}
			found := false
			for _, old := range acks {
				x := old.StorageValue()
				if x.ParticipantID == a.ParticipantID {
					one, _ := json.Marshal(x)
					two, _ := json.Marshal(a)
					found = string(one) == string(two)
				}
			}
			if !found {
				return auth.Outcome{}, auth.ErrConflict
			}
		} else if e = lt.PutAcknowledgment(ctx, auth.RoomSecret(a)); e != nil {
			return auth.Outcome{}, auth.SafeError(e)
		}
		return result("acknowledged", d, data.Binding{})
	}
	_, e := s.do(ctx, caller, "launch.acknowledge", r.WorkspaceID, r.RoomID, r, auth.RoomCallbacks{Apply: func(c context.Context, t auth.Transaction, v auth.SessionData) (auth.Outcome, error) {
		return work(c, t, v, nil)
	}, Replay: func(c context.Context, t auth.Transaction, v auth.SessionData, o auth.Outcome) error {
		_, e := work(c, t, v, &o)
		return e
	}})
	return e
}
func (s *Service) Launch(ctx context.Context, caller Caller, request LaunchRequest) (Session, error) {
	r := request.StorageValue()
	if r.Revision < 1 || r.Revision >= math.MaxInt64 {
		return Session{}, auth.ErrInvalid
	}
	var answer Session
	var reserved *Reservation
	var cfg ConfigurationData
	var bootstrap string
	defer func() {
		if reserved != nil {
			reserved.Cancel()
		}
	}()
	work := func(ctx context.Context, tx auth.Transaction, v auth.SessionData, saved *auth.Outcome) (auth.Outcome, error) {
		rt, lt, rm, e := s.room(ctx, tx, r.WorkspaceID, r.RoomID)
		if e != nil {
			return auth.Outcome{}, e
		}
		if rm.State != "lobby" && rm.State != "launched" {
			return auth.Outcome{}, auth.ErrConflict
		}
		part, e := participant(ctx, tx.Core(), rt, rm, v)
		if e != nil || !part.Host {
			return auth.Outcome{}, auth.ErrDenied
		}
		p, e := lt.Preparation(ctx, rm.Scope)
		if e != nil {
			return auth.Outcome{}, auth.SafeError(e)
		}
		d := p.StorageValue()
		if d.Revision != r.Revision {
			return auth.Outcome{}, auth.ErrConflict
		}
		cfg, e = s.config(r.WorkspaceID, d.ConfigurationID)
		if e != nil {
			return auth.Outcome{}, e
		}
		hash, e := sessionRequest(cfg, rm.Scope).authenticate(ctx)
		if e != nil || hash != d.GraphHash {
			return auth.Outcome{}, auth.ErrDenied
		}
		acks, e := lt.Acknowledgments(ctx, rm.Scope)
		if e != nil {
			return auth.Outcome{}, auth.SafeError(e)
		}
		bootstrap, e = checkReadiness(ctx, tx.Core(), rt, d, cfg, acks, s.state().models)
		if e != nil {
			return auth.Outcome{}, e
		}
		b := data.Binding{Workspace: rm.Scope.WorkspaceID, GraphHash: d.GraphHash}
		existing, e := lt.Session(ctx, rm.Scope)
		if e == nil {
			x := existing.StorageValue()
			b.Session = x.Binding.Session
			if rm.State != "launched" || x.Binding != b || x.ConfigurationID != d.ConfigurationID || x.ConfigurationHash != d.ConfigurationHash || x.Revision != d.Revision {
				return auth.Outcome{}, auth.ErrConflict
			}
			answer = existing
		} else {
			if e != auth.ErrDenied {
				return auth.Outcome{}, auth.SafeError(e)
			}
			if rm.State != "lobby" || saved != nil {
				return auth.Outcome{}, auth.ErrConflict
			}
			var nonce [16]byte
			if _, e = rand.Read(nonce[:]); e != nil {
				return auth.Outcome{}, auth.ErrUnavailable
			}
			b.Session = "session-" + hex.EncodeToString(nonce[:])
			clear(nonce[:])
			reserved, e = s.state().actors.Reserve(b)
			if e != nil {
				return auth.Outcome{}, e
			}
			repo, e := lt.SessionRepository(rm.Scope, b)
			if e != nil {
				return auth.Outcome{}, auth.SafeError(e)
			}
			f, e := cfg.Factory.WithRepository(repo)
			if e != nil {
				return auth.Outcome{}, auth.ErrUnavailable
			}
			q := cfg.Request
			q.Workspace = b.Workspace
			q.Session = b.Session
			created, e := f.Create(ctx, q)
			if e != nil {
				return auth.Outcome{}, auth.ErrDenied
			}
			matches := created.Binding == b
			closeErr := created.Close()
			if !matches || closeErr != nil {
				return auth.Outcome{}, auth.ErrUnavailable
			}
			answer = auth.RoomSecret(SessionData{Scope: rm.Scope, ConfigurationID: d.ConfigurationID, ConfigurationHash: d.ConfigurationHash, Revision: d.Revision, Binding: b})
			if e = lt.PutSession(ctx, answer); e != nil {
				return auth.Outcome{}, auth.SafeError(e)
			}
		}
		if saved != nil && !sameResult(*saved, "launched", d, b) {
			return auth.Outcome{}, auth.ErrConflict
		}
		return result("launched", d, b)
	}
	_, e := s.do(ctx, caller, "launch.start", r.WorkspaceID, r.RoomID, r, auth.RoomCallbacks{Apply: func(c context.Context, t auth.Transaction, v auth.SessionData) (auth.Outcome, error) {
		return work(c, t, v, nil)
	}, Replay: func(c context.Context, t auth.Transaction, v auth.SessionData, o auth.Outcome) error {
		_, e := work(c, t, v, &o)
		return e
	}})
	if e != nil {
		return Session{}, e
	}
	// Only the fully committed creation and private receipt activate an Actor.
	if e = s.state().actors.Activate(ctx, reserved, answer.StorageValue().Binding, auth.RoomSecret(cfg), bootstrap); e != nil {
		return Session{}, auth.ErrOutcomeUnknown
	}
	return answer, nil
}
