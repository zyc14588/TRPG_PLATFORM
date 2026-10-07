//go:build integration

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package room_test

import (
	"context"
	"fmt"
	"testing"

	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
)

func TestGuestClaimExistingParticipationConflictIsAtomic(t *testing.T) {
	f := newFixture(t, nil)
	r := f.create(t)
	i := f.invite(t, r, false, 4)
	account, login := f.register(t, "Account nickname")
	original := f.join(t, account, i)["participant_id"].(string)
	guest, _, _ := f.guest(t, r, i)
	q := f.authRequest(t, "claim", map[string]any{"mode": "existing_account", "login_name": login, "password": "synthetic-claim-password"})
	out, e := f.authority.Authentication().Mutate(f.ctx, guest.StorageValue().Cookie, guest.StorageValue().CSRF, "claim-conflict-key1", "owned-network", q)
	want(t, e, auth.ErrConflict)
	if len(out.StorageValue().Body) != 0 || out.StorageValue().Cookie.StorageValue() != "" {
		t.Fatal("claim conflict returned cookie")
	}
	view := f.call(t, guest, "get_room", f.w, r, "", nil)
	if len(view["participants"].([]any)) != 2 {
		t.Fatal("claim conflict changed participants")
	}
	need(t, f.r.Transact(f.ctx, func(tx auth.Transaction) error {
		tr, e := f.store.Bind(tx.Core())
		if e != nil {
			return e
		}
		p, e := tr.Participant(f.ctx, core.Scope{WorkspaceID: f.w, RoomID: r, GameID: "game"}, original)
		if e != nil {
			return e
		}
		if p.StorageValue().AccountID != account.StorageValue().ID {
			t.Fatal("existing account relation changed")
		}
		rows, e := tr.Participants(f.ctx, p.StorageValue().Scope)
		if e != nil {
			return e
		}
		for _, row := range rows {
			d := row.StorageValue()
			if d.ID == original {
				continue
			}
			if d.AccountID != "" {
				t.Fatal("claim conflict projected account")
			}
			g, e := tx.Core().Guest(f.ctx, d.Scope, d.GuestID)
			if e != nil {
				return e
			}
			if g.ClaimedAccountID != "" {
				t.Fatal("claim conflict updated core guest")
			}
		}
		return nil
	}))
}
func TestInvitationAndAnonymousDeadlineExpiry(t *testing.T) {
	f := newFixture(t, nil)
	r := f.create(t)
	i := f.invite(t, r, false, 4)
	a := f.account(t)
	f.sql(t, "UPDATE platform_room.invitations SET expires_at=clock_timestamp()-interval '1 second' WHERE workspace_id='"+f.w+"' AND room_id='"+r+"' AND id='"+i["invitation_id"].(string)+"'")
	_, e := f.do(t, a, "expired-invite-key1", f.request(t, "request_admission", "", "", "", map[string]any{"mode": "account", "room_code": i["room_code"]}))
	want(t, e, auth.ErrDenied)
	i = f.invite(t, r, true, 4)
	anon := f.anonymous(t)
	pending := f.call(t, anon, "request_admission", "", "", "", map[string]any{"mode": "guest", "invite_token": i["invite_token"], "display_name": "Guest"})
	f.sql(t, "UPDATE platform_auth.sessions SET expires_at=clock_timestamp()-interval '1 second' WHERE token_hash='"+hashed(anon.StorageValue().Cookie.StorageValue())+"'")
	_, e = f.do(t, f.owner, "expired-approve-01", f.request(t, "decide_admission", f.w, r, pending["admission_id"].(string), map[string]any{"decision": "approve"}))
	want(t, e, auth.ErrDenied)
	_, e = f.do(t, anon, "", f.request(t, "get_admission", "", "", pending["admission_id"].(string), nil))
	want(t, e, auth.ErrUnauthenticated)
}
func TestPending64AcrossServiceInstancesAndCapacity65Denied(t *testing.T) {
	f := newFixture(t, nil)
	r := f.create(t)
	i := f.invite(t, r, true, 64)
	for j := 0; j < 64; j++ {
		if j%16 == 0 {
			f.recompose(t)
		}
		a := f.anonymous(t)
		data := f.call(t, a, "request_admission", "", "", "", map[string]any{"mode": "guest", "invite_token": i["invite_token"], "display_name": "Guest"})
		if data["status"] != "pending" {
			t.Fatal("pending request consumed approval")
		}
	}
	f.recompose(t)
	a := f.anonymous(t)
	_, e := f.do(t, a, "pending-overflow01", f.request(t, "request_admission", "", "", "", map[string]any{"mode": "guest", "invite_token": i["invite_token"], "display_name": "Guest"}))
	want(t, e, auth.ErrConflict)
	queue := f.call(t, f.owner, "list_admissions", f.w, r, "", nil)
	if len(queue["admissions"].([]any)) != 64 {
		t.Fatal("pending queue bound differs")
	}
	view := f.call(t, f.owner, "get_room", f.w, r, "", nil)
	if len(view["participants"].([]any)) != 0 {
		t.Fatal("pending overflow allocated participant")
	}
}
func TestParticipant64AcrossServiceInstancesAndCapacity65Denied(t *testing.T) {
	f := newFixture(t, nil)
	r := f.create(t)
	i := f.invite(t, r, false, 64)
	for j := 0; j < 64; j++ {
		if j%16 == 0 {
			f.recompose(t)
		}
		f.join(t, f.account(t), i)
	}
	f.recompose(t)
	second := f.invite(t, r, false, 4)
	a := f.account(t)
	_, e := f.do(t, a, "participant-limit1", f.request(t, "request_admission", "", "", "", map[string]any{"mode": "account", "invite_token": second["invite_token"]}))
	want(t, e, auth.ErrConflict)
	view := f.call(t, f.owner, "get_room", f.w, r, "", nil)
	if len(view["participants"].([]any)) != 64 {
		t.Fatal("participant bound differs")
	}
}
func TestServiceAndVerifiedIdentityRateLimits(t *testing.T) {
	t.Run("identity_action_10", func(t *testing.T) {
		f := newFixture(t, nil)
		r := f.create(t)
		q := f.request(t, "get_room", f.w, r, "", nil)
		for j := 0; j < 10; j++ {
			_, e := f.do(t, f.owner, "", q)
			need(t, e)
		}
		_, e := f.do(t, f.owner, "", q)
		want(t, e, auth.ErrRateLimited)
	})
	t.Run("service_60", func(t *testing.T) {
		f := newFixture(t, nil)
		r := f.create(t)
		i := f.invite(t, r, false, 64)
		for j := 0; j < 58; j++ {
			a := f.account(t)
			_, e := f.do(t, a, fmt.Sprintf("rate-request-%06d", j), f.request(t, "request_admission", "", "", "", map[string]any{"mode": "account", "invite_token": i["invite_token"]}))
			need(t, e)
		}
		a := f.account(t)
		_, e := f.do(t, a, "global-overflow-01", f.request(t, "request_admission", "", "", "", map[string]any{"mode": "account", "invite_token": i["invite_token"]}))
		want(t, e, auth.ErrRateLimited)
	})
}
func TestDisabledApplicantApprovalAndLaunchedLobbyMutationsDenied(t *testing.T) {
	f := newFixture(t, nil)
	r := f.create(t)
	i := f.invite(t, r, true, 4)
	a := f.account(t)
	pending := f.join(t, a, i)
	need(t, f.r.Transact(f.ctx, func(tx auth.Transaction) error { return tx.Core().DisableAccount(f.ctx, a.StorageValue().ID) }))
	_, e := f.do(t, f.owner, "disabled-approval1", f.request(t, "decide_admission", f.w, r, pending["admission_id"].(string), map[string]any{"decision": "approve"}))
	want(t, e, auth.ErrDenied)
	f.sql(t, "UPDATE platform_room.rooms SET state='launched' WHERE workspace_id='"+f.w+"' AND room_id='"+r+"'")
	for _, q := range []struct {
		a      string
		fields map[string]any
	}{{"close_room", map[string]any{}}, {"create_invite", map[string]any{"approval_required": false, "expires_in_seconds": 60, "max_uses": 1}}} {
		_, e = f.do(t, f.owner, fmt.Sprintf("launched-denied-%06d", sequence.Add(1)), f.request(t, q.a, f.w, r, "", q.fields))
		want(t, e, auth.ErrConflict)
	}
}
func TestRoomReceiptUnknownCommitRestartAndDigestConflict(t *testing.T) {
	var armed bool
	f := newFixture(t, func(_ context.Context, p string) error {
		if armed && p == "after-commit" {
			armed = false
			return auth.ErrUnavailable
		}
		return nil
	})
	r := f.create(t)
	q := f.request(t, "create_invite", f.w, r, "", map[string]any{"approval_required": false, "expires_in_seconds": 3600, "max_uses": 4})
	armed = true
	out, e := f.do(t, f.owner, "unknown-room-key01", q)
	want(t, e, auth.ErrOutcomeUnknown)
	if len(out.StorageValue().Body) != 0 {
		t.Fatal("unknown room commit returned secret")
	}
	f.recompose(t)
	out, e = f.do(t, f.owner, "unknown-room-key01", q)
	need(t, e)
	again, e := f.do(t, f.owner, "unknown-room-key01", q)
	need(t, e)
	if string(out.StorageValue().Body) != string(again.StorageValue().Body) {
		t.Fatal("unknown room receipt not stable")
	}
	changed := f.request(t, "create_invite", f.w, r, "", map[string]any{"approval_required": false, "expires_in_seconds": 3600, "max_uses": 3})
	_, e = f.do(t, f.owner, "unknown-room-key01", changed)
	want(t, e, auth.ErrConflict)
}
