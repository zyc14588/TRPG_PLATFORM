//go:build integration

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package room_test

import (
	"fmt"
	"sync"
	"testing"

	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/room"
)

func TestRoomOwnerWorkspaceAndParticipationSeparation(t *testing.T) {
	f := newFixture(t, nil)
	r := f.create(t)
	other := f.account(t)
	q := f.request(t, "get_room", f.w, r, "", nil)
	_, e := f.do(t, other, "", q)
	want(t, e, auth.ErrDenied)
	need(t, f.r.Transact(f.ctx, func(tx auth.Transaction) error {
		return tx.Core().PutMembership(f.ctx, core.Membership{WorkspaceID: f.w, AccountID: other.StorageValue().ID, Role: core.Admin})
	}))
	_, e = f.do(t, other, "", q)
	want(t, e, auth.ErrDenied)
	created := f.call(t, other, "create_room", f.w, "", "", map[string]any{"name": "Admin-created room", "game_id": "other-game"})
	_, e = f.do(t, f.owner, "", f.request(t, "get_room", f.w, created["room_id"].(string), "", nil))
	want(t, e, auth.ErrDenied)
	view := f.call(t, f.owner, "get_room", f.w, r, "", nil)
	if len(view["participants"].([]any)) != 0 {
		t.Fatal("owner automatically became participant")
	}
	_, e = f.do(t, f.owner, "owner-leave-key-01", f.request(t, "leave_room", f.w, r, "", map[string]any{}))
	want(t, e, auth.ErrDenied)
	if room.AuthorizePrivateView() != auth.ErrDenied || room.AuthorizeSeatControl() != auth.ErrDenied {
		t.Fatal("management granted game capabilities")
	}
}
func TestConcurrentInvitationQuotaAndNoDoubleConsumption(t *testing.T) {
	f := newFixture(t, nil)
	r := f.create(t)
	i := f.invite(t, r, false, 3)
	actors := make([]actor, 16)
	requests := make([]room.Request, 16)
	for j := range actors {
		actors[j] = f.account(t)
		requests[j] = f.request(t, "request_admission", "", "", "", map[string]any{"mode": "account", "invite_token": i["invite_token"]})
	}
	start := make(chan struct{})
	results := make(chan error, 16)
	var wg sync.WaitGroup
	for j := range actors {
		wg.Add(1)
		go func(j int) {
			defer wg.Done()
			<-start
			_, e := f.do(t, actors[j], fmt.Sprintf("quota-request-%04d", j), requests[j])
			results <- e
		}(j)
	}
	close(start)
	wg.Wait()
	close(results)
	joined := 0
	for e := range results {
		if e == nil {
			joined++
		} else {
			want(t, e, auth.ErrConflict)
		}
	}
	if joined != 3 {
		t.Fatalf("quota admitted %d, expected 3", joined)
	}
	view := f.call(t, f.owner, "get_room", f.w, r, "", nil)
	if len(view["participants"].([]any)) != 3 {
		t.Fatal("concurrent participant count differs")
	}
	need(t, f.r.Transact(f.ctx, func(tx auth.Transaction) error {
		tr, e := f.store.Bind(tx.Core())
		if e != nil {
			return e
		}
		row, e := tr.Invitation(f.ctx, core.Scope{WorkspaceID: f.w, RoomID: r, GameID: "game"}, i["invitation_id"].(string))
		if e != nil {
			return e
		}
		if row.StorageValue().Uses != 3 {
			t.Fatal("quota use count differs")
		}
		return nil
	}))
	for j, a := range actors {
		out, e := f.do(t, a, fmt.Sprintf("quota-request-%04d", j), requests[j])
		if e == nil {
			if public(t, out)["status"] != "approved" {
				t.Fatal("retry result differs")
			}
		} else {
			want(t, e, auth.ErrConflict)
		}
	}
}
func TestPendingApprovalRejectionAndAccountRevocation(t *testing.T) {
	f := newFixture(t, nil)
	r := f.create(t)
	i := f.invite(t, r, true, 2)
	a := f.account(t)
	pending := f.join(t, a, i)
	id := pending["admission_id"].(string)
	if pending["status"] != "pending" || pending["participant_id"] != nil {
		t.Fatal("pending allocated participant")
	}
	view := f.call(t, f.owner, "get_room", f.w, r, "", nil)
	if len(view["participants"].([]any)) != 0 {
		t.Fatal("pending consumed room capacity")
	}
	_, e := f.do(t, a, "unauthorized-approve", f.request(t, "decide_admission", f.w, r, id, map[string]any{"decision": "approve"}))
	want(t, e, auth.ErrDenied)
	f.call(t, f.owner, "decide_admission", f.w, r, id, map[string]any{"decision": "reject"})
	_, e = f.do(t, a, "changed-join-key-01", f.request(t, "request_admission", "", "", "", map[string]any{"mode": "account", "room_code": i["room_code"]}))
	want(t, e, auth.ErrDenied)
	b := f.account(t)
	second := f.join(t, b, i)
	f.call(t, f.owner, "decide_admission", f.w, r, second["admission_id"].(string), map[string]any{"decision": "approve"})
	f.call(t, f.owner, "revoke_invite", f.w, r, i["invitation_id"].(string), map[string]any{})
	f.call(t, b, "get_room", f.w, r, "", nil)
	f.call(t, b, "leave_room", f.w, r, "", map[string]any{})
	_, e = f.do(t, b, "", f.request(t, "get_room", f.w, r, "", nil))
	want(t, e, auth.ErrDenied)
}
func TestRoomAdministratorAndHostNegativeMatrix(t *testing.T) {
	f := newFixture(t, nil)
	r := f.create(t)
	i := f.invite(t, r, false, 8)
	admin := f.account(t)
	host := f.account(t)
	need(t, f.r.Transact(f.ctx, func(tx auth.Transaction) error {
		return tx.Core().PutMembership(f.ctx, core.Membership{WorkspaceID: f.w, AccountID: admin.StorageValue().ID, Role: core.Member})
	}))
	pa := f.join(t, admin, i)["participant_id"].(string)
	ph := f.join(t, host, i)["participant_id"].(string)
	f.call(t, f.owner, "set_role", f.w, r, pa, map[string]any{"role": "administrator", "enabled": true})
	f.call(t, admin, "set_role", f.w, r, ph, map[string]any{"role": "host", "enabled": true})
	for _, q := range []room.Request{f.request(t, "create_invite", f.w, r, "", map[string]any{"approval_required": false, "expires_in_seconds": 60, "max_uses": 1}), f.request(t, "list_admissions", f.w, r, "", nil), f.request(t, "set_role", f.w, r, pa, map[string]any{"role": "administrator", "enabled": true})} {
		_, e := f.do(t, host, fmt.Sprintf("host-denied-%06d", sequence.Add(1)), q)
		want(t, e, auth.ErrDenied)
	}
	_, e := f.do(t, admin, "admin-close-key-01", f.request(t, "close_room", f.w, r, "", map[string]any{}))
	want(t, e, auth.ErrDenied)
	f.call(t, admin, "leave_room", f.w, r, "", map[string]any{})
	view := f.call(t, admin, "get_room", f.w, r, "", nil)
	roles := view["caller_roles"].([]any)
	if len(roles) != 1 || roles[0] != "administrator" {
		t.Fatal("administrator/participant dimensions merged")
	}
	need(t, f.r.Transact(f.ctx, func(tx auth.Transaction) error {
		return tx.Core().DeleteMembership(f.ctx, f.w, admin.StorageValue().ID)
	}))
	_, e = f.do(t, admin, "", f.request(t, "get_room", f.w, r, "", nil))
	want(t, e, auth.ErrDenied)
	need(t, f.r.Transact(f.ctx, func(tx auth.Transaction) error {
		return tx.Core().PutMembership(f.ctx, core.Membership{WorkspaceID: f.w, AccountID: admin.StorageValue().ID, Role: core.Member})
	}))
	_, e = f.do(t, admin, "", f.request(t, "get_room", f.w, r, "", nil))
	want(t, e, auth.ErrDenied)
}
func TestTenantScopeAndClosedRoomRevocation(t *testing.T) {
	f := newFixture(t, nil)
	r := f.create(t)
	i := f.invite(t, r, false, 8)
	guest, _, _ := f.guest(t, r, i)
	other := newFixture(t, nil)
	need(t, other.r.Transact(other.ctx, func(tx auth.Transaction) error {
		tr, e := other.store.Bind(tx.Core())
		if e != nil {
			return e
		}
		return tr.InsertRoom(other.ctx, auth.RoomSecret(room.RoomData{ID: r, Scope: core.Scope{WorkspaceID: other.w, RoomID: r, GameID: "game"}, Name: "Same-name tenant lobby", Owner: other.owner.StorageValue().ID, State: "lobby"}))
	}))
	_, e := f.do(t, guest, "", f.request(t, "get_room", other.w, r, "", nil))
	want(t, e, auth.ErrDenied)
	_, e = f.do(t, f.owner, "wrong-tenant-revoke", f.request(t, "revoke_invite", other.w, r, i["invitation_id"].(string), map[string]any{}))
	want(t, e, auth.ErrDenied)
	f.call(t, f.owner, "close_room", f.w, r, "", map[string]any{})
	_, e = f.do(t, guest, "", f.request(t, "get_room", f.w, r, "", nil))
	want(t, e, auth.ErrUnauthenticated)
	a := f.account(t)
	_, e = f.do(t, a, "closed-join-key-01", f.request(t, "request_admission", "", "", "", map[string]any{"mode": "account", "invite_token": i["invite_token"]}))
	want(t, e, auth.ErrDenied)
}
func TestIssuedSecretReplayChecksCurrentAuthorization(t *testing.T) {
	f := newFixture(t, nil)
	r := f.create(t)
	q := f.request(t, "create_invite", f.w, r, "", map[string]any{"approval_required": false, "expires_in_seconds": 3600, "max_uses": 4})
	out, e := f.do(t, f.owner, "issued-replay-key-01", q)
	need(t, e)
	i := public(t, out)
	again, e := f.do(t, f.owner, "issued-replay-key-01", q)
	need(t, e)
	if string(out.StorageValue().Body) != string(again.StorageValue().Body) {
		t.Fatal("invitation replay bytes differ")
	}
	f.call(t, f.owner, "revoke_invite", f.w, r, i["invitation_id"].(string), map[string]any{})
	_, e = f.do(t, f.owner, "issued-replay-key-01", q)
	want(t, e, auth.ErrDenied)
}
