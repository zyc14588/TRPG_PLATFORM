//go:build linux && integration

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package platform_session_test

import (
	"context"
	"encoding/json"
	"fmt"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/launch"
	platformsession "github.com/zyc14588/TRPG_PLATFORM/internal/platform/session"
)

func TestNativeAuthenticatedTwoSeatCommandsAndPrivateDelivery(t *testing.T) {
	n := newNativeFixture(t, false)
	gm, player := n.connect(t, n.owner, 0), n.connect(t, n.player, 0)
	firstGM, firstPlayer := nextNative(t, n, gm), nextNative(t, n, player)
	checkPrivate(t, firstGM.StorageValue(), true)
	checkPrivate(t, firstPlayer.StorageValue(), false)
	if firstPlayer.StorageValue().View.Table["pending_action"].String != "choose-player" {
		t.Fatal("own pending action not restored")
	}
	c := n.envelope("first-command", "gm", "increment", 1)
	r, e := gm.Submit(n.ctx, c)
	need(t, e)
	if r.StorageValue().Version != 2 || r.StorageValue().Cursor != 1 {
		t.Fatal("native authoritative receipt mismatch")
	}
	checkPrivate(t, r.StorageValue(), false)
	for _, connection := range []*platformsession.Connection{gm, player} {
		frame := nextNative(t, n, connection)
		checkPrivate(t, frame.StorageValue(), connection == gm)
		if frame.StorageValue().Version != 2 || frame.StorageValue().Cursor != 1 {
			t.Fatal("delivery before authoritative commit")
		}
	}
	saved, e := n.storage.RuntimeRepository().LookupCommand(n.ctx, n.launched.StorageValue().Binding, n.hostPart, c.CommandID)
	need(t, e)
	if saved.Inputs.Time <= 0 || len(saved.Inputs.Random) != 1 || saved.Inputs.Random[0] < 0 || saved.Inputs.Random[0] >= 10 {
		t.Fatal("trusted time/random facts not recorded")
	}
	for _, kind := range []platformsession.ExportKind{platformsession.Public, platformsession.Personal} {
		out, e := player.Export(n.ctx, kind, 0, 8)
		need(t, e)
		checkPrivate(t, out.StorageValue(), false)
		if len(out.StorageValue().Events) != 1 {
			t.Fatal("authorized exported event missing")
		}
	}
	public, e := gm.Export(n.ctx, platformsession.Public, 0, 8)
	need(t, e)
	checkPrivate(t, public.StorageValue(), false)
}
func TestNativeReconnectRecoveryAndDuplicateNeverRepeatCommittedEffect(t *testing.T) {
	n := newNativeFixture(t, false)
	gm := n.connect(t, n.owner, 0)
	_ = nextNative(t, n, gm)
	c := n.envelope("once-only", "gm", "increment", 1)
	first, e := gm.Submit(n.ctx, c)
	need(t, e)
	_ = nextNative(t, n, gm)
	point, e := gm.CreateRecoveryPoint(n.ctx)
	need(t, e)
	checkPrivate(t, point.StorageValue(), false)
	if point.StorageValue().Version != 2 || point.StorageValue().Cursor != 1 {
		t.Fatal("durable point metadata mismatch")
	}
	n.policy.mu.Lock()
	before := n.policy.inputs
	n.policy.mu.Unlock()
	gm.Close()
	need(t, n.service.Close())
	n.recomposeLaunch(t, n.configs)
	n.compose(t)
	gm = n.connect(t, n.owner, 0)
	frame := nextNative(t, n, gm)
	if frame.StorageValue().Version != 2 || frame.StorageValue().Cursor != 1 || frame.StorageValue().View.Table["pending_action"].String != "choose-gm" {
		t.Fatal("cold recovery lost cursor, view or pending action")
	}
	checkPrivate(t, frame.StorageValue(), true)
	again, e := gm.Submit(n.ctx, c)
	need(t, e)
	if !again.StorageValue().Replayed || again.StorageValue().Version != first.StorageValue().Version || again.StorageValue().Result.Number != first.StorageValue().Result.Number {
		t.Fatal("duplicate did not return original result")
	}
	n.policy.mu.Lock()
	after := n.policy.inputs
	n.policy.mu.Unlock()
	if before != after {
		t.Fatal("duplicate generated another random/time fact")
	}
	ctx, cancel := context.WithTimeout(n.ctx, 30*time.Millisecond)
	defer cancel()
	if _, e = gm.Next(ctx); e == nil {
		t.Fatal("duplicate broadcast another event")
	}
	for table, expected := range map[string]string{"sessions": "1", "requests": "1", "events": "1"} {
		if n.sql(t, `SELECT count(*) FROM host_command.`+table+` WHERE workspace='`+n.w+`'`) != expected {
			t.Fatal("duplicate/cold recovery changed immutable effect count")
		}
	}
	restored, e := gm.RecoveryPoint(n.ctx)
	need(t, e)
	if restored.StorageValue() != point.StorageValue() {
		t.Fatal("recovery metadata not retained after restart")
	}
}
func TestNativeRejectsClientSeatSessionVersionAndCSRFSubstitution(t *testing.T) {
	n := newNativeFixture(t, false)
	gm := n.connect(t, n.owner, 0)
	_ = nextNative(t, n, gm)
	for _, name := range []string{"seat", "session", "version", "type"} {
		c := n.envelope("forged-"+name, "gm", "increment", 1)
		switch name {
		case "seat":
			c.SeatID = "player"
		case "session":
			c.SessionID = "another-session"
		case "version":
			c.ExpectedStateVersion = 0
		case "type":
			c.Type = "unapproved"
		}
		if _, e := gm.Submit(n.ctx, c); e == nil {
			t.Fatal("client supplied authority field accepted")
		}
	}
	wrong := n.owner.StorageValue()
	wrong.CSRF = strings.Repeat("B", 43)
	c, e := n.native.Connect(n.ctx, n.caller(auth.RoomSecret(wrong), "wrong-csrf"), n.w, n.room, 0)
	need(t, e)
	defer c.Close()
	_ = nextNative(t, n, c)
	if _, e = c.Submit(n.ctx, n.envelope("wrong-csrf-command", "gm", "increment", 1)); e != auth.ErrDenied {
		t.Fatal("wrong CSRF reached authoritative command")
	}
	if n.sql(t, `SELECT count(*) FROM host_command.requests WHERE workspace='`+n.w+`'`) != "0" {
		t.Fatal("denied request left authoritative effects")
	}
}
func TestNativeCurrentRevocationRejectsQueuedViewCommandAndExport(t *testing.T) {
	for _, name := range []string{"cookie", "account", "participant", "guest-expiry", "guest-revoke"} {
		t.Run(name, func(t *testing.T) {
			guest := strings.HasPrefix(name, "guest")
			n := newNativeFixture(t, guest)
			player := n.connect(t, n.player, 0)
			switch name {
			case "cookie":
				need(t, n.r.Transact(n.ctx, func(tx auth.Transaction) error {
					s, e := tx.Session(n.ctx, hashed(n.player.StorageValue().Cookie.StorageValue()))
					if e != nil {
						return e
					}
					v := s.StorageValue()
					v.Revoked = true
					return tx.PutSession(n.ctx, auth.StoredSession(v))
				}))
			case "account":
				need(t, n.r.Transact(n.ctx, func(tx auth.Transaction) error { return tx.Core().DisableAccount(n.ctx, n.player.StorageValue().ID) }))
			case "participant":
				n.sql(t, `UPDATE platform_room.participants SET active=false WHERE workspace_id='`+n.w+`' AND id='`+n.playerPart+`'`)
			case "guest-expiry":
				n.sql(t, `UPDATE platform_core.guests SET expires_at=clock_timestamp()-interval '1 second' WHERE workspace_id='`+n.w+`'`)
			case "guest-revoke":
				n.sql(t, `UPDATE platform_core.guests SET disabled=true WHERE workspace_id='`+n.w+`'`)
			}
			if _, e := player.Next(n.ctx); e == nil {
				t.Fatal("queued frame delivered after revocation")
			}
			if _, e := player.Submit(n.ctx, n.envelope("revoked", "player", "increment", 1)); e == nil {
				t.Fatal("revoked command executed")
			}
			if _, e := player.Export(n.ctx, platformsession.Personal, 0, 4); e == nil {
				t.Fatal("revoked export exposed")
			}
			if n.sql(t, `SELECT count(*) FROM host_command.requests WHERE workspace='`+n.w+`'`) != "0" {
				t.Fatal("revoked identity produced effects")
			}
		})
	}
}
func TestNativeQueuedFrameAndExportUseNarrowedLivePolicy(t *testing.T) {
	n := newNativeFixture(t, false)
	gm := n.connect(t, n.owner, 0)
	n.policy.mu.Lock()
	n.policy.private = false
	n.policy.mu.Unlock()
	frame := nextNative(t, n, gm)
	checkPrivate(t, frame.StorageValue(), false)
	r, e := gm.Submit(n.ctx, n.envelope("after-narrowing", "gm", "increment", 1))
	need(t, e)
	_ = r
	frame = nextNative(t, n, gm)
	checkPrivate(t, frame.StorageValue(), false)
	exported, e := gm.Export(n.ctx, platformsession.Personal, 0, 8)
	need(t, e)
	checkPrivate(t, exported.StorageValue(), false)
	n.policy.mu.Lock()
	n.policy.commands = false
	n.policy.export = false
	n.policy.mu.Unlock()
	if _, e = gm.Submit(n.ctx, n.envelope("removed-command", "gm", "increment", 2)); e != auth.ErrDenied {
		t.Fatal("removed package command retained")
	}
	if _, e = gm.Export(n.ctx, platformsession.Personal, 0, 8); e != auth.ErrDenied {
		t.Fatal("removed package export retained")
	}
}
func TestNativeManagementRoleNeverGrantsAnUnseatedObserver(t *testing.T) {
	n := newNativeFixture(t, false)
	admin := n.account(t)
	need(t, n.r.Transact(n.ctx, func(tx auth.Transaction) error {
		return tx.Core().PutMembership(n.ctx, core.Membership{WorkspaceID: n.w, AccountID: admin.StorageValue().ID, Role: core.Admin})
	}))
	if _, e := n.native.Connect(n.ctx, n.caller(admin, "unseated-admin"), n.w, n.room, 0); e != auth.ErrDenied {
		t.Fatal("unseated administrator gained game-private transport")
	}
	player := n.connect(t, n.player, 0)
	_ = nextNative(t, n, player)
	if _, e := player.CreateRecoveryPoint(n.ctx); e != auth.ErrDenied {
		t.Fatal("ordinary participant saved privileged recovery point")
	}
	if _, e := player.Export(n.ctx, platformsession.Host, 0, 4); e != auth.ErrDenied {
		t.Fatal("ordinary player exported host secrets")
	}
	gm := n.connect(t, n.owner, 0)
	_ = nextNative(t, n, gm)
	n.call(t, n.owner, "set_role", n.w, n.room, n.hostPart, map[string]any{"role": "host", "enabled": false})
	if _, e := gm.CreateRecoveryPoint(n.ctx); e != auth.ErrDenied {
		t.Fatal("withdrawn host saved recovery point")
	}
	if _, e := gm.Export(n.ctx, platformsession.Host, 0, 4); e != auth.ErrDenied {
		t.Fatal("withdrawn host exported host profile")
	}
	if _, e := gm.Submit(n.ctx, n.envelope("revoked-host-end", "gm", "end", 1)); e != auth.ErrDenied {
		t.Fatal("withdrawn host ended game")
	}
}
func TestNativeTenantAndRoomBindingsCannotShareObjectHashAuthority(t *testing.T) {
	a, b := newNativeFixture(t, false), newNativeFixture(t, false)
	if a.launched.StorageValue().Binding.GraphHash != b.launched.StorageValue().Binding.GraphHash {
		t.Fatal("fixture did not use the same content-addressed graph")
	}
	if _, e := a.native.Connect(a.ctx, a.caller(a.owner, "cross-tenant"), b.w, b.room, 0); e != auth.ErrDenied {
		t.Fatal("object identity conferred cross-workspace participation")
	}
	wrong := b.launched.StorageValue().Binding
	scope := scopeFor(a)
	if _, e := a.sessions.ReadPage(a.ctx, scope, wrong, 0, 4); e != auth.ErrDenied {
		t.Fatal("storage read escaped workspace boundary")
	}
	wrong = a.launched.StorageValue().Binding
	scope.RoomID = b.room
	if _, e := a.sessions.ReadPage(a.ctx, scope, wrong, 0, 4); e != auth.ErrDenied {
		t.Fatal("storage read escaped room binding")
	}
}
func TestNativeRecoveryExportPaginationAndEndedResultsRemainFiltered(t *testing.T) {
	n := newNativeFixture(t, false)
	gm := n.connect(t, n.owner, 0)
	_ = nextNative(t, n, gm)
	for v := uint64(1); v <= 3; v++ {
		_, e := gm.Submit(n.ctx, n.envelope(fmt.Sprintf("page-%d", v), "gm", "increment", v))
		need(t, e)
		_ = nextNative(t, n, gm)
	}
	first, e := gm.Export(n.ctx, platformsession.Public, 0, 2)
	need(t, e)
	f := first.StorageValue()
	checkPrivate(t, f, false)
	if len(f.Events) != 2 || !f.More || f.NextCursor != 2 || f.Cursor != 3 {
		t.Fatal("bounded export silently lost pagination")
	}
	second, e := gm.Export(n.ctx, platformsession.Public, f.NextCursor, 2)
	need(t, e)
	if len(second.StorageValue().Events) != 1 || second.StorageValue().More || second.StorageValue().NextCursor != 3 {
		t.Fatal("bounded export continuation failed")
	}
	_, e = gm.CreateRecoveryPoint(n.ctx)
	need(t, e)
	_, e = gm.Submit(n.ctx, n.envelope("end-session", "gm", "end", 4))
	need(t, e)
	ending, e := gm.Export(n.ctx, platformsession.Public, 3, 2)
	need(t, e)
	checkPrivate(t, ending.StorageValue(), false)
	if !ending.StorageValue().Ended || ending.StorageValue().Version != 5 || len(ending.StorageValue().Events) != 1 {
		t.Fatal("ended game results unavailable or unfiltered")
	}
}
func TestNativeDisconnectRetainsHumanSeatAndPendingActionWithoutAIHandoff(t *testing.T) {
	n := newNativeFixture(t, false)
	player := n.connect(t, n.player, 0)
	_ = nextNative(t, n, player)
	player.Close()
	again := n.connect(t, n.player, 0)
	frame := nextNative(t, n, again)
	if frame.StorageValue().View.Table["pending_action"].String != "choose-player" {
		t.Fatal("human pending action changed after disconnect")
	}
	if n.sql(t, `SELECT count(*) FROM host_command.requests WHERE workspace='`+n.w+`'`) != "0" {
		t.Fatal("disconnect executed a game command")
	}
	var prep launch.Preparation
	need(t, n.authority.Inspect(n.ctx, n.owner.StorageValue().Cookie, "", false, func(ctx context.Context, tx auth.Transaction, _ auth.SessionData) error {
		bound, e := n.storage.Bind(tx.Core())
		if e != nil {
			return e
		}
		prep, e = bound.Preparation(ctx, scopeFor(n))
		return e
	}))
	for _, slot := range prep.StorageValue().Slots {
		if slot.ID == "player" && (slot.Mode != "human" || slot.ParticipantID != n.playerPart || slot.ModelSelection != "") {
			t.Fatal("human seat silently switched to AI")
		}
	}
}
func TestNativeQueuePressureDropsOnlySlowConnectionAndReclaimsCapacity(t *testing.T) {
	n := newNativeFixture(t, false)
	slow := n.connect(t, n.owner, 0)
	for v := uint64(1); v <= 16; v++ {
		_, e := slow.Submit(n.ctx, n.envelope(fmt.Sprintf("slow-%d", v), "gm", "increment", v))
		need(t, e)
	}
	if _, e := slow.Next(n.ctx); e == nil {
		t.Fatal("over-capacity connection still delivered")
	}
	replacement := n.connect(t, n.owner, 16)
	frame := nextNative(t, n, replacement)
	if frame.StorageValue().Cursor != 16 || frame.StorageValue().Version != 17 {
		t.Fatal("backpressure recovery lost authoritative state")
	}
}
func TestNativeDuplicateConcurrentCommandsHaveOneWriterAndOneRandomFact(t *testing.T) {
	n := newNativeFixture(t, false)
	gm := n.connect(t, n.owner, 0)
	_ = nextNative(t, n, gm)
	var wg sync.WaitGroup
	fail := make(chan error, 4)
	c := n.envelope("concurrent-once", "gm", "increment", 1)
	for range 4 {
		wg.Add(1)
		go func() {
			defer wg.Done()
			r, e := gm.Submit(n.ctx, c)
			if e != nil {
				fail <- e
				return
			}
			if r.StorageValue().Version != 2 {
				fail <- auth.ErrConflict
			}
		}()
	}
	wg.Wait()
	close(fail)
	for e := range fail {
		need(t, e)
	}
	n.policy.mu.Lock()
	inputs := n.policy.inputs
	n.policy.mu.Unlock()
	if inputs != 1 || n.sql(t, `SELECT count(*) FROM host_command.requests WHERE workspace='`+n.w+`'`) != "1" {
		t.Fatal("duplicate concurrent command repeated server inputs/effects")
	}
	_ = nextNative(t, n, gm)
}
func TestNativeBadEnvelopeAndFailedLuaLeaveNoBroadcastOrAuthoritativeEffect(t *testing.T) {
	n := newNativeFixture(t, false)
	gm := n.connect(t, n.owner, 0)
	_ = nextNative(t, n, gm)
	if _, e := gm.Submit(n.ctx, n.envelope("lua-fail", "gm", "fail", 1)); e == nil {
		t.Fatal("failing Lua command accepted")
	}
	if n.sql(t, `SELECT count(*) FROM host_command.requests WHERE workspace='`+n.w+`'`) != "0" {
		t.Fatal("failed Lua command left receipt")
	}
	ctx, cancel := context.WithTimeout(n.ctx, 30*time.Millisecond)
	defer cancel()
	if _, e := gm.Next(ctx); e == nil {
		t.Fatal("failed command broadcast")
	}
	out, e := gm.Submit(n.ctx, n.envelope("retry-after-failure", "gm", "increment", 1))
	need(t, e)
	if out.StorageValue().Version != 2 {
		t.Fatal("failed command advanced state version")
	}
}
func TestNativeHandlesAndExportsNeverContainBrowserCredentials(t *testing.T) {
	n := newNativeFixture(t, false)
	gm := n.connect(t, n.owner, 0)
	frame := nextNative(t, n, gm)
	export, e := gm.Export(n.ctx, platformsession.Personal, 0, 4)
	need(t, e)
	rawCookie := n.owner.StorageValue().Cookie.StorageValue()
	csrf := n.owner.StorageValue().CSRF
	for _, v := range []any{gm, n.native, n.sessions, frame, export} {
		for _, verb := range []string{"%v", "%+v", "%#v", "%d", "%f", "%w", "%*v"} {
			text := fmt.Sprintf(verb, v)
			if strings.Contains(text, rawCookie) || strings.Contains(text, csrf) || strings.Contains(text, PrivateValue) {
				t.Fatal("ordinary formatting leaked protected data")
			}
		}
		if _, e := json.Marshal(v); e == nil {
			t.Fatal("opaque handle exported JSON")
		}
	}
	b, e := json.Marshal(export.StorageValue())
	need(t, e)
	defer clear(b)
	if strings.Contains(string(b), rawCookie) || strings.Contains(string(b), csrf) {
		t.Fatal("filtered export contains raw browser credentials")
	}
}
