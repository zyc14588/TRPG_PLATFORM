//go:build integration

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package room_test

import (
	"context"
	"fmt"
	"strings"
	"sync/atomic"
	"testing"

	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
)

func TestGuestAdmissionOriginalCookieAndRevokedToken(t *testing.T) {
	f := newFixture(t, nil)
	r := f.create(t)
	i := f.invite(t, r, true, 4)
	anon, id, token := f.prepareGuest(t, r, i)
	wrong := f.anonymous(t)
	_, e := f.exchange(t, wrong, token, "wrong-cookie-key-01")
	want(t, e, auth.ErrDenied)
	_, e = f.do(t, wrong, "", f.request(t, "get_admission", "", "", id, nil))
	want(t, e, auth.ErrDenied)
	f.call(t, f.owner, "revoke_invite", f.w, r, i["invitation_id"].(string), map[string]any{})
	_, e = f.exchange(t, anon, token, "revoked-guest-key01")
	want(t, e, auth.ErrDenied)
	_, e = f.do(t, anon, "guest-token-replay1", f.request(t, "guest_token", "", "", id, map[string]any{}))
	want(t, e, auth.ErrDenied)
}
func TestGuestCannotCrossRoomOrBecomeAdministrator(t *testing.T) {
	f := newFixture(t, nil)
	r := f.create(t)
	i := f.invite(t, r, false, 4)
	guest, _, _ := f.guest(t, r, i)
	view := f.call(t, guest, "get_room", f.w, r, "", nil)
	p := view["participants"].([]any)[0].(map[string]any)["participant_id"].(string)
	f.call(t, f.owner, "set_role", f.w, r, p, map[string]any{"role": "host", "enabled": true})
	_, e := f.do(t, f.owner, "guest-admin-key001", f.request(t, "set_role", f.w, r, p, map[string]any{"role": "administrator", "enabled": true}))
	want(t, e, auth.ErrDenied)
	other := f.create(t)
	invite := f.invite(t, other, false, 4)
	_, e = f.do(t, guest, "guest-cross-key001", f.request(t, "request_admission", "", "", "", map[string]any{"mode": "guest", "invite_token": invite["invite_token"], "display_name": "Other"}))
	want(t, e, auth.ErrClaimRequired)
	_, e = f.do(t, guest, "guest-management01", f.request(t, "create_invite", f.w, r, "", map[string]any{"approval_required": false, "expires_in_seconds": 60, "max_uses": 1}))
	want(t, e, auth.ErrDenied)
	f.call(t, f.owner, "kick_participant", f.w, r, p, map[string]any{})
	_, e = f.do(t, guest, "", f.request(t, "get_room", f.w, r, "", nil))
	want(t, e, auth.ErrUnauthenticated)
}
func TestGuestExchangeRollbackAtEachAtomicWrite(t *testing.T) {
	for _, tc := range []struct {
		point      string
		occurrence int
	}{{"after-room-participant", 1}, {"after-room-invitation", 1}, {"after-room-admission", 1}, {"after-session", 2}, {"after-session", 3}, {"after-receipt", 1}, {"before-commit", 1}} {
		t.Run(fmt.Sprintf("%s_%d", tc.point, tc.occurrence), func(t *testing.T) {
			var armed atomic.Bool
			var hits atomic.Int32
			f := newFixture(t, func(_ context.Context, p string) error {
				if armed.Load() && p == tc.point && hits.Add(1) == int32(tc.occurrence) {
					return auth.ErrUnavailable
				}
				return nil
			})
			r := f.create(t)
			i := f.invite(t, r, false, 4)
			anon, id, token := f.prepareGuest(t, r, i)
			armed.Store(true)
			out, e := f.exchange(t, anon, token, "atomic-guest-key01")
			armed.Store(false)
			want(t, e, auth.ErrUnavailable)
			if hits.Load() < int32(tc.occurrence) {
				t.Fatal("fault point not executed")
			}
			if len(out.StorageValue().Body) != 0 || out.StorageValue().Cookie.StorageValue() != "" {
				t.Fatal("rollback returned outcome")
			}
			need(t, f.r.Transact(f.ctx, func(tx auth.Transaction) error {
				tr, e := f.store.Bind(tx.Core())
				if e != nil {
					return e
				}
				a, e := tr.Admission(f.ctx, id)
				if e != nil {
					return e
				}
				d := a.StorageValue()
				if d.TokenUsed || d.ParticipantID != "" {
					t.Fatal("admission survived rollback")
				}
				inv, e := tr.Invitation(f.ctx, d.Scope, d.InvitationID)
				if e != nil {
					return e
				}
				if inv.StorageValue().Uses != 0 {
					t.Fatal("quota survived rollback")
				}
				parts, e := tr.Participants(f.ctx, d.Scope)
				if e != nil {
					return e
				}
				if len(parts) != 0 {
					t.Fatal("participant survived rollback")
				}
				session, e := tx.Session(f.ctx, hashed(anon.StorageValue().Cookie.StorageValue()))
				if e != nil {
					return e
				}
				if session.StorageValue().Retired {
					t.Fatal("session retirement survived rollback")
				}
				return nil
			}))
			if f.sql(t, "SELECT count(*) FROM platform_core.guests WHERE workspace_id='"+f.w+"' AND room_id='"+r+"'") != "0" {
				t.Fatal("guest survived rollback")
			}
			_, e = f.exchange(t, anon, token, "atomic-guest-key01")
			need(t, e)
		})
	}
}
func TestGuestExchangeUnknownCommitRestartAndRevoke(t *testing.T) {
	var armed atomic.Bool
	f := newFixture(t, func(_ context.Context, p string) error {
		if p == "after-commit" && armed.Swap(false) {
			return auth.ErrUnavailable
		}
		return nil
	})
	r := f.create(t)
	i := f.invite(t, r, false, 4)
	anon, _, token := f.prepareGuest(t, r, i)
	armed.Store(true)
	out, e := f.exchange(t, anon, token, "unknown-guest-key01")
	want(t, e, auth.ErrOutcomeUnknown)
	if out.StorageValue().Cookie.StorageValue() != "" || len(out.StorageValue().Body) != 0 {
		t.Fatal("unknown commit returned secret outcome")
	}
	f.recompose(t)
	out, e = f.exchange(t, anon, token, "unknown-guest-key01")
	need(t, e)
	data := public(t, out)
	if data["principal"].(map[string]any)["kind"] != "guest" {
		t.Fatal("unknown commit did not recover guest")
	}
	replayed, e := f.exchange(t, anon, token, "unknown-guest-key01")
	need(t, e)
	if string(out.StorageValue().Body) != string(replayed.StorageValue().Body) || out.StorageValue().Cookie.StorageValue() != replayed.StorageValue().Cookie.StorageValue() {
		t.Fatal("recovered outcome changed")
	}
	guest := auth.RoomSecret(actorData{Cookie: out.StorageValue().Cookie, CSRF: data["csrf_token"].(string)})
	view := f.call(t, guest, "get_room", f.w, r, "", nil)
	rows := view["participants"].([]any)
	if len(rows) != 1 {
		t.Fatal("unknown retry duplicated participation")
	}
	pid := rows[0].(map[string]any)["participant_id"].(string)
	f.call(t, f.owner, "kick_participant", f.w, r, pid, map[string]any{})
	_, e = f.exchange(t, anon, token, "unknown-guest-key01")
	want(t, e, auth.ErrUnauthenticated)
}
func TestGuestClaimRetainsIdentityHostAndLongAccountNickname(t *testing.T) {
	f := newFixture(t, nil)
	r := f.create(t)
	i := f.invite(t, r, false, 4)
	guest, _, _ := f.guest(t, r, i)
	view := f.call(t, guest, "get_room", f.w, r, "", nil)
	pid := view["participants"].([]any)[0].(map[string]any)["participant_id"].(string)
	f.call(t, f.owner, "set_role", f.w, r, pid, map[string]any{"role": "host", "enabled": true})
	name := strings.Repeat("N", 128)
	login := fmt.Sprintf("u_%s_%d", prefix, sequence.Add(1))
	q := f.authRequest(t, "claim", map[string]any{"mode": "new_account", "login_name": login, "password": "synthetic-claim-password", "display_name": name})
	out, e := f.authority.Authentication().Mutate(f.ctx, guest.StorageValue().Cookie, guest.StorageValue().CSRF, "claim-long-name-01", "owned-network", q)
	need(t, e)
	claimed := public(t, out)
	accountContext := claimed["context"].(map[string]any)
	accountID := accountContext["principal"].(map[string]any)["account_id"].(string)
	actor := auth.RoomSecret(actorData{ID: accountID, Cookie: out.StorageValue().Cookie, CSRF: accountContext["csrf_token"].(string)})
	view = f.call(t, actor, "get_room", f.w, r, "", nil)
	p := view["participants"].([]any)[0].(map[string]any)
	if p["participant_id"] != pid || p["kind"] != "account" || p["display_name"] != name || len(p["roles"].([]any)) != 1 || p["roles"].([]any)[0] != "host" {
		t.Fatal("claim lost public participation projection")
	}
	if len(view["caller_roles"].([]any)) != 1 || view["caller_roles"].([]any)[0] != "host" {
		t.Fatal("claim gained management")
	}
	need(t, f.r.Transact(f.ctx, func(tx auth.Transaction) error {
		if _, e := tx.Core().Membership(f.ctx, f.w, accountID); e != core.ErrDenied {
			t.Fatal("claim created membership")
		}
		tr, e := f.store.Bind(tx.Core())
		if e != nil {
			return e
		}
		p, e := tr.Participant(f.ctx, core.Scope{WorkspaceID: f.w, RoomID: r, GameID: "game"}, pid)
		if e != nil {
			return e
		}
		if p.StorageValue().GuestID != claimed["participation"].(map[string]any)["participation_id"] {
			t.Fatal("claim changed guest identity")
		}
		return nil
	}))
}
func TestNicknameCompatibilityExistingAccountJoin(t *testing.T) {
	for _, n := range []int{80, 81, 128} {
		t.Run(fmt.Sprintf("nickname_%d", n), func(t *testing.T) {
			f := newFixture(t, nil)
			r := f.create(t)
			i := f.invite(t, r, false, 4)
			a := f.account(t)
			name := strings.Repeat("N", n)
			f.sql(t, "UPDATE platform_core.accounts SET display_name='"+name+"' WHERE id='"+a.StorageValue().ID+"'")
			d := f.join(t, a, i)
			if d["display_name"] != name {
				t.Fatal("account admission nickname truncated")
			}
			view := f.call(t, a, "get_room", f.w, r, "", nil)
			if view["participants"].([]any)[0].(map[string]any)["display_name"] != name {
				t.Fatal("account participant nickname truncated")
			}
		})
	}
}
