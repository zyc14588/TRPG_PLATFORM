// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package player

import (
	"bytes"
	"context"
	"crypto/rand"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"io"
	"math"
	"sync"
	"time"

	"github.com/santhosh-tekuri/jsonschema/v6"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/launch"
	platformsession "github.com/zyc14588/TRPG_PLATFORM/internal/platform/session"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/command"
)

type Description struct{ WorkspaceID, ConfigurationID, GameID, Title string }
type Options struct {
	Context        context.Context
	Authority      *auth.RoomAuthority
	Launch         *launch.Service
	Sessions       *platformsession.Service
	Control        *Control
	Descriptions   []Description
	Schema         []byte
	MaxConnections int
}
type Service struct{ data **serviceData }
type serviceData struct {
	options     Options
	schemas     map[string]*jsonschema.Schema
	games       map[string]Game
	mu          sync.Mutex
	connections map[string]*connection
	reserved    int
	closed      bool
}
type connection struct {
	native *platformsession.Connection
	lease  Lease
}

func (Service) Format(f fmt.State, _ rune)   { _, _ = io.WriteString(f, "<private player facade>") }
func (Service) MarshalJSON() ([]byte, error) { return nil, auth.ErrDenied }
func (s *Service) state() *serviceData {
	if s == nil || s.data == nil {
		return nil
	}
	return *s.data
}
func New(o Options) (*Service, error) {
	if o.Context == nil || o.Context.Err() != nil || o.Authority == nil || o.Launch == nil || o.Sessions == nil || o.Control.state() == nil || !o.Launch.PlayerAuthority(o.Authority) || !o.Launch.PlayerControlledBy(o.Control) || !o.Sessions.UsesLaunch(o.Launch) || o.MaxConnections < 1 || o.MaxConnections > 256 || len(o.Descriptions) < 1 || len(o.Descriptions) > 128 {
		return nil, auth.ErrInvalid
	}
	schemas, e := compile(o.Schema)
	if e != nil {
		return nil, e
	}
	d := &serviceData{options: o, schemas: schemas, games: map[string]Game{}, connections: map[string]*connection{}}
	registered := map[string]launch.PlayerConfigurationData{}
	for _, x := range o.Launch.PlayerConfigurations() {
		c := x.StorageValue()
		registered[c.WorkspaceID+"/"+c.ID] = c
	}
	for _, x := range o.Descriptions {
		key := x.WorkspaceID + "/" + x.ConfigurationID
		c, ok := registered[key]
		if !ok || !store.ValidID(x.GameID) {
			return nil, auth.ErrInvalid
		}
		if _, ok = d.games[key]; ok {
			return nil, auth.ErrInvalid
		}
		g := Game{GameID: x.GameID, ConfigurationID: x.ConfigurationID, Title: x.Title, ContentTags: append([]string{}, c.ContentTags...), SafetyTags: append([]string{}, c.SafetyTags...), Seats: []Seat{}}
		for _, r := range c.Seats {
			g.Seats = append(g.Seats, Seat{r.ID, r.Required, append([]string{}, r.Modes...)})
		}
		raw, e := json.Marshal(g)
		if e != nil {
			return nil, auth.ErrInvalid
		}
		v, e := auth.RoomJSON(raw)
		if e != nil || schemas["Game"].Validate(v) != nil {
			return nil, auth.ErrInvalid
		}
		d.games[key] = g
	}
	// Every registered configuration has explicit trusted catalog metadata.
	if len(registered) != len(d.games) {
		return nil, auth.ErrInvalid
	}
	d.options.Schema = nil
	d.options.Descriptions = nil
	return &Service{data: &d}, nil
}
func (s *Service) Authentication() *auth.Service {
	if s.state() == nil {
		return nil
	}
	return s.state().options.Authority.Authentication()
}
func (s *Service) Close() {
	if s.state() == nil {
		return
	}
	d := s.state()
	d.mu.Lock()
	d.closed = true
	for k, c := range d.connections {
		c.native.Close()
		delete(d.connections, k)
	}
	d.mu.Unlock()
}
func (s *Service) prune() {
	d := s.state()
	now := time.Now()
	d.mu.Lock()
	defer d.mu.Unlock()
	for k, c := range d.connections {
		l := c.lease.StorageValue()
		if l.Closed || !now.Before(l.ExpiresAt) {
			c.native.Close()
			delete(d.connections, k)
		}
	}
}
func (s *Service) remove(id string) {
	d := s.state()
	d.mu.Lock()
	defer d.mu.Unlock()
	if c := d.connections[id]; c != nil {
		c.native.Close()
		delete(d.connections, id)
	}
}
func (s *Service) native(ctx context.Context, c launch.Caller, ref launch.PlayerReference, l Lease, after uint64) (*platformsession.Connection, error) {
	d := s.state()
	ld := l.StorageValue()
	d.mu.Lock()
	// A successfully admitted replacement retires only this cookie's old
	// native handle. Other humans keep their independently issued leases.
	for id, old := range d.connections {
		x := old.lease.StorageValue()
		if id != ld.ID && x.Binding == ld.Binding && x.CookieHash == ld.CookieHash && x.Principal == ld.Principal {
			old.native.Close()
			delete(d.connections, id)
		}
	}
	if old := d.connections[ld.ID]; old != nil {
		x := old.lease.StorageValue()
		if x.Binding != ld.Binding || x.Scope != ld.Scope || x.CookieHash != ld.CookieHash || x.Principal != ld.Principal || x.Seat != ld.Seat {
			d.mu.Unlock()
			return nil, auth.ErrDenied
		}
		old.lease = l
		native := old.native
		d.mu.Unlock()
		return native, nil
	}
	if d.closed || len(d.connections)+d.reserved >= d.options.MaxConnections {
		d.mu.Unlock()
		return nil, auth.ErrRateLimited
	}
	d.reserved++
	d.mu.Unlock()
	x, e := d.options.Sessions.ConnectPolling(ctx, c, ref.StorageValue().Access.Scope.WorkspaceID, ref.StorageValue().Access.Scope.RoomID, after)
	d.mu.Lock()
	defer d.mu.Unlock()
	d.reserved--
	if e != nil {
		return nil, platformsession.SafeError(e)
	}
	if d.closed {
		x.Close()
		return nil, auth.ErrUnavailable
	}
	if old := d.connections[ld.ID]; old != nil {
		x.Close()
		old.lease = l
		return old.native, nil
	}
	d.connections[ld.ID] = &connection{native: x, lease: l}
	return x, nil
}

// A failed native connection must not leave a durable online seat behind.
// This cleanup uses the exact server-issued ownership tuple and still pauses
// required humans through the same durable state transition.
func (s *Service) abandon(ctx context.Context, a admission) error {
	bounded, cancel := context.WithTimeout(context.WithoutCancel(ctx), 3*time.Second)
	defer cancel()
	d := s.state()
	want := a.lease.StorageValue()
	var state State
	e := d.options.Control.state().storage.Transact(bounded, func(tx Transaction) error {
		if e := tx.Barrier(bounded, want.Binding); e != nil {
			return e
		}
		leases, e := tx.Leases(bounded, want.Binding)
		if e != nil {
			return e
		}
		for _, l := range leases {
			v := l.StorageValue()
			if v.ID != want.ID {
				continue
			}
			if v.Binding != want.Binding || v.Scope != want.Scope || v.CookieHash != want.CookieHash || v.Principal != want.Principal || v.Seat != want.Seat {
				return auth.ErrDenied
			}
			v.Closed = true
			if e = tx.PutLease(bounded, auth.RoomSecret(v)); e != nil {
				return e
			}
		}
		state, e = d.options.Control.ReadWithin(bounded, tx, a.ref)
		return e
	})
	s.remove(want.ID)
	if e == nil && state.StorageValue().Quiescing {
		e = d.options.Control.Quiesce(bounded, want.Binding)
	}
	return e
}

type admission struct {
	ref   launch.PlayerReference
	state State
	lease Lease
}
type admissionReceipt struct {
	ConnectionID    string `json:"connection_id,omitempty"`
	ControlRevision uint64 `json:"control_revision,omitempty"`
	Admitted        bool   `json:"admitted"`
	Refused         string `json:"refused,omitempty"`
}

func encodeAdmission(v admissionReceipt) auth.Outcome {
	b, _ := json.Marshal(v)
	return auth.RoomOutcome(b)
}
func (s *Service) admit(ctx context.Context, c launch.Caller, r RequestData) (admission, error) {
	d := s.state()
	cv := c.StorageValue()
	out := admission{}
	refused := error(nil)
	write := requestSchemas[r.Action] != ""
	cmd := auth.RoomSecret(auth.RoomCommandData{Action: "player." + r.Action, TargetKey: r.WorkspaceID + "/" + r.RoomID, Canonical: r.Canonical, Write: write})
	work := func(ctx context.Context, tx auth.Transaction, v auth.SessionData, saved *auth.Outcome) (auth.Outcome, error) {
		ref, e := d.options.Launch.PlayerReferenceWithin(ctx, tx, v, r.WorkspaceID, r.RoomID)
		if e != nil {
			return auth.Outcome{}, e
		}
		out.ref = ref
		a := ref.StorageValue().Access
		pt, e := d.options.Control.Bind(tx.Core())
		if e != nil {
			return auth.Outcome{}, auth.SafeError(e)
		}
		out.state, e = d.options.Control.ReadWithin(ctx, pt, ref)
		if e != nil {
			return auth.Outcome{}, e
		}
		receipt := admissionReceipt{Admitted: true}
		if saved != nil {
			if checkpoint.StrictDecode(saved.StorageValue().Body, &receipt, 4096) != nil || !receipt.Admitted {
				return auth.Outcome{}, auth.ErrDenied
			}
		}
		if receipt.Refused != "" {
			switch receipt.Refused {
			case "DENIED":
				refused = auth.ErrDenied
			case "CONFLICT":
				refused = auth.ErrConflict
			case "PLAYER_PAUSED":
				refused = ErrPaused
			case "PLAYER_CONNECTION_EXPIRED":
				refused = ErrConnectionExpired
			default:
				return auth.Outcome{}, auth.ErrDenied
			}
			return encodeAdmission(receipt), nil
		}
		switch r.Action {
		case "pause", "resume":
			expected, _ := counter(r.Fields["expected_control_revision"].(string))
			if saved != nil {
				if out.state.StorageValue().Revision != receipt.ControlRevision {
					refused = auth.ErrConflict
				}
			} else {
				var changed State
				if r.Action == "pause" {
					changed, e = d.options.Control.PauseWithin(ctx, pt, ref, expected)
				} else {
					changed, e = d.options.Control.ResumeWithin(ctx, pt, ref, expected)
				}
				if e == nil {
					out.state = changed
				}
				if e != nil {
					if e == auth.ErrConflict || e == auth.ErrDenied {
						refused = e
					} else {
						return auth.Outcome{}, e
					}
				} else {
					receipt.ControlRevision = out.state.StorageValue().Revision
				}
			}
		case "connect", "snapshot", "command", "disconnect":
			leases, e := pt.Leases(ctx, a.Binding)
			if e != nil {
				return auth.Outcome{}, auth.SafeError(e)
			}
			now, e := pt.Now(ctx)
			if e != nil {
				return auth.Outcome{}, auth.SafeError(e)
			}
			id := ""
			if r.Action != "connect" {
				id = r.Fields["connection_id"].(string)
			} else if saved != nil {
				id = receipt.ConnectionID
			}
			if id != "" {
				for _, l := range leases {
					if l.StorageValue().ID == id {
						out.lease = l
					}
				}
				l := out.lease.StorageValue()
				if l.ID == "" || l.Binding != a.Binding || l.Scope != a.Scope || l.CookieHash != v.Hash || l.Principal != a.Principal || l.Seat != a.Seat {
					refused = auth.ErrDenied
					break
				}
				if l.Closed || !now.Before(l.ExpiresAt) {
					if r.Action == "disconnect" && saved != nil && l.Closed && out.state.StorageValue().Revision == receipt.ControlRevision {
						break
					}
					refused = ErrConnectionExpired
					break
				}
				if r.Action == "disconnect" {
					l.Closed = true
					if e = pt.PutLease(ctx, auth.RoomSecret(l)); e != nil {
						return auth.Outcome{}, auth.SafeError(e)
					}
					out.lease = auth.RoomSecret(l)
					out.state, e = d.options.Control.ReadWithin(ctx, pt, ref)
					if e != nil {
						return auth.Outcome{}, e
					}
					receipt.ControlRevision = out.state.StorageValue().Revision
				} else {
					l.ExpiresAt = now.Add(LeaseLifetime)
					if v.ExpiresAt.Before(l.ExpiresAt) {
						l.ExpiresAt = v.ExpiresAt
					}
					out.lease = auth.RoomSecret(l)
					if e = pt.PutLease(ctx, out.lease); e != nil {
						return auth.Outcome{}, auth.SafeError(e)
					}
				}
			} else if r.Action == "connect" && saved == nil {
				var nonce [16]byte
				if _, e = rand.Read(nonce[:]); e != nil {
					return auth.Outcome{}, auth.ErrUnavailable
				}
				id = "connection-" + hex.EncodeToString(nonce[:])
				clear(nonce[:])
				// A new connection replaces only this cookie's old seat lease.
				active := 0
				for _, l := range leases {
					x := l.StorageValue()
					if !x.Closed && now.Before(x.ExpiresAt) {
						if x.CookieHash == v.Hash && x.Principal == a.Principal {
							x.Closed = true
							if e = pt.PutLease(ctx, auth.RoomSecret(x)); e != nil {
								return auth.Outcome{}, auth.SafeError(e)
							}
						} else {
							active++
						}
					}
				}
				if active >= MaxRoomConnections {
					return auth.Outcome{}, auth.ErrRateLimited
				}
				l := LeaseData{Scope: a.Scope, Binding: a.Binding, ID: id, CookieHash: v.Hash, Principal: a.Principal, Seat: a.Seat, ExpiresAt: now.Add(LeaseLifetime)}
				if v.ExpiresAt.Before(l.ExpiresAt) {
					l.ExpiresAt = v.ExpiresAt
				}
				out.lease = auth.RoomSecret(l)
				receipt.ConnectionID = id
				if e = pt.PutLease(ctx, out.lease); e != nil {
					return auth.Outcome{}, auth.SafeError(e)
				}
			} else {
				refused = auth.ErrDenied
			}
		}
		if (r.Action == "command" || r.Action == "create_point") && out.state.StorageValue().Paused {
			refused = ErrPaused
		}
		if refused != nil {
			receipt.Refused = refused.Error()
		}
		// Refusals caused by expiry occur outside this transaction so the durable
		// pause survives. Authentication/schema/ownership still fail closed.
		return encodeAdmission(receipt), nil
	}
	_, e := d.options.Authority.Do(ctx, cv.Credential, cv.CSRF, cv.IdempotencyKey, cv.Network, cmd, auth.RoomCallbacks{Apply: func(ctx context.Context, tx auth.Transaction, v auth.SessionData) (auth.Outcome, error) {
		return work(ctx, tx, v, nil)
	}, Replay: func(ctx context.Context, tx auth.Transaction, v auth.SessionData, saved auth.Outcome) error {
		_, e := work(ctx, tx, v, &saved)
		return e
	}})
	if e != nil {
		return admission{}, e
	}
	if refused != nil {
		return out, refused
	}
	return out, nil
}
func (s *Service) currentControl(ctx context.Context, c launch.Caller, ref launch.PlayerReference) (State, error) {
	basis := ref.StorageValue()
	var state State
	cv := c.StorageValue()
	d := s.state()
	e := d.options.Authority.Inspect(ctx, cv.Credential, cv.CSRF, false, func(ctx context.Context, tx auth.Transaction, v auth.SessionData) error {
		r, e := d.options.Launch.PlayerReferenceWithin(ctx, tx, v, basis.Access.Scope.WorkspaceID, basis.Access.Scope.RoomID)
		if e != nil {
			return e
		}
		a := r.StorageValue()
		if a.Access.Binding != basis.Access.Binding || a.Access.Principal != basis.Access.Principal || a.Access.Seat != basis.Access.Seat || a.PreparationRevision != basis.PreparationRevision {
			return auth.ErrDenied
		}
		t, e := d.options.Control.Bind(tx.Core())
		if e != nil {
			return e
		}
		state, e = d.options.Control.ReadWithin(ctx, t, r)
		return e
	})
	return state, e
}
func (s *Service) Do(ctx context.Context, c launch.Caller, request Request) (auth.Outcome, error) {
	if s.state() == nil || ctx == nil || ctx.Err() != nil || s.state().options.Context.Err() != nil {
		return auth.Outcome{}, auth.ErrUnavailable
	}
	d := s.state()
	d.mu.Lock()
	closed := d.closed
	d.mu.Unlock()
	if closed {
		return auth.Outcome{}, auth.ErrUnavailable
	}
	r := request.StorageValue()
	// Re-decode the frozen canonical bytes rather than trusting mutable maps
	// retained by an internal caller of the private storage-value seam.
	fields, e := auth.CanonicalRoomFields(r.Fields)
	if e != nil || !bytes.Equal(fields, r.Canonical) {
		return auth.Outcome{}, auth.ErrInvalid
	}
	body := r.Canonical
	if requestSchemas[r.Action] == "" {
		if len(r.Fields) != 0 {
			return auth.Outcome{}, auth.ErrInvalid
		}
		body = nil
	}
	sealed, e := s.Decode(r.Action, r.WorkspaceID, r.RoomID, body)
	if e != nil {
		return auth.Outcome{}, e
	}
	r = sealed.StorageValue()
	ctx, cancel := context.WithTimeout(ctx, 5*time.Second)
	defer cancel()
	s.prune()
	switch r.Action {
	case "catalog":
		configs, e := d.options.Launch.PlayerCatalog(ctx, c, r.WorkspaceID)
		if e != nil {
			return auth.Outcome{}, e
		}
		games := []Game{}
		for _, x := range configs {
			cfg := x.StorageValue()
			g, ok := d.games[cfg.WorkspaceID+"/"+cfg.ID]
			if !ok {
				return auth.Outcome{}, auth.ErrDenied
			}
			games = append(games, g)
		}
		return s.response(r.Action, struct {
			Games []Game `json:"games"`
		}{games})
	case "lobby", "readiness":
		v, e := d.options.Launch.PlayerLobby(ctx, c, r.WorkspaceID, r.RoomID)
		if e != nil {
			return auth.Outcome{}, e
		}
		if !validCounters(v.StorageValue().Preparation.Revision, v.StorageValue().Own.Revision) {
			return auth.Outcome{}, auth.ErrUnavailable
		}
		return s.response(r.Action, lobbyDTO(v))
	case "configure":
		l, e := d.options.Launch.PlayerLobby(ctx, c, r.WorkspaceID, r.RoomID)
		if e != nil {
			return auth.Outcome{}, e
		}
		cfg := r.Fields["configuration_id"].(string)
		g, ok := d.games[r.WorkspaceID+"/"+cfg]
		if !ok || g.GameID != l.StorageValue().Scope.GameID {
			return auth.Outcome{}, auth.ErrDenied
		}
		slots := []launch.Slot{}
		for _, raw := range r.Fields["slots"].([]any) {
			x := raw.(map[string]any)
			slot := launch.Slot{ID: x["id"].(string), Mode: x["mode"].(string)}
			slot.ParticipantID, _ = x["participant_id"].(string)
			slot.ModelSelection, _ = x["model_selection"].(string)
			slots = append(slots, slot)
		}
		_, e = d.options.Launch.Configure(ctx, c, auth.RoomSecret(launch.ConfigureData{WorkspaceID: r.WorkspaceID, RoomID: r.RoomID, ConfigurationID: cfg, Slots: slots}))
		if e != nil {
			return auth.Outcome{}, e
		}
		l, e = d.options.Launch.PlayerLobby(ctx, c, r.WorkspaceID, r.RoomID)
		if e != nil {
			return auth.Outcome{}, e
		}
		if !validCounters(l.StorageValue().Preparation.Revision, l.StorageValue().Own.Revision) {
			return auth.Outcome{}, auth.ErrUnavailable
		}
		return s.response(r.Action, lobbyDTO(l))
	case "consent":
		rev, _ := counter(r.Fields["revision"].(string))
		boundaries := []string{}
		for _, x := range r.Fields["boundaries"].([]any) {
			boundaries = append(boundaries, x.(string))
		}
		e := d.options.Launch.Acknowledge(ctx, c, auth.RoomSecret(launch.AcknowledgeData{WorkspaceID: r.WorkspaceID, RoomID: r.RoomID, Revision: rev, Consent: r.Fields["consent"].(bool), Ready: r.Fields["ready"].(bool), SafetyConfirmed: r.Fields["safety_confirmed"].(bool), Boundaries: boundaries}))
		if e != nil {
			return auth.Outcome{}, e
		}
		return s.response(r.Action, struct {
			Applied bool `json:"applied"`
		}{true})
	case "launch":
		rev, _ := counter(r.Fields["revision"].(string))
		v, e := d.options.Launch.Launch(ctx, c, auth.RoomSecret(launch.LaunchData{WorkspaceID: r.WorkspaceID, RoomID: r.RoomID, Revision: rev}))
		if e != nil {
			return auth.Outcome{}, e
		}
		if _, e = s.admit(ctx, c, r); e != nil {
			return auth.Outcome{}, auth.ErrOutcomeUnknown
		}
		if !validCounters(v.StorageValue().Revision) {
			return auth.Outcome{}, auth.ErrOutcomeUnknown
		}
		return s.response(r.Action, struct {
			SessionID string `json:"session_id"`
			Revision  string `json:"revision"`
		}{v.StorageValue().Binding.Session, decimal(v.StorageValue().Revision)})
	}
	a, e := s.admit(ctx, c, r)
	if a.state.StorageValue().Quiescing {
		if qe := d.options.Control.Quiesce(ctx, a.ref.StorageValue().Access.Binding); qe != nil {
			return auth.Outcome{}, qe
		}
	}
	if e != nil {
		return auth.Outcome{}, SafeError(e)
	}
	ref := a.ref.StorageValue()
	ctl := controlDTO(a.state, ref.Access.Principal)
	if r.Action == "pause" || r.Action == "disconnect" {
		if r.Action == "disconnect" {
			s.remove(r.Fields["connection_id"].(string))
		}
		if a.state.StorageValue().Paused {
			if e = d.options.Control.Quiesce(ctx, ref.Access.Binding); e != nil {
				return auth.Outcome{}, e
			}
		}
		return s.response(r.Action, ctl)
	}
	if r.Action == "resume" {
		return s.response(r.Action, ctl)
	}
	if r.Action == "connect" || r.Action == "snapshot" || r.Action == "command" {
		after := uint64(0)
		if x, ok := r.Fields["after_cursor"].(string); ok {
			after, _ = counter(x)
		}
		native, e := s.native(ctx, c, a.ref, a.lease, after)
		if e != nil {
			if s.abandon(ctx, a) != nil {
				return auth.Outcome{}, auth.ErrOutcomeUnknown
			}
			return auth.Outcome{}, e
		}
		ctx = withNativeLease(ctx, a.lease)
		switch r.Action {
		case "connect":
			state, e := s.currentControl(ctx, c, a.ref)
			if e != nil {
				return auth.Outcome{}, e
			}
			ctl = controlDTO(state, ref.Access.Principal)
			seconds := int(time.Until(a.lease.StorageValue().ExpiresAt) / time.Second)
			if seconds < 1 {
				return auth.Outcome{}, ErrConnectionExpired
			}
			if seconds > 30 {
				seconds = 30
			}
			return s.response(r.Action, ConnectionDTO{a.lease.StorageValue().ID, ref.Access.Binding.Session, ref.Access.Seat, seconds, ctl})
		case "snapshot":
			limit, _ := integer(r.Fields["limit"])
			page, e := native.SnapshotPage(ctx, after, limit)
			if e != nil {
				return auth.Outcome{}, platformsession.SafeError(e)
			}
			x := page.StorageValue()
			events, e := eventDTO(x.Events)
			if e != nil || !validCounters(x.Version, x.Cursor, x.NextCursor) || checkpoint.Validate(x.View) != nil {
				return auth.Outcome{}, auth.ErrUnavailable
			}
			state, e := s.currentControl(ctx, c, a.ref)
			if e != nil {
				return auth.Outcome{}, e
			}
			ctl = controlDTO(state, ref.Access.Principal)
			return s.response(r.Action, Snapshot{x.Binding.Session, x.Seat, decimal(x.Version), decimal(x.NextCursor), x.View, events, x.More, x.Ended, ctl})
		case "command":
			version, _ := counter(r.Fields["expected_state_version"].(string))
			raw, _ := json.Marshal(r.Fields["payload"])
			var payload checkpoint.Value
			if checkpoint.StrictDecode(raw, &payload, MaxRequestBytes) != nil {
				return auth.Outcome{}, auth.ErrInvalid
			}
			out, e := native.Submit(ctx, command.Envelope{SessionID: ref.Access.Binding.Session, SeatID: ref.Access.Seat, CommandID: r.Fields["command_id"].(string), ExpectedStateVersion: version, Type: r.Fields["type"].(string), Payload: payload, CorrelationID: r.Fields["correlation_id"].(string)})
			if e != nil {
				if ctx.Err() != nil {
					return auth.Outcome{}, auth.ErrOutcomeUnknown
				}
				state, ce := s.currentControl(ctx, c, a.ref)
				if ce == nil && state.StorageValue().Paused {
					return auth.Outcome{}, ErrPaused
				}
				return auth.Outcome{}, SafeError(platformsession.SafeError(e))
			}
			x := out.StorageValue()
			if x.Version >= math.MaxInt64 || x.Cursor >= math.MaxInt64 || checkpoint.Validate(x.Result) != nil {
				return auth.Outcome{}, auth.ErrUnavailable
			}
			state, e := s.currentControl(ctx, c, a.ref)
			if e != nil {
				return auth.Outcome{}, auth.ErrOutcomeUnknown
			}
			ctl = controlDTO(state, ref.Access.Principal)
			return s.response(r.Action, Result{x.CommandID, decimal(x.Version), decimal(x.Cursor), x.Replayed, x.Result, ctl})
		}
	}
	native, e := d.options.Sessions.ConnectPolling(ctx, c, r.WorkspaceID, r.RoomID, 0)
	if e != nil {
		return auth.Outcome{}, platformsession.SafeError(e)
	}
	defer native.Close()
	if r.Action == "export" {
		after, _ := counter(r.Fields["after_cursor"].(string))
		limit, _ := integer(r.Fields["limit"])
		x, e := native.Export(ctx, platformsession.ExportKind(r.Fields["kind"].(string)), after, limit)
		if e != nil {
			return auth.Outcome{}, platformsession.SafeError(e)
		}
		v := x.StorageValue()
		events, e := eventDTO(v.Events)
		if e != nil || !validCounters(v.Version, v.Cursor, v.NextCursor) || checkpoint.Validate(v.View) != nil {
			return auth.Outcome{}, auth.ErrUnavailable
		}
		return s.response(r.Action, Export{v.FormatVersion, v.Kind, v.Binding.Session, v.Seat, decimal(v.Version), decimal(v.Cursor), decimal(v.NextCursor), v.More, v.Ended, v.View, events})
	}
	var point platformsession.Point
	if r.Action == "create_point" {
		point, e = native.CreateRecoveryPoint(ctx)
	} else if r.Action == "read_point" {
		point, e = native.RecoveryPoint(ctx)
	} else {
		return auth.Outcome{}, auth.ErrInvalid
	}
	if e != nil {
		return auth.Outcome{}, platformsession.SafeError(e)
	}
	v := point.StorageValue()
	if !validCounters(v.Version, v.Cursor) {
		return auth.Outcome{}, auth.ErrUnavailable
	}
	return s.response(r.Action, RecoveryPoint{v.Binding.Session, decimal(v.Version), decimal(v.Cursor)})
}
