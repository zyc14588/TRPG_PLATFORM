// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package room

import (
	"context"
	"encoding/json"

	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
)

// A receipt is a result, never a capability. Current authoritative rows must
// still permit returning it, including secret-bearing invitation responses.
func (s *Service) replay(ctx context.Context, authTx auth.Transaction, t Transaction, v auth.SessionData, q RequestData, out auth.Outcome) error {
	c := authTx.Core()
	var envelope map[string]any
	if e := json.Unmarshal(out.StorageValue().Body, &envelope); e != nil {
		return auth.ErrUnavailable
	}
	if s.state().schemas[responseSchemas[q.Action]].Validate(envelope) != nil {
		return auth.ErrUnavailable
	}
	data, ok := envelope["data"].(map[string]any)
	if !ok {
		return auth.ErrUnavailable
	}
	if q.Action == "request_admission" || q.Action == "guest_token" {
		row, e := t.Admission(ctx, field(data, "admission_id"))
		if e != nil {
			return safe(e)
		}
		a := row.StorageValue()
		if !ownedAdmission(v, a) {
			return auth.ErrDenied
		}
		now, e := c.Now(ctx)
		if e != nil {
			return safe(e)
		}
		a, e = s.admissionCurrent(ctx, c, t, a, utc(now))
		if e != nil {
			return e
		}
		if a.Status == "expired" || a.Status == "rejected" {
			return auth.ErrDenied
		}
		if q.Action == "guest_token" {
			if a.Status != "approved" || a.TokenUsed || !now.Before(a.TokenExpiresAt) || a.TokenHash != s.hash("admission", field(data, "admission_token")) {
				return auth.ErrDenied
			}
		}
		return nil
	}
	workspace, room := q.WorkspaceID, q.RoomID
	if q.Action == "create_room" {
		room = field(data, "room_id")
		if field(data, "workspace_id") != workspace {
			return auth.ErrDenied
		}
	}
	r, now, e := roomNow(ctx, c, t, workspace, room)
	if e != nil {
		return e
	}
	mgmt, e := manager(ctx, c, t, v, r)
	if e != nil {
		return e
	}
	if q.Action == "leave_room" {
		if r.State != "lobby" || v.Kind == "account" && v.AccountID == r.Owner {
			return auth.ErrDenied
		}
		// Never recreate a participant. A former account can only acknowledge
		// its own recorded departure while it remains departed.
		if v.Kind != "account" {
			return auth.ErrDenied
		}
		p, e := t.AccountParticipant(ctx, r.Scope, v.AccountID)
		if e != nil {
			return safe(e)
		}
		if p.StorageValue().Active {
			return auth.ErrDenied
		}
		return nil
	}
	if !mgmt {
		return auth.ErrDenied
	}
	if q.Action == "close_room" {
		if v.AccountID != r.Owner || r.State != "closed" {
			return auth.ErrDenied
		}
		return nil
	}
	if r.State != "lobby" {
		return auth.ErrDenied
	}
	switch q.Action {
	case "create_room":
		if r.Owner != v.AccountID {
			return auth.ErrDenied
		}
	case "create_invite":
		i, e := t.Invitation(ctx, r.Scope, field(data, "invitation_id"))
		if e != nil {
			return safe(e)
		}
		if e = invitationCurrent(i.StorageValue(), r, now); e != nil {
			return e
		}
	case "revoke_invite":
		i, e := t.Invitation(ctx, r.Scope, q.ResourceID)
		if e != nil {
			return safe(e)
		}
		if !i.StorageValue().Revoked {
			return auth.ErrDenied
		}
	case "decide_admission":
		row, e := t.Admission(ctx, q.ResourceID)
		if e != nil {
			return safe(e)
		}
		a := row.StorageValue()
		if a.Scope != r.Scope {
			return auth.ErrDenied
		}
		a, e = s.admissionCurrent(ctx, c, t, a, now)
		if e != nil {
			return e
		}
		want := "approved"
		if field(q.Fields, "decision") == "reject" {
			want = "rejected"
		}
		if a.Status != want {
			return auth.ErrDenied
		}
		if a.Mode == "guest" && a.Status == "approved" && !a.TokenUsed {
			if e = guestApplicantLive(ctx, authTx, a, now); e != nil {
				return e
			}
		}
	case "kick_participant", "set_role":
		row, e := t.Participant(ctx, r.Scope, q.ResourceID)
		if e != nil {
			return safe(e)
		}
		p := row.StorageValue()
		if q.Action == "kick_participant" {
			if p.AccountID == r.Owner || p.Active {
				return auth.ErrDenied
			}
			other, e := t.Manager(ctx, r.Scope, p.AccountID)
			if e != nil {
				return safe(e)
			}
			if other && v.AccountID != r.Owner {
				return auth.ErrDenied
			}
			return nil
		}
		if field(q.Fields, "role") == "administrator" {
			if v.AccountID != r.Owner || p.AccountID == "" || p.AccountID == r.Owner {
				return auth.ErrDenied
			}
			if e = workspaceAccount(ctx, c, auth.SessionData{Kind: "account", AccountID: p.AccountID}, r.Scope.WorkspaceID, false); e != nil {
				return e
			}
			enabled, e := t.Manager(ctx, r.Scope, p.AccountID)
			if e != nil {
				return safe(e)
			}
			if enabled != q.Fields["enabled"].(bool) {
				return auth.ErrDenied
			}
		} else {
			live, e := liveParticipant(ctx, c, p, now)
			if e != nil {
				return e
			}
			if !live || p.Host != q.Fields["enabled"].(bool) {
				return auth.ErrDenied
			}
		}
	default:
		return auth.ErrDenied
	}
	return nil
}
