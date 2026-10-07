// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package room

import (
	"context"
	"fmt"
	"io"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
)

func admissionPublic(a AdmissionData) map[string]any {
	var participant any
	if a.Mode == "account" && a.Status == "approved" {
		participant = a.ParticipantID
	}
	return map[string]any{"admission_id": a.ID, "workspace_id": a.Scope.WorkspaceID, "room_id": a.Scope.RoomID, "game_id": a.Scope.GameID, "mode": a.Mode, "status": a.Status, "display_name": a.Name, "expires_at": a.ExpiresAt.Format(time.RFC3339Nano), "participant_id": participant}
}
func invitationCurrent(i InvitationData, r RoomData, now time.Time) error {
	if i.Scope != r.Scope || i.Revoked || !now.Before(i.ExpiresAt) || r.State != "lobby" {
		return auth.ErrDenied
	}
	return nil
}
func ownedAdmission(v auth.SessionData, a AdmissionData) bool {
	return a.Mode == "account" && v.Kind == "account" && v.AccountID == a.OwnerAccount || a.Mode == "guest" && v.Kind == "preauth" && v.Hash == a.OwnerSession
}
func (s *Service) admissionCurrent(ctx context.Context, c core.Transaction, t Transaction, a AdmissionData, now time.Time) (AdmissionData, error) {
	r, _, e := roomNow(ctx, c, t, a.Scope.WorkspaceID, a.Scope.RoomID)
	if e != nil {
		return AdmissionData{}, e
	}
	if r.Scope != a.Scope {
		return AdmissionData{}, auth.ErrDenied
	}
	if a.Status == "approved" && a.Mode == "account" {
		p, e := t.Participant(ctx, a.Scope, a.ParticipantID)
		if e != nil {
			return AdmissionData{}, safe(e)
		}
		d := p.StorageValue()
		live, e := liveParticipant(ctx, c, d, now)
		if e != nil {
			return AdmissionData{}, e
		}
		if r.State == "closed" || !live || d.AccountID != a.OwnerAccount {
			return AdmissionData{}, auth.ErrDenied
		}
		return a, nil
	}
	i, e := t.Invitation(ctx, a.Scope, a.InvitationID)
	if e != nil {
		return AdmissionData{}, safe(e)
	}
	if a.Status != "rejected" && (invitationCurrent(i.StorageValue(), r, now) != nil || !now.Before(a.ExpiresAt)) {
		a.Status = "expired"
	}
	return a, nil
}

func capacity(ctx context.Context, c core.Transaction, t Transaction, scope core.Scope, now time.Time) error {
	rows, e := t.Participants(ctx, scope)
	if e != nil {
		return safe(e)
	}
	n := 0
	for _, row := range rows {
		p := row.StorageValue()
		live, e := liveParticipant(ctx, c, p, now)
		if e != nil {
			return e
		}
		if live {
			n++
		} else if p.Active {
			if e = removeParticipant(ctx, c, t, p, false); e != nil {
				return e
			}
		}
	}
	if n >= 64 {
		return auth.ErrConflict
	}
	return nil
}
func joinAccount(ctx context.Context, c core.Transaction, t Transaction, r RoomData, i InvitationData, account string, now time.Time) (string, error) {
	a, e := c.Account(ctx, account)
	if e != nil {
		return "", safe(e)
	}
	if a.Disabled {
		return "", auth.ErrDenied
	}
	old, e := t.AccountParticipant(ctx, r.Scope, account)
	var p ParticipantData
	if e == nil {
		p = old.StorageValue()
		live, e := liveParticipant(ctx, c, p, now)
		if e != nil {
			return "", e
		}
		if live {
			return p.ID, nil
		}
	} else if !absent(e) {
		return "", safe(e)
	}
	if e = invitationCurrent(i, r, now); e != nil {
		return "", e
	}
	if i.Uses >= i.MaxUses {
		return "", auth.ErrConflict
	}
	if e = capacity(ctx, c, t, r.Scope, now); e != nil {
		return "", e
	}
	if p.ID == "" {
		id, e := randomID()
		if e != nil {
			return "", e
		}
		p = ParticipantData{ID: id, Scope: r.Scope, AccountID: account, Name: a.DisplayName}
	}
	p.Active = true
	p.Host = false
	i.Uses++
	if e = t.PutInvitation(ctx, auth.RoomSecret(i)); e != nil {
		return "", safe(e)
	}
	if e = t.PutParticipant(ctx, auth.RoomSecret(p)); e != nil {
		return "", safe(e)
	}
	return p.ID, nil
}

func (s *Service) requestAdmission(ctx context.Context, c core.Transaction, t Transaction, v auth.SessionData, q RequestData) (auth.Outcome, error) {
	mode := field(q.Fields, "mode")
	if v.Kind == "guest" {
		return auth.Outcome{}, auth.ErrClaimRequired
	}
	if mode == "account" && v.Kind != "account" || mode == "guest" && v.Kind != "preauth" {
		return auth.Outcome{}, auth.ErrDenied
	}
	kind, value := "link", field(q.Fields, "invite_token")
	if value == "" {
		kind = "code"
		value = field(q.Fields, "room_code")
	}
	inv, e := t.InvitationBySecret(ctx, kind, s.hash(kind, value))
	if e != nil {
		return auth.Outcome{}, safe(e)
	}
	i := inv.StorageValue()
	r, now, e := roomNow(ctx, c, t, i.Scope.WorkspaceID, i.Scope.RoomID)
	if e != nil {
		return auth.Outcome{}, e
	}
	if e = invitationCurrent(i, r, now); e != nil {
		return auth.Outcome{}, e
	}
	owner, name := v.Hash, field(q.Fields, "display_name")
	if mode == "account" {
		owner = v.AccountID
		a, e := c.Account(ctx, owner)
		if e != nil {
			return auth.Outcome{}, safe(e)
		}
		if a.Disabled {
			return auth.Outcome{}, auth.ErrDenied
		}
		name = a.DisplayName
	}
	old, e := t.ActorAdmission(ctx, r.Scope, i.ID, mode, owner)
	if e == nil {
		a, e := s.admissionCurrent(ctx, c, t, old.StorageValue(), now)
		if e != nil {
			return auth.Outcome{}, e
		}
		if a.Status == "rejected" || a.Status == "expired" {
			return auth.Outcome{}, auth.ErrDenied
		}
		return s.response(q.Action, admissionPublic(a))
	} else if !absent(e) {
		return auth.Outcome{}, safe(e)
	}
	if i.Uses >= i.MaxUses {
		// An already admitted account consumes no additional use on a new link.
		if mode != "account" {
			return auth.Outcome{}, auth.ErrConflict
		}
		p, e := t.AccountParticipant(ctx, r.Scope, v.AccountID)
		if e != nil {
			return auth.Outcome{}, auth.ErrConflict
		}
		live, e := liveParticipant(ctx, c, p.StorageValue(), now)
		if e != nil {
			return auth.Outcome{}, e
		}
		if !live {
			return auth.Outcome{}, auth.ErrConflict
		}
	}
	id, e := randomID()
	if e != nil {
		return auth.Outcome{}, e
	}
	a := AdmissionData{ID: id, Scope: r.Scope, InvitationID: i.ID, Mode: mode, Status: "pending", Name: name, ExpiresAt: i.ExpiresAt}
	if mode == "account" {
		a.OwnerAccount = v.AccountID
	} else {
		a.OwnerSession = v.Hash
		a.ExpiresAt = earlier(a.ExpiresAt, v.ExpiresAt)
	}
	already := false
	if mode == "account" {
		p, e := t.AccountParticipant(ctx, r.Scope, v.AccountID)
		if e == nil {
			live, e := liveParticipant(ctx, c, p.StorageValue(), now)
			if e != nil {
				return auth.Outcome{}, e
			}
			if live {
				already = true
				a.ParticipantID = p.StorageValue().ID
			}
		} else if !absent(e) {
			return auth.Outcome{}, safe(e)
		}
	}
	if !i.ApprovalRequired || already {
		a.Status = "approved"
		if mode == "account" {
			pid, e := joinAccount(ctx, c, t, r, i, v.AccountID, now)
			if e != nil {
				return auth.Outcome{}, e
			}
			a.ParticipantID = pid
		}
	} else {
		pending, e := t.Pending(ctx, r.Scope, now)
		if e != nil {
			return auth.Outcome{}, safe(e)
		}
		if len(pending) >= 64 {
			return auth.Outcome{}, auth.ErrConflict
		}
	}
	if e = t.PutAdmission(ctx, auth.RoomSecret(a)); e != nil {
		return auth.Outcome{}, safe(e)
	}
	return s.response(q.Action, admissionPublic(a))
}
func (s *Service) ownAdmission(ctx context.Context, c core.Transaction, t Transaction, v auth.SessionData, q RequestData) (auth.Outcome, error) {
	row, e := t.Admission(ctx, q.ResourceID)
	if e != nil {
		return auth.Outcome{}, safe(e)
	}
	a := row.StorageValue()
	if !ownedAdmission(v, a) {
		return auth.Outcome{}, auth.ErrDenied
	}
	now, e := c.Now(ctx)
	if e != nil {
		return auth.Outcome{}, safe(e)
	}
	now = utc(now)
	a, e = s.admissionCurrent(ctx, c, t, a, now)
	if e != nil {
		return auth.Outcome{}, e
	}
	if q.Action == "get_admission" {
		return s.response(q.Action, admissionPublic(a))
	}
	if a.Mode != "guest" || a.Status != "approved" || a.TokenUsed || a.TokenHash != "" || !now.Before(a.ExpiresAt) {
		return auth.Outcome{}, auth.ErrDenied
	}
	token, e := randomToken()
	if e != nil {
		return auth.Outcome{}, e
	}
	a.TokenHash = s.hash("admission", token)
	a.TokenExpiresAt = earlier(now.Add(5*time.Minute), a.ExpiresAt)
	if e = t.PutAdmission(ctx, auth.RoomSecret(a)); e != nil {
		return auth.Outcome{}, safe(e)
	}
	return s.response(q.Action, map[string]any{"admission_id": a.ID, "admission_token": token, "expires_at": a.TokenExpiresAt.Format(time.RFC3339Nano)})
}
func (s *Service) decide(ctx context.Context, authTx auth.Transaction, t Transaction, v auth.SessionData, r RoomData, now time.Time, q RequestData) (auth.Outcome, error) {
	c := authTx.Core()
	row, e := t.Admission(ctx, q.ResourceID)
	if e != nil {
		return auth.Outcome{}, safe(e)
	}
	a := row.StorageValue()
	if a.Scope != r.Scope {
		return auth.Outcome{}, auth.ErrDenied
	}
	a, e = s.admissionCurrent(ctx, c, t, a, now)
	if e != nil {
		return auth.Outcome{}, e
	}
	if a.Status != "pending" {
		return auth.Outcome{}, auth.ErrConflict
	}
	if field(q.Fields, "decision") == "reject" {
		a.Status = "rejected"
	} else {
		i, e := t.Invitation(ctx, r.Scope, a.InvitationID)
		if e != nil {
			return auth.Outcome{}, safe(e)
		}
		d := i.StorageValue()
		if e = invitationCurrent(d, r, now); e != nil {
			return auth.Outcome{}, e
		}
		if d.Uses >= d.MaxUses {
			return auth.Outcome{}, auth.ErrConflict
		}
		if a.Mode == "account" {
			pid, e := joinAccount(ctx, c, t, r, d, a.OwnerAccount, now)
			if e != nil {
				return auth.Outcome{}, e
			}
			a.ParticipantID = pid
		} else {
			if e = guestApplicantLive(ctx, authTx, a, now); e != nil {
				return auth.Outcome{}, e
			}
			if e = capacity(ctx, c, t, r.Scope, now); e != nil {
				return auth.Outcome{}, e
			}
		}
		a.Status = "approved"
	}
	if e = t.PutAdmission(ctx, auth.RoomSecret(a)); e != nil {
		return auth.Outcome{}, safe(e)
	}
	return s.response(q.Action, admissionPublic(a))
}
func guestApplicantLive(ctx context.Context, tx auth.Transaction, a AdmissionData, now time.Time) error {
	row, e := tx.Session(ctx, a.OwnerSession)
	if e != nil {
		return safe(e)
	}
	v := row.StorageValue()
	if a.Mode != "guest" || v.Hash != a.OwnerSession || v.Kind != "preauth" || v.Retired || v.Revoked || !now.Before(v.ExpiresAt) || now.Sub(v.LastSeen) >= 30*time.Minute {
		return auth.ErrDenied
	}
	return nil
}

type AdmissionVerifier struct{ data **verifierData }
type verifierData struct {
	storage Storage
	key     []byte
}

func NewAdmissionVerifier(storage Storage, key []byte) (*AdmissionVerifier, error) {
	if storage == nil || len(key) != 32 {
		return nil, auth.ErrInvalid
	}
	d := &verifierData{storage: storage, key: append([]byte(nil), key...)}
	return &AdmissionVerifier{data: &d}, nil
}
func (v *AdmissionVerifier) state() *verifierData {
	if v == nil || v.data == nil {
		return nil
	}
	return *v.data
}
func (*AdmissionVerifier) Format(f fmt.State, _ rune) {
	_, _ = io.WriteString(f, "<room admission verifier>")
}
func (*AdmissionVerifier) MarshalJSON() ([]byte, error) { return nil, auth.ErrDenied }

func (v *AdmissionVerifier) Verify(ctx context.Context, c core.Transaction, token auth.BrowserCredential) (core.Guest, error) {
	if v.state() == nil || !linkToken.MatchString(token.StorageValue()) {
		return core.Guest{}, auth.ErrDenied
	}
	session, e := auth.RoomAdmissionSession(c)
	if e != nil {
		return core.Guest{}, auth.ErrDenied
	}
	sd := session.StorageValue()
	if sd.Kind != "preauth" || sd.Retired || sd.Revoked {
		return core.Guest{}, auth.ErrDenied
	}
	t, e := v.state().storage.Bind(c)
	if e != nil {
		return core.Guest{}, safe(e)
	}
	row, e := t.AdmissionByToken(ctx, keyedHash(v.state().key, "admission", token.StorageValue()))
	if e != nil {
		return core.Guest{}, safe(e)
	}
	a := row.StorageValue()
	if a.Mode != "guest" || a.Status != "approved" || a.TokenUsed || a.OwnerSession != sd.Hash {
		return core.Guest{}, auth.ErrDenied
	}
	r, now, e := roomNow(ctx, c, t, a.Scope.WorkspaceID, a.Scope.RoomID)
	if e != nil {
		return core.Guest{}, e
	}
	if a.Scope != r.Scope || !now.Before(a.ExpiresAt) || !now.Before(a.TokenExpiresAt) || !now.Before(sd.ExpiresAt) || now.Sub(sd.LastSeen) >= 30*time.Minute {
		return core.Guest{}, auth.ErrDenied
	}
	inv, e := t.Invitation(ctx, r.Scope, a.InvitationID)
	if e != nil {
		return core.Guest{}, safe(e)
	}
	i := inv.StorageValue()
	if e = invitationCurrent(i, r, now); e != nil {
		return core.Guest{}, e
	}
	if i.Uses >= i.MaxUses {
		return core.Guest{}, auth.ErrConflict
	}
	if e = capacity(ctx, c, t, r.Scope, now); e != nil {
		return core.Guest{}, e
	}
	id, e := randomID()
	if e != nil {
		return core.Guest{}, e
	}
	pid, e := randomID()
	if e != nil {
		return core.Guest{}, e
	}
	g := core.Guest{Scope: r.Scope, ID: id, ExpiresAt: now.Add(8 * time.Hour)}
	if e = c.InsertGuest(ctx, g); e != nil {
		return core.Guest{}, safe(e)
	}
	if e = t.PutParticipant(ctx, auth.RoomSecret(ParticipantData{Scope: r.Scope, ID: pid, GuestID: id, Name: a.Name, Active: true})); e != nil {
		return core.Guest{}, safe(e)
	}
	i.Uses++
	a.TokenUsed = true
	a.ParticipantID = pid
	if e = t.PutInvitation(ctx, auth.RoomSecret(i)); e != nil {
		return core.Guest{}, safe(e)
	}
	if e = t.PutAdmission(ctx, auth.RoomSecret(a)); e != nil {
		return core.Guest{}, safe(e)
	}
	return g, nil
}
func (v *AdmissionVerifier) BeforeRoomClaim(ctx context.Context, c core.Transaction, scope core.Scope, guest, account string) error {
	if v.state() == nil {
		return core.ErrDenied
	}
	t, e := v.state().storage.Bind(c)
	if e != nil {
		return roomCoreError(e)
	}
	r, now, e := roomNow(ctx, c, t, scope.WorkspaceID, scope.RoomID)
	if e != nil {
		return roomCoreError(e)
	}
	if r.Scope != scope || r.State == "closed" {
		return core.ErrDenied
	}
	row, e := t.GuestParticipant(ctx, scope, guest)
	if e != nil {
		return roomCoreError(e)
	}
	p := row.StorageValue()
	live, e := liveParticipant(ctx, c, p, now)
	if e != nil {
		return roomCoreError(e)
	}
	if !live || p.AccountID != "" {
		return core.ErrDenied
	}
	other, e := t.AccountParticipant(ctx, scope, account)
	if e == nil && other.StorageValue().ID != p.ID {
		return core.ErrConflict
	}
	if e != nil && !absent(e) {
		return roomCoreError(e)
	}
	a, e := c.Account(ctx, account)
	if e != nil {
		return roomCoreError(e)
	}
	if a.Disabled {
		return core.ErrDenied
	}
	p.AccountID = account
	p.Name = a.DisplayName
	return roomCoreError(t.PutParticipant(ctx, auth.RoomSecret(p)))
}
func roomCoreError(e error) error {
	switch auth.SafeError(e) {
	case nil:
		return nil
	case auth.ErrDenied:
		return core.ErrDenied
	case auth.ErrConflict:
		return core.ErrConflict
	case auth.ErrOutcomeUnknown:
		return core.ErrOutcomeUnknown
	default:
		return core.ErrUnavailable
	}
}
