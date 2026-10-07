// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package room

import (
	"context"
	"crypto/hmac"
	"crypto/rand"
	"crypto/sha256"
	"encoding/base64"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"io"
	"time"

	"github.com/santhosh-tekuri/jsonschema/v6"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
)

type Service struct{ data **serviceData }
type serviceData struct {
	authority *auth.RoomAuthority
	storage   Storage
	key       []byte
	schemas   map[string]*jsonschema.Schema
}

func NewService(authority *auth.RoomAuthority, storage Storage, key, schema []byte) (*Service, error) {
	if authority == nil || storage == nil || !authority.InvitationKeyMatches(key) {
		return nil, auth.ErrInvalid
	}
	schemas, e := compile(schema)
	if e != nil {
		return nil, e
	}
	d := &serviceData{authority: authority, storage: storage, key: append([]byte(nil), key...), schemas: schemas}
	return &Service{data: &d}, nil
}
func (s *Service) state() *serviceData {
	if s == nil || s.data == nil {
		return nil
	}
	return *s.data
}
func (*Service) String() string               { return "<private room service>" }
func (*Service) GoString() string             { return "<private room service>" }
func (*Service) Format(f fmt.State, _ rune)   { _, _ = io.WriteString(f, "<private room service>") }
func (*Service) MarshalJSON() ([]byte, error) { return nil, auth.ErrDenied }
func (s *Service) Authentication() *auth.Service {
	if s.state() == nil {
		return nil
	}
	return s.state().authority.Authentication()
}

func randomToken() (string, error) {
	b := make([]byte, 32)
	if _, e := rand.Read(b); e != nil {
		return "", auth.ErrUnavailable
	}
	return base64.RawURLEncoding.EncodeToString(b), nil
}
func randomID() (string, error) {
	v, e := randomToken()
	if e != nil {
		return "", e
	}
	return "r" + v, nil
}
func randomCode() (string, error) {
	var b [10]byte
	if _, e := rand.Read(b[:]); e != nil {
		return "", auth.ErrUnavailable
	}
	const alphabet = "0123456789ABCDEFGHJKMNPQRSTVWXYZ"
	out := make([]byte, 16)
	var bits uint32
	var n uint
	i := 0
	for _, v := range b {
		bits = (bits << 8) | uint32(v)
		n += 8
		for n >= 5 {
			n -= 5
			out[i] = alphabet[(bits>>n)&31]
			i++
		}
	}
	return string(out), nil
}
func keyedHash(key []byte, kind, value string) string {
	h := hmac.New(sha256.New, key)
	_, _ = h.Write([]byte("room-v1\x00" + kind + "\x00" + value))
	return hex.EncodeToString(h.Sum(nil))
}
func (s *Service) hash(kind, value string) string { return keyedHash(s.state().key, kind, value) }
func absent(e error) bool                         { return e == auth.ErrDenied || e == core.ErrDenied }
func safe(e error) error                          { return auth.SafeError(e) }
func utc(t time.Time) time.Time                   { return t.UTC().Truncate(time.Microsecond) }
func earlier(a, b time.Time) time.Time {
	if a.Before(b) {
		return a
	}
	return b
}
func field(m map[string]any, k string) string { v, _ := m[k].(string); return v }

func (s *Service) Do(ctx context.Context, cookie auth.BrowserCredential, csrf, key, network string, request Request) (auth.Outcome, error) {
	if s.state() == nil {
		return auth.Outcome{}, auth.ErrUnavailable
	}
	r := request.StorageValue()
	// Re-decode to freeze a fresh field map even for trusted in-process callers.
	body, e := json.Marshal(r.Fields)
	if e != nil {
		return auth.Outcome{}, auth.ErrInvalid
	}
	if requestSchemas[r.Action] == "" {
		body = nil
	}
	frozen, e := s.Decode(r.Action, r.WorkspaceID, r.RoomID, r.ResourceID, body)
	if e != nil {
		return auth.Outcome{}, e
	}
	r = frozen.StorageValue()
	canonical, e := json.Marshal(r)
	if e != nil {
		return auth.Outcome{}, auth.ErrInvalid
	}
	command := auth.RoomSecret(auth.RoomCommandData{Action: r.Action, TargetKey: r.WorkspaceID + "/" + r.RoomID + "/" + r.ResourceID, Canonical: canonical, Write: requestSchemas[r.Action] != ""})
	return s.state().authority.Do(ctx, cookie, csrf, key, network, command, auth.RoomCallbacks{
		Apply: func(ctx context.Context, a auth.Transaction, v auth.SessionData) (auth.Outcome, error) {
			t, e := s.state().storage.Bind(a.Core())
			if e != nil {
				return auth.Outcome{}, safe(e)
			}
			return s.apply(ctx, a, t, v, r)
		},
		Replay: func(ctx context.Context, a auth.Transaction, v auth.SessionData, out auth.Outcome) error {
			t, e := s.state().storage.Bind(a.Core())
			if e != nil {
				return safe(e)
			}
			return s.replay(ctx, a, t, v, r, out)
		},
	})
}

func workspaceAccount(ctx context.Context, c core.Transaction, v auth.SessionData, w string, management bool) error {
	if v.Kind != "account" {
		return auth.ErrDenied
	}
	a, e := c.Account(ctx, v.AccountID)
	if e != nil {
		return safe(e)
	}
	if a.Disabled {
		return auth.ErrDenied
	}
	if _, e = c.Workspace(ctx, w); e != nil {
		return safe(e)
	}
	m, e := c.Membership(ctx, w, v.AccountID)
	if e != nil {
		return safe(e)
	}
	if management && m.Role != core.Owner && m.Role != core.Admin {
		return auth.ErrDenied
	}
	return nil
}
func manager(ctx context.Context, c core.Transaction, t Transaction, v auth.SessionData, r RoomData) (bool, error) {
	if v.Kind != "account" {
		return false, nil
	}
	a, e := c.Account(ctx, v.AccountID)
	if e != nil {
		return false, safe(e)
	}
	if a.Disabled {
		return false, nil
	}
	if v.AccountID == r.Owner {
		return true, nil
	}
	if e = workspaceAccount(ctx, c, v, r.Scope.WorkspaceID, false); e != nil {
		if absent(e) {
			return false, nil
		}
		return false, e
	}
	return t.Manager(ctx, r.Scope, v.AccountID)
}
func liveParticipant(ctx context.Context, c core.Transaction, p ParticipantData, now time.Time) (bool, error) {
	if !p.Active {
		return false, nil
	}
	if p.AccountID != "" {
		a, e := c.Account(ctx, p.AccountID)
		if absent(e) {
			return false, nil
		}
		if e != nil || a.Disabled {
			return false, safe(e)
		}
		if p.GuestID != "" {
			g, e := c.Guest(ctx, p.Scope, p.GuestID)
			if absent(e) {
				return false, nil
			}
			return e == nil && !g.Disabled && g.ClaimedAccountID == p.AccountID, safe(e)
		}
		return true, nil
	}
	g, e := c.Guest(ctx, p.Scope, p.GuestID)
	if absent(e) {
		return false, nil
	}
	return e == nil && !g.Disabled && g.ClaimedAccountID == "" && now.Before(g.ExpiresAt), safe(e)
}
func actorParticipant(ctx context.Context, c core.Transaction, t Transaction, v auth.SessionData, r RoomData, now time.Time) (ParticipantData, error) {
	var p Participant
	var e error
	switch v.Kind {
	case "account":
		p, e = t.AccountParticipant(ctx, r.Scope, v.AccountID)
	case "guest":
		if v.Scope != r.Scope {
			return ParticipantData{}, auth.ErrDenied
		}
		p, e = t.GuestParticipant(ctx, r.Scope, v.GuestID)
	default:
		return ParticipantData{}, auth.ErrDenied
	}
	if e != nil {
		return ParticipantData{}, safe(e)
	}
	d := p.StorageValue()
	live, e := liveParticipant(ctx, c, d, now)
	if e != nil {
		return ParticipantData{}, e
	}
	if !live {
		return ParticipantData{}, auth.ErrDenied
	}
	return d, nil
}
func roomNow(ctx context.Context, c core.Transaction, t Transaction, w, id string) (RoomData, time.Time, error) {
	if _, e := c.Workspace(ctx, w); e != nil {
		return RoomData{}, time.Time{}, safe(e)
	}
	r, e := t.Room(ctx, w, id)
	if e != nil {
		return RoomData{}, time.Time{}, safe(e)
	}
	now, e := c.Now(ctx)
	return r.StorageValue(), utc(now), safe(e)
}
func requireLobby(r RoomData) error {
	if r.State != "lobby" {
		return auth.ErrConflict
	}
	return nil
}

func (s *Service) roomResponse(ctx context.Context, c core.Transaction, t Transaction, v auth.SessionData, r RoomData, now time.Time) (map[string]any, error) {
	if r.State == "closed" {
		return nil, auth.ErrDenied
	}
	mgmt, e := manager(ctx, c, t, v, r)
	if e != nil {
		return nil, e
	}
	p, e := actorParticipant(ctx, c, t, v, r, now)
	if e != nil && !absent(e) {
		return nil, e
	}
	if !mgmt && e != nil {
		return nil, auth.ErrDenied
	}
	roles := []string{}
	if v.Kind == "account" && v.AccountID == r.Owner {
		roles = append(roles, "owner")
	}
	if mgmt && v.AccountID != r.Owner {
		roles = append(roles, "administrator")
	}
	if e == nil && p.Host {
		roles = append(roles, "host")
	}
	rows, e := t.Participants(ctx, r.Scope)
	if e != nil {
		return nil, safe(e)
	}
	public := []any{}
	for _, row := range rows {
		d := row.StorageValue()
		live, e := liveParticipant(ctx, c, d, now)
		if e != nil {
			return nil, e
		}
		if !live {
			continue
		}
		kind, name := "guest", d.Name
		rs := []string{}
		if d.AccountID != "" {
			kind = "account"
			a, e := c.Account(ctx, d.AccountID)
			if e != nil {
				return nil, safe(e)
			}
			name = a.DisplayName
			m, e := manager(ctx, c, t, auth.SessionData{Kind: "account", AccountID: d.AccountID}, r)
			if e != nil {
				return nil, e
			}
			if m && d.AccountID != r.Owner {
				rs = append(rs, "administrator")
			}
		}
		if d.Host {
			rs = append(rs, "host")
		}
		public = append(public, map[string]any{"participant_id": d.ID, "kind": kind, "display_name": name, "roles": rs})
	}
	return map[string]any{"workspace_id": r.Scope.WorkspaceID, "room_id": r.Scope.RoomID, "game_id": r.Scope.GameID, "name": r.Name, "owner_account_id": r.Owner, "state": r.State, "caller_roles": roles, "participants": public}, nil
}

func (s *Service) apply(ctx context.Context, a auth.Transaction, t Transaction, v auth.SessionData, q RequestData) (auth.Outcome, error) {
	c := a.Core()
	if q.Action == "request_admission" {
		return s.requestAdmission(ctx, c, t, v, q)
	}
	if q.Action == "get_admission" || q.Action == "guest_token" {
		return s.ownAdmission(ctx, c, t, v, q)
	}
	if q.Action == "create_room" {
		if e := workspaceAccount(ctx, c, v, q.WorkspaceID, true); e != nil {
			return auth.Outcome{}, e
		}
		id, e := randomID()
		if e != nil {
			return auth.Outcome{}, e
		}
		r := RoomData{ID: id, Scope: core.Scope{WorkspaceID: q.WorkspaceID, RoomID: id, GameID: field(q.Fields, "game_id")}, Name: field(q.Fields, "name"), Owner: v.AccountID, State: "lobby"}
		if e = t.InsertRoom(ctx, auth.RoomSecret(r)); e != nil {
			return auth.Outcome{}, safe(e)
		}
		now, e := c.Now(ctx)
		if e != nil {
			return auth.Outcome{}, safe(e)
		}
		data, e := s.roomResponse(ctx, c, t, v, r, now)
		if e != nil {
			return auth.Outcome{}, e
		}
		return s.response(q.Action, data)
	}
	r, now, e := roomNow(ctx, c, t, q.WorkspaceID, q.RoomID)
	if e != nil {
		return auth.Outcome{}, e
	}
	if q.Action == "get_room" {
		data, e := s.roomResponse(ctx, c, t, v, r, now)
		if e != nil {
			return auth.Outcome{}, e
		}
		return s.response(q.Action, data)
	}
	if e = requireLobby(r); e != nil {
		return auth.Outcome{}, e
	}
	mgmt, e := manager(ctx, c, t, v, r)
	if e != nil {
		return auth.Outcome{}, e
	}
	if q.Action == "leave_room" {
		if v.Kind == "account" && v.AccountID == r.Owner {
			return auth.Outcome{}, auth.ErrDenied
		}
		p, e := actorParticipant(ctx, c, t, v, r, now)
		if e != nil {
			return auth.Outcome{}, e
		}
		if e = removeParticipant(ctx, c, t, p, false); e != nil {
			return auth.Outcome{}, e
		}
		return s.response(q.Action, map[string]any{"applied": true})
	}
	if !mgmt {
		return auth.Outcome{}, auth.ErrDenied
	}
	switch q.Action {
	case "close_room":
		if v.AccountID != r.Owner {
			return auth.Outcome{}, auth.ErrDenied
		}
		if e = t.CloseRoom(ctx, r.Scope); e != nil {
			return auth.Outcome{}, safe(e)
		}
	case "create_invite":
		id, e := randomID()
		if e != nil {
			return auth.Outcome{}, e
		}
		link, e := randomToken()
		if e != nil {
			return auth.Outcome{}, e
		}
		code, e := randomCode()
		if e != nil {
			return auth.Outcome{}, e
		}
		i := InvitationData{Scope: r.Scope, ID: id, LinkHash: s.hash("link", link), CodeHash: s.hash("code", code), ApprovalRequired: q.Fields["approval_required"].(bool), ExpiresAt: now.Add(time.Duration(q.Fields["expires_in_seconds"].(int64)) * time.Second), MaxUses: int(q.Fields["max_uses"].(int64))}
		if e = t.PutInvitation(ctx, auth.RoomSecret(i)); e != nil {
			return auth.Outcome{}, safe(e)
		}
		return s.response(q.Action, map[string]any{"invitation_id": i.ID, "approval_required": i.ApprovalRequired, "expires_at": i.ExpiresAt.Format(time.RFC3339Nano), "max_uses": i.MaxUses, "remaining_uses": i.MaxUses, "revoked": false, "invite_token": link, "room_code": code})
	case "revoke_invite":
		i, e := t.Invitation(ctx, r.Scope, q.ResourceID)
		if e != nil {
			return auth.Outcome{}, safe(e)
		}
		d := i.StorageValue()
		d.Revoked = true
		if e = t.PutInvitation(ctx, auth.RoomSecret(d)); e != nil {
			return auth.Outcome{}, safe(e)
		}
	case "list_admissions":
		rows, e := t.Pending(ctx, r.Scope, now)
		if e != nil {
			return auth.Outcome{}, safe(e)
		}
		out := []any{}
		for _, a := range rows {
			valid, e := s.admissionCurrent(ctx, c, t, a.StorageValue(), now)
			if e != nil {
				return auth.Outcome{}, e
			}
			if valid.Status == "pending" {
				out = append(out, admissionPublic(valid))
			}
		}
		return s.response(q.Action, map[string]any{"admissions": out})
	case "decide_admission":
		return s.decide(ctx, a, t, v, r, now, q)
	case "kick_participant", "set_role":
		p, e := t.Participant(ctx, r.Scope, q.ResourceID)
		if e != nil {
			return auth.Outcome{}, safe(e)
		}
		d := p.StorageValue()
		if d.AccountID == r.Owner && (q.Action == "kick_participant" || field(q.Fields, "role") == "administrator") {
			return auth.Outcome{}, auth.ErrDenied
		}
		if q.Action == "kick_participant" {
			other, e := t.Manager(ctx, r.Scope, d.AccountID)
			if e != nil {
				return auth.Outcome{}, safe(e)
			}
			if other && v.AccountID != r.Owner && d.AccountID != v.AccountID {
				return auth.Outcome{}, auth.ErrDenied
			}
			if e = removeParticipant(ctx, c, t, d, true); e != nil {
				return auth.Outcome{}, e
			}
		} else {
			role, enabled := field(q.Fields, "role"), q.Fields["enabled"].(bool)
			if role == "administrator" {
				if v.AccountID != r.Owner || d.AccountID == "" {
					return auth.Outcome{}, auth.ErrDenied
				}
				if e = workspaceAccount(ctx, c, auth.SessionData{Kind: "account", AccountID: d.AccountID}, r.Scope.WorkspaceID, false); e != nil {
					return auth.Outcome{}, e
				}
				if e = t.SetManager(ctx, r.Scope, d.AccountID, enabled); e != nil {
					return auth.Outcome{}, safe(e)
				}
			} else {
				live, e := liveParticipant(ctx, c, d, now)
				if e != nil {
					return auth.Outcome{}, e
				}
				if !live {
					return auth.Outcome{}, auth.ErrDenied
				}
				d.Host = enabled
				if e = t.PutParticipant(ctx, auth.RoomSecret(d)); e != nil {
					return auth.Outcome{}, safe(e)
				}
			}
		}
	default:
		return auth.Outcome{}, auth.ErrInvalid
	}
	return s.response(q.Action, map[string]any{"applied": true})
}

func removeParticipant(ctx context.Context, c core.Transaction, t Transaction, p ParticipantData, kick bool) error {
	p.Active = false
	p.Host = false
	if e := t.PutParticipant(ctx, auth.RoomSecret(p)); e != nil {
		return safe(e)
	}
	if p.GuestID != "" {
		if e := c.DisableGuest(ctx, p.Scope, p.GuestID); e != nil {
			return safe(e)
		}
	}
	if kick && p.AccountID != "" {
		if e := t.SetManager(ctx, p.Scope, p.AccountID, false); e != nil {
			return safe(e)
		}
	}
	return nil
}
