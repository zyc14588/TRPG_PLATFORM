//go:build linux && integration

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package player_test

import (
	"fmt"
	"strconv"
	"strings"
	"sync"
	"testing"

	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
)

func TestPlayerHTTPSInstalledActorReceiptAndPrivateSeatFiltering(t *testing.T) {
	n := newPlayerFixture(t, false)
	n.configureAndLaunch(t)
	gm := n.snapshot(t, n.owner, n.hostConnection, "0", 8)
	player := n.snapshot(t, n.participant, n.playerConnection, "0", 8)
	private(t, gm, true)
	private(t, player, false)
	if player["view"].(map[string]any)["table"].(map[string]any)["pending_action"].(map[string]any)["string"] != "choose-player" {
		t.Fatal("own pending action missing")
	}
	r := n.api(t, n.owner, "command", n.commandFields(n.hostConnection, "once-only", "1"))
	private(t, r, false)
	if r["state_version"] != "2" || r["event_cursor"] != "1" {
		t.Fatal("authoritative HTTPS result counters mismatch")
	}
	player = n.snapshot(t, n.participant, n.playerConnection, "0", 8)
	private(t, player, false)
	if len(player["events"].([]any)) != 1 || player["event_cursor"] != "1" {
		t.Fatal("filtered committed event missing")
	}
	for _, kind := range []string{"public", "personal", "host", "administrator"} {
		a := n.owner
		if kind == "personal" {
			a = n.participant
		}
		x := n.api(t, a, "export", map[string]any{"kind": kind, "after_cursor": "0", "limit": 8})
		private(t, x, kind == "host" || kind == "administrator")
	}
	if n.denied(t, n.participant, "export", n.w, n.room, map[string]any{"kind": "host", "after_cursor": "0", "limit": 8}) != "DENIED" {
		t.Fatal("non-host export role changed")
	}
	point := n.api(t, n.owner, "create_point", nil)
	private(t, point, false)
	if point["state_version"] != "2" || point["event_cursor"] != "1" {
		t.Fatal("recovery metadata mismatch")
	}
	if n.api(t, n.participant, "read_point", nil)["state_version"] != "2" {
		t.Fatal("ordinary seat could not read redacted point")
	}
	if n.denied(t, n.participant, "create_point", n.w, n.room, map[string]any{}) != "DENIED" {
		t.Fatal("ordinary seat created recovery point")
	}
	if n.sql(t, `SELECT count(*) FROM host_command.requests WHERE workspace='`+n.w+`'`) != "1" {
		t.Fatal("HTTP response did not correspond to one native commit")
	}
}
func TestPlayerCatalogTenantGuestAndSeatlessAdminBoundaries(t *testing.T) {
	n := newPlayerFixture(t, true)
	games := n.api(t, n.owner, "catalog", nil)["games"].([]any)
	if len(games) != 3 {
		t.Fatal("installed account catalog incomplete")
	}
	private(t, games, false)
	if len(n.api(t, n.participant, "catalog", nil)["games"].([]any)) != 0 {
		t.Fatal("guest read an unconfigured workspace catalog")
	}
	n.configureAndLaunch(t)
	if len(n.api(t, n.participant, "catalog", nil)["games"].([]any)) != 1 {
		t.Fatal("guest catalog was not confined to room configuration")
	}
	other := newPlayerFixture(t, false)
	for _, action := range []string{"catalog", "lobby", "readiness", "read_point"} {
		if n.denied(t, n.participant, action, other.w, other.room, nil) != "DENIED" {
			t.Fatal("cross-tenant route authorization changed")
		}
	}
	admin := n.account(t)
	need(t, n.r.Transact(n.ctx, func(tx auth.Transaction) error {
		return tx.Core().PutMembership(n.ctx, core.Membership{WorkspaceID: n.w, AccountID: admin.StorageValue().ID, Role: core.Admin})
	}))
	if n.denied(t, admin, "export", n.w, n.room, map[string]any{"kind": "administrator", "after_cursor": "0", "limit": 8}) != "DENIED" {
		t.Fatal("seatless administrator obtained private observation")
	}
	if n.denied(t, n.participant, "command", n.w, n.room, n.commandFields(n.hostConnection, "stolen-connection", "1")) != "DENIED" {
		t.Fatal("connection identifier conferred another cookie's seat")
	}
	if n.sql(t, `SELECT count(*) FROM host_command.requests WHERE workspace='`+n.w+`'`) != "0" {
		t.Fatal("denied authority produced native effect")
	}
}
func TestPlayerOwnConsentRedactionAndPreparationInvalidation(t *testing.T) {
	n := newPlayerFixture(t, false)
	slots := []map[string]any{{"id": "gm", "mode": "human", "participant_id": n.hostPart}, {"id": "player", "mode": "human", "participant_id": n.playerPart}}
	n.api(t, n.owner, "configure", map[string]any{"configuration_id": "minimal", "slots": slots})
	n.api(t, n.owner, "consent", map[string]any{"revision": "1", "consent": true, "ready": true, "safety_confirmed": true, "boundaries": []string{"gore"}})
	n.api(t, n.participant, "consent", map[string]any{"revision": "1", "consent": true, "ready": true, "safety_confirmed": true, "boundaries": []string{"violence"}})
	guestLobby := n.api(t, n.participant, "lobby", nil)
	own := guestLobby["own_consent"].(map[string]any)["boundaries"].([]any)
	if len(own) != 1 || own[0] != "violence" {
		t.Fatal("another participant boundary escaped own consent projection")
	}
	n.api(t, n.owner, "configure", map[string]any{"configuration_id": "minimal", "slots": slots})
	lobby := n.api(t, n.participant, "readiness", nil)
	if lobby["revision"] != "2" || lobby["own_consent"].(map[string]any)["consent"].(bool) || lobby["readiness"].(map[string]any)["ready"].(bool) {
		t.Fatal("new preparation retained old consent")
	}
	if n.denied(t, n.participant, "consent", n.w, n.room, map[string]any{"revision": "1", "consent": true, "ready": true, "safety_confirmed": true, "boundaries": []string{}}) != "CONFLICT" {
		t.Fatal("stale consent revision accepted")
	}
	if n.denied(t, n.owner, "launch", n.w, n.room, map[string]any{"revision": "2"}) != "DENIED" {
		t.Fatal("readiness query replaced launch revalidation")
	}
}
func TestPlayerPauseResumeDisconnectExpiryAndRestartAreDurable(t *testing.T) {
	n := newPlayerFixture(t, false)
	n.configureAndLaunch(t)
	ctl := n.snapshot(t, n.owner, n.hostConnection, "0", 8)["control"].(map[string]any)
	ctl = n.api(t, n.participant, "pause", map[string]any{"expected_control_revision": ctl["revision"]})
	if !ctl["paused"].(bool) || ctl["reason"] != "safety" {
		t.Fatal("ordinary participant pause not persisted")
	}
	if n.denied(t, n.owner, "command", n.w, n.room, n.commandFields(n.hostConnection, "after-pause", "1")) != "PLAYER_PAUSED" {
		t.Fatal("paused command was not refused")
	}
	n.restart(t)
	newHost := n.api(t, n.owner, "connect", map[string]any{"after_cursor": "0"})
	n.hostConnection = newHost["connection_id"].(string)
	newPlayer := n.api(t, n.participant, "connect", map[string]any{"after_cursor": "0"})
	n.playerConnection = newPlayer["connection_id"].(string)
	ctl = newPlayer["control"].(map[string]any)
	if !ctl["paused"].(bool) || ctl["reason"] != "safety" {
		t.Fatal("restart or reconnect automatically resumed")
	}
	ctl = n.api(t, n.owner, "resume", map[string]any{"expected_control_revision": ctl["revision"]})
	if !ctl["paused"].(bool) {
		t.Fatal("host resumed for player")
	}
	ctl = n.api(t, n.participant, "resume", map[string]any{"expected_control_revision": ctl["revision"]})
	if ctl["paused"].(bool) {
		t.Fatal("explicit unanimous resume failed")
	}
	ctl = n.api(t, n.participant, "disconnect", map[string]any{"connection_id": n.playerConnection})
	if !ctl["paused"].(bool) || ctl["reason"] != "disconnect" {
		t.Fatal("necessary disconnect did not pause")
	}
	newPlayer = n.api(t, n.participant, "connect", map[string]any{"after_cursor": "0"})
	n.playerConnection = newPlayer["connection_id"].(string)
	ctl = newPlayer["control"].(map[string]any)
	if !ctl["paused"].(bool) {
		t.Fatal("disconnected seat automatically resumed")
	}
	ctl = n.api(t, n.owner, "resume", map[string]any{"expected_control_revision": ctl["revision"]})
	ctl = n.api(t, n.participant, "resume", map[string]any{"expected_control_revision": ctl["revision"]})
	n.sql(t, `UPDATE platform_player.leases SET expires_at=clock_timestamp()-interval '1 second' WHERE workspace_id='`+n.w+`' AND id='`+n.playerConnection+`'`)
	if n.denied(t, n.owner, "command", n.w, n.room, n.commandFields(n.hostConnection, "expired-human", "1")) != "PLAYER_PAUSED" {
		t.Fatal("expired necessary lease reached mutation")
	}
	if n.sql(t, `SELECT paused::text FROM platform_player.control WHERE workspace_id='`+n.w+`'`) != "true" {
		t.Fatal("expiry refusal rolled back its pause")
	}
	if n.sql(t, `SELECT count(*) FROM host_command.requests WHERE workspace='`+n.w+`'`) != "0" {
		t.Fatal("pause/disconnect/expiry changed native game state")
	}
}
func TestPlayerReplayRechecksControlCookieAndCurrentFilter(t *testing.T) {
	n := newPlayerFixture(t, false)
	n.configureAndLaunch(t)
	fields := n.commandFields(n.hostConnection, "stable-command", "1")
	key := "player-replay-owned-key"
	first, status := n.wire(t, n.owner, "command", n.w, n.room, key, fields)
	if status != 200 {
		t.Fatal("initial replay fixture command failed")
	}
	private(t, first, false)
	second, status := n.wire(t, n.owner, "command", n.w, n.room, key, fields)
	if status != 200 || !second["data"].(map[string]any)["replayed"].(bool) {
		t.Fatal("HTTP retry failed to return native original result")
	}
	ctl := n.snapshot(t, n.owner, n.hostConnection, "0", 8)["control"].(map[string]any)
	n.api(t, n.participant, "pause", map[string]any{"expected_control_revision": ctl["revision"]})
	paused, status := n.wire(t, n.owner, "command", n.w, n.room, key, fields)
	if status != 409 || paused["error"].(map[string]any)["code"] != "PLAYER_PAUSED" {
		t.Fatal("receipt replay bypassed current pause")
	}
	if n.sql(t, `SELECT count(*) FROM host_command.requests WHERE workspace='`+n.w+`'`) != "1" {
		t.Fatal("replay repeated native effect")
	}
	n.policy.mu.Lock()
	n.policy.private = false
	n.policy.mu.Unlock()
	private(t, n.snapshot(t, n.owner, n.hostConnection, "0", 8), false)
	need(t, n.r.Transact(n.ctx, func(tx auth.Transaction) error {
		s, e := tx.Session(n.ctx, hashed(n.owner.StorageValue().Cookie.StorageValue()))
		if e != nil {
			return e
		}
		v := s.StorageValue()
		v.Revoked = true
		return tx.PutSession(n.ctx, auth.StoredSession(v))
	}))
	if n.denied(t, n.owner, "snapshot", n.w, n.room, map[string]any{"connection_id": n.hostConnection, "after_cursor": "0", "limit": 8}) == "PLAYER_PAUSED" {
		t.Fatal("revoked cookie received game metadata")
	}
}
func TestPlayerPaginationMakesProgressWithoutExposingHiddenEvents(t *testing.T) {
	n := newPlayerFixture(t, false)
	n.configureAndLaunch(t)
	for i := 1; i <= 5; i++ {
		n.api(t, n.owner, "command", n.commandFields(n.hostConnection, fmt.Sprintf("event-%d", i), strconv.Itoa(i)))
	}
	after := "0"
	seen := 0
	for page := 0; page < 5; page++ {
		x := n.snapshot(t, n.participant, n.playerConnection, after, 2)
		private(t, x, false)
		events := x["events"].([]any)
		if len(events) > 2 {
			t.Fatal("page exceeded requested event limit")
		}
		seen += len(events)
		next := x["event_cursor"].(string)
		if next == after && x["more"].(bool) {
			t.Fatal("event page did not make progress")
		}
		after = next
		if !x["more"].(bool) {
			break
		}
	}
	if seen != 5 || after != "5" {
		t.Fatal("bounded pagination lost or repeated events")
	}
	n.policy.mu.Lock()
	n.policy.export = false
	n.policy.mu.Unlock()
	if n.denied(t, n.participant, "export", n.w, n.room, map[string]any{"kind": "personal", "after_cursor": "0", "limit": 8}) != "DENIED" {
		t.Fatal("export retained removed live policy")
	}
}
func TestPlayerPauseOrdersConcurrentCommandsBeforeAcknowledgment(t *testing.T) {
	n := newPlayerFixture(t, false)
	n.configureAndLaunch(t)
	ctl := n.snapshot(t, n.owner, n.hostConnection, "0", 8)["control"].(map[string]any)
	var wg sync.WaitGroup
	start := make(chan struct{})
	codes := make(chan string, 8)
	for i := 0; i < 8; i++ {
		wg.Add(1)
		go func(i int) {
			defer wg.Done()
			<-start
			fields := n.commandFields(n.hostConnection, fmt.Sprintf("race-%d", i), "1")
			fields["schema_version"] = 1
			_, e := n.direct(t, n.owner, "command", fmt.Sprintf("race-owned-%08d", i), fields)
			if e == nil {
				codes <- "OK"
			} else {
				codes <- e.Error()
			}
		}(i)
	}
	close(start)
	paused := n.api(t, n.participant, "pause", map[string]any{"expected_control_revision": ctl["revision"]})
	if !paused["paused"].(bool) {
		t.Fatal("pause did not acknowledge persistent state")
	}
	wg.Wait()
	close(codes)
	for code := range codes {
		switch code {
		case "OK", "PLAYER_PAUSED", "CONFLICT", "UNAVAILABLE", "OUTCOME_UNKNOWN", "RATE_LIMITED":
		default:
			t.Fatal("unsafe concurrent outcome")
		}
	}
	before := n.sql(t, `SELECT count(*) FROM host_command.requests WHERE workspace='`+n.w+`'`)
	for i := 0; i < 3; i++ {
		if n.denied(t, n.owner, "command", n.w, n.room, n.commandFields(n.hostConnection, fmt.Sprintf("after-ack-%d", i), "1")) != "PLAYER_PAUSED" {
			t.Fatal("command entered after pause acknowledgment")
		}
	}
	after := n.sql(t, `SELECT count(*) FROM host_command.requests WHERE workspace='`+n.w+`'`)
	if before != after {
		t.Fatal("native commit overtook pause acknowledgment")
	}
}
func TestPlayerHTTPSCSRFAndLeaseOwnershipNeverComeFromClient(t *testing.T) {
	n := newPlayerFixture(t, false)
	n.configureAndLaunch(t)
	wrong := n.owner.StorageValue()
	wrong.CSRF = strings.Repeat("B", 43)
	if n.denied(t, auth.RoomSecret(wrong), "command", n.w, n.room, n.commandFields(n.hostConnection, "wrong-csrf", "1")) != "DENIED" {
		t.Fatal("CSRF substitution reached Actor")
	}
	fields := n.commandFields(n.hostConnection, "forged-seat", "1")
	fields["seat_id"] = "player"
	if n.denied(t, n.owner, "command", n.w, n.room, fields) != "INVALID_REQUEST" {
		t.Fatal("client-selected seat accepted")
	}
	fields = n.commandFields(n.hostConnection, "forged-session", "1")
	fields["session_id"] = n.sessionID
	if n.denied(t, n.owner, "command", n.w, n.room, fields) != "INVALID_REQUEST" {
		t.Fatal("client-selected session accepted")
	}
}
