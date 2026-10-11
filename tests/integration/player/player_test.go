//go:build linux && integration

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package player_test

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"log"
	"net/http/httptest"
	"os"
	"strconv"
	"strings"
	"sync"
	"testing"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/archive"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/install"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/httpapi"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/launch"
	playerapi "github.com/zyc14588/TRPG_PLATFORM/internal/platform/player"
	platformsession "github.com/zyc14588/TRPG_PLATFORM/internal/platform/session"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/command"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/postgres"
)

func TestPlayerExecutionFenceRecoversAfterLosingItsOwnedBackend(t *testing.T) {
	f := newFixture(t, nil)
	p, e := postgres.NewPlatformPlayerStorage(f.r)
	need(t, e)
	b := data.Binding{Workspace: f.w, Session: "owned-execution-release", GraphHash: "sha256:" + strings.Repeat("a", 64)}
	release, e := p.ExecutionLock(f.ctx, b, true)
	need(t, e)
	defer release()
	key := "platform-player:" + b.Workspace + "/" + b.Session + "/" + b.GraphHash
	query := `SELECT pid FROM pg_locks WHERE database=(SELECT oid FROM pg_database WHERE datname=current_database()) AND locktype='advisory' AND classid=((hashtextextended('` + key + `',0)>>32)&4294967295)::oid AND objid=(hashtextextended('` + key + `',0)&4294967295)::oid AND granted`
	pid, e := strconv.Atoi(f.sql(t, query))
	if e != nil || pid < 1 {
		t.Fatal("owned execution fence backend missing")
	}
	if f.sql(t, "SELECT pg_terminate_backend("+strconv.Itoa(pid)+")") != "t" {
		t.Fatal("owned backend failure not exercised")
	}
	release()
	// The pool must remain usable and this exact fence must be reacquired.
	again, e := p.ExecutionLock(f.ctx, b, true)
	need(t, e)
	again()
}

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

func TestPlayerEndedHTTPSExportSurvivesRestartAndKeepsCurrentAuthority(t *testing.T) {
	n := newPlayerFixture(t, false)
	n.configureAndLaunch(t)
	live, e := n.native.ConnectPolling(n.ctx, n.caller(n.owner, "ended-native-connection"), n.w, n.room, 0)
	need(t, e)
	defer live.Close()
	n.api(t, n.owner, "command", n.commandFields(n.hostConnection, "before-ending", "1"))
	end := n.commandFields(n.hostConnection, "durable-ending", "2")
	end["type"] = "end"
	ended := n.api(t, n.owner, "command", end)
	if ended["state_version"] != "3" || n.sql(t, `SELECT count(*) FROM host_command.endings WHERE workspace='`+n.w+`'`) != "1" {
		t.Fatal("fixture session did not end durably once")
	}
	assertNoSessionVM := forbidEndedSessionVM(t, n)
	before := endedDurableInspection(t, n)
	if _, e := live.Reconnect(n.ctx, 0); e == nil {
		t.Fatal("native ended reconnect accepted")
	}
	if _, e := live.Submit(n.ctx, command.Envelope{SessionID: n.sessionID, SeatID: "gm", CommandID: "after-durable-ending", ExpectedStateVersion: 3, Type: "increment", Payload: checkpoint.Object(map[string]checkpoint.Value{"delta": checkpoint.Int(1)}), CorrelationID: "ended-native-denial"}); e == nil {
		t.Fatal("native ended command accepted")
	}
	// Discard every live lease/Actor before testing the existing export route.
	n.restart(t)
	fields := func(kind, after string, limit int) map[string]any {
		return map[string]any{"kind": kind, "after_cursor": after, "limit": limit}
	}
	first := endedExport(t, n, n.participant, fields("personal", "0", 1))
	private(t, first, false)
	if first["ended"] != true || first["state_version"] != "3" || first["event_cursor"] != "2" || first["next_cursor"] != "1" || first["more"] != true || len(first["events"].([]any)) != 1 {
		t.Fatal("cold ended export did not preserve bounded durable pagination")
	}
	last := endedExport(t, n, n.participant, fields("personal", "1", 1))
	private(t, last, false)
	if last["ended"] != true || last["state_version"] != "3" || last["next_cursor"] != "2" || last["more"] != false || len(last["events"].([]any)) != 1 {
		t.Fatal("cold ended export lost the filtered ending event")
	}
	for _, kind := range []string{"public", "host", "administrator"} {
		x := endedExport(t, n, n.owner, fields(kind, "0", 8))
		private(t, x, kind == "host" || kind == "administrator")
		if x["ended"] != true || x["state_version"] != "3" {
			t.Fatal("existing ended export profile unavailable")
		}
	}
	// Two actual HTTPS readers have separate short-lived authorization handles.
	var wg sync.WaitGroup
	start := make(chan struct{})
	type readResult struct {
		response map[string]any
		status   int
	}
	results := make(chan readResult, 2)
	for i := 0; i < 2; i++ {
		wg.Add(1)
		go func(i int) {
			defer wg.Done()
			<-start
			response, status := n.wire(t, n.participant, "export", n.w, n.room, fmt.Sprintf("ended-concurrent-%d", i), fields("personal", "0", 8))
			results <- readResult{response, status}
		}(i)
	}
	close(start)
	wg.Wait()
	close(results)
	reads := 0
	for result := range results {
		if result.status != 200 {
			t.Fatal("concurrent authorized ended export failed")
		}
		x := result.response["data"].(map[string]any)
		private(t, x, false)
		if x["ended"] != true || x["state_version"] != "3" || x["next_cursor"] != "2" {
			t.Fatal("concurrent ended export read different authoritative history")
		}
		reads++
	}
	if reads != 2 {
		t.Fatal("missing actual concurrent ended export result")
	}
	other := newPlayerFixture(t, false)
	other.configureAndLaunch(t)
	if n.configs[0].StorageValue().Request.Root != other.configs[0].StorageValue().Request.Root {
		t.Fatal("foreign fixture did not share the installed content hash")
	}
	deny := func(a actor, kind string) {
		t.Helper()
		response, status := n.wire(t, a, "export", n.w, n.room, fmt.Sprintf("ended-export-denial-%08d", sequence.Add(1)), fields(kind, "0", 8))
		if status != 403 || response["data"] != nil || response["error"].(map[string]any)["code"] != "DENIED" {
			t.Fatal("ended export authorization returned data or failed to deny")
		}
		private(t, response, false)
	}
	deny(other.owner, "personal")
	deny(n.participant, "host")
	deny(n.participant, "administrator")
	admin := n.account(t)
	need(t, n.r.Transact(n.ctx, func(tx auth.Transaction) error {
		return tx.Core().PutMembership(n.ctx, core.Membership{WorkspaceID: n.w, AccountID: admin.StorageValue().ID, Role: core.Admin})
	}))
	deny(admin, "administrator")
	n.policy.mu.Lock()
	n.policy.private = false
	n.policy.mu.Unlock()
	private(t, endedExport(t, n, n.owner, fields("personal", "0", 8)), false)
	n.policy.mu.Lock()
	n.policy.export = false
	n.policy.mu.Unlock()
	deny(n.owner, "personal")
	n.policy.mu.Lock()
	n.policy.export = true
	n.policy.mu.Unlock()
	n.sql(t, `UPDATE platform_room.participants SET host=false WHERE workspace_id='`+n.w+`' AND id='`+n.hostPart+`'`)
	deny(n.owner, "host")
	if endedDurableInspection(t, n) != before {
		t.Fatal("ended export reads or denials mutated durable bytes or record counts")
	}
	assertNoSessionVM()
}

func TestPlayerEndedHTTPSExportRechecksRevocationAndGuestLifetime(t *testing.T) {
	for _, name := range []string{"cookie", "account", "participant", "guest-expiry", "guest-disable"} {
		t.Run(name, func(t *testing.T) {
			n := newPlayerFixture(t, strings.HasPrefix(name, "guest"))
			n.configureAndLaunch(t)
			end := n.commandFields(n.hostConnection, "ended-authority", "1")
			end["type"] = "end"
			n.api(t, n.owner, "command", end)
			n.restart(t)
			assertNoSessionVM := forbidEndedSessionVM(t, n)
			before := endedDurableInspection(t, n)
			fields := map[string]any{"kind": "personal", "after_cursor": "0", "limit": 8}
			own := endedExport(t, n, n.participant, fields)
			private(t, own, false)
			if own["ended"] != true || own["state_version"] != "2" {
				t.Fatal("current account or guest could not export the ended session")
			}
			status, code := 401, "UNAUTHENTICATED"
			switch name {
			case "cookie":
				n.sql(t, `UPDATE platform_auth.sessions SET revoked=true WHERE account_id='`+n.participant.StorageValue().ID+`'`)
			case "account":
				need(t, n.r.Transact(n.ctx, func(tx auth.Transaction) error {
					return tx.Core().DisableAccount(n.ctx, n.participant.StorageValue().ID)
				}))
			case "participant":
				n.sql(t, `UPDATE platform_room.participants SET active=false WHERE workspace_id='`+n.w+`' AND id='`+n.playerPart+`'`)
				status, code = 403, "DENIED"
			case "guest-expiry":
				n.sql(t, `UPDATE platform_core.guests SET expires_at=clock_timestamp()-interval '1 second' WHERE workspace_id='`+n.w+`'`)
			case "guest-disable":
				n.sql(t, `UPDATE platform_core.guests SET disabled=true WHERE workspace_id='`+n.w+`'`)
			}
			response, actual := n.wire(t, n.participant, "export", n.w, n.room, "ended-revoked-"+name, fields)
			if actual != status || response["data"] != nil || response["error"].(map[string]any)["code"] != code {
				t.Fatal("ended export bypassed current account, participant or guest authorization")
			}
			private(t, response, false)
			if endedDurableInspection(t, n) != before {
				t.Fatal("ended authorization check changed durable bytes or record counts")
			}
			assertNoSessionVM()
		})
	}
}

func forbidEndedSessionVM(t *testing.T, n *playerFixture) func() {
	t.Helper()
	count := func() int {
		total := 0
		for _, e := range n.executions {
			if e.Case == "session-vm-ready" {
				total++
			}
		}
		return total
	}
	n.mu.Lock()
	before := count()
	quarantineBefore := 0
	for _, e := range n.executions {
		if e.Case == "host-quarantine-destroy" && e.Reaped {
			quarantineBefore++
		}
	}
	n.vmFault = func(e install.Execution) error {
		if e.Case == "session-vm-ready" {
			return auth.ErrUnavailable
		}
		// Retain the existing mandatory package/quarantine verification VMs.
		return nil
	}
	n.mu.Unlock()
	return func() {
		t.Helper()
		n.mu.Lock()
		after := count()
		quarantineAfter := 0
		for _, e := range n.executions {
			if e.Case == "host-quarantine-destroy" && e.Reaped {
				quarantineAfter++
			}
		}
		n.mu.Unlock()
		if after != before {
			t.Fatal("ended read recreated the session Actor/runtime VM")
		}
		if quarantineAfter <= quarantineBefore {
			t.Fatal("ended export skipped actual package/quarantine verification")
		}
		t.Logf("ended reads: session-vm-ready delta=0; reaped quarantine verification delta=%d", quarantineAfter-quarantineBefore)
	}
}

func endedDurableInspection(t *testing.T, n *playerFixture) postgres.RecoveryInspection {
	t.Helper()
	host, e := postgres.OpenHostRepository(n.ctx, dsn, nil)
	need(t, e)
	defer func() { need(t, host.Close()) }()
	b := data.Binding{Workspace: n.w, Session: n.sessionID, GraphHash: n.sql(t, `SELECT graph_hash FROM host_command.sessions WHERE workspace='`+n.w+`' AND session='`+n.sessionID+`'`)}
	v, e := host.InspectRecovery(n.ctx, b)
	need(t, e)
	return v
}

const endedPublicMarker = "fixture-ended-public-log"
const endedOwnMarker = "fixture-ended-player-private"
const endedOtherMarker = "fixture-ended-other-seat-private"

// The installed Lua emits real durable events. All three additional event types
// use the existing bounded event Schema and explicit trusted fixture policies.
func endedLogBuild(runtime install.RuntimeConfig, source string) (*archive.Package, install.PolicyConfig, error) {
	if source == "" {
		source = strings.Replace(IncrementSource, `M.on_session_end=function() host.event.emit("change",state());return {} end`, `M.on_session_end=function()
 host.event.emit("change",state())
 host.event.emit("public-log",{counter=host.state.get({"counter"}),secret="`+endedPublicMarker+`"})
 host.event.emit("player-log",{counter=host.state.get({"counter"}),secret="`+endedOwnMarker+`"})
 host.event.emit("other-log",{counter=host.state.get({"counter"}),secret="`+endedOtherMarker+`"})
 host.event.emit("gm-log",state())
 return {}
end`, 1)
	}
	pkg, policy, e := Build(runtime, source)
	if e == nil {
		contract := policy.Artifacts[string(pkg.ArtifactIdentity().Digest())].Host
		for _, kind := range []string{"public-log", "player-log", "other-log", "gm-log"} {
			contract.Events[PackageID+"/"+kind] = contract.Events[PackageID+"/change"]
		}
	}
	return pkg, policy, e
}

type endedLogPolicy struct {
	base       *nativePolicy
	mu         sync.Mutex
	publicOnly bool
}

func (p *endedLogPolicy) Current(ctx context.Context, access launch.SessionAccess) (platformsession.SeatPolicy, error) {
	base, e := p.base.Current(ctx, access)
	if e != nil {
		return base, e
	}
	p.mu.Lock()
	publicOnly := p.publicOnly
	p.mu.Unlock()
	seat := access.StorageValue().Seat
	filter := func(v command.ViewPolicy, kind platformsession.ExportKind) (command.ViewPolicy, error) {
		v, e := command.OwnPolicy(v)
		if e != nil {
			return v, e
		}
		v.EventFields[PackageID+"/public-log"] = []string{"counter", "secret"}
		if kind != platformsession.Public && !publicOnly {
			if seat == "player" {
				v.EventFields[PackageID+"/player-log"] = []string{"counter", "secret"}
			}
			if seat == "gm" {
				v.EventFields[PackageID+"/gm-log"] = []string{"counter", "secret"}
			}
			if kind == platformsession.Host || kind == platformsession.Administrator {
				v.EventFields[PackageID+"/player-log"] = []string{"counter", "secret"}
				v.EventFields[PackageID+"/other-log"] = []string{"counter", "secret"}
			}
		}
		return v, nil
	}
	base.View, e = filter(base.View, platformsession.Personal)
	if e != nil {
		return base, e
	}
	for kind, v := range base.Exports {
		base.Exports[kind], e = filter(v, kind)
		if e != nil {
			return base, e
		}
	}
	return base, nil
}

func composeEndedLogPlayer(t *testing.T, n *playerFixture, policy *endedLogPolicy) {
	t.Helper()
	n.players.Close()
	n.server.Close()
	storage, e := postgres.NewPlatformSessionStorage(n.r)
	need(t, e)
	n.native, e = platformsession.New(platformsession.Options{Launch: n.service, Storage: storage, Policies: policy})
	need(t, e)
	schema, e := os.ReadFile("../../../schemas/platform/platform-player-api-v1.schema.json")
	need(t, e)
	descriptions := []playerapi.Description{}
	for _, c := range n.configs {
		x := c.StorageValue()
		descriptions = append(descriptions, playerapi.Description{WorkspaceID: x.WorkspaceID, ConfigurationID: x.ID, GameID: "game", Title: "Owned installed game"})
	}
	n.players, e = playerapi.New(playerapi.Options{Context: n.ctx, Authority: n.authority, Launch: n.service, Sessions: n.native, Control: n.control, Descriptions: descriptions, Schema: schema, MaxConnections: 64})
	need(t, e)
	service := n.players
	t.Cleanup(service.Close)
	server := httptest.NewUnstartedServer(nil)
	handler, e := httpapi.NewPlayerHandler(n.players, n.rooms, "https://"+server.Listener.Addr().String())
	need(t, e)
	server.Config.Handler = handler
	server.Config.ErrorLog = log.New(io.Discard, "", 0)
	server.StartTLS()
	server.Client().Timeout = n.server.Client().Timeout
	n.server = server
	t.Cleanup(server.Close)
}

func endedLogMarkers(t *testing.T, value any, public, own, other, gm bool) {
	t.Helper()
	raw, e := json.Marshal(value)
	need(t, e)
	defer clear(raw)
	for marker, expected := range map[string]bool{endedPublicMarker: public, endedOwnMarker: own, endedOtherMarker: other, PrivateValue: gm} {
		if bytes.Contains(raw, []byte(marker)) != expected {
			t.Fatal("ended event privacy mismatch; private output withheld")
		}
	}
}

func TestPlayerEndedHTTPSExportFiltersOwnPrivateAndHiddenPagesByCurrentSeat(t *testing.T) {
	l := newLaunchFixtureWithBuilder(t, nil, endedLogBuild)
	n := playerFixtureWithLaunch(t, l, false)
	policy := &endedLogPolicy{base: n.policy}
	composeEndedLogPlayer(t, n, policy)
	unseated := n.account(t)
	n.configureAndLaunch(t)
	active := endedExport(t, n, n.participant, map[string]any{"kind": "personal", "after_cursor": "0", "limit": 8})
	if active["ended"] != false || active["view"].(map[string]any)["table"].(map[string]any)["pending_action"].(map[string]any)["string"] != "choose-player" {
		t.Fatal("active export lost the current native projection")
	}
	n.api(t, n.owner, "command", n.commandFields(n.hostConnection, "ended-marker-increment", "1"))
	ending := n.commandFields(n.hostConnection, "ended-marker-end", "2")
	ending["type"] = "end"
	n.api(t, n.owner, "command", ending)
	n.restart(t)
	composeEndedLogPlayer(t, n, policy)
	noSessionVM := forbidEndedSessionVM(t, n)
	before := endedDurableInspection(t, n)
	fields := func(kind, after string, limit int) map[string]any {
		return map[string]any{"kind": kind, "after_cursor": after, "limit": limit}
	}
	for cursor := 1; cursor <= 6; cursor++ {
		x := endedExport(t, n, n.participant, fields("personal", strconv.Itoa(cursor-1), 1))
		endedLogMarkers(t, x, cursor == 3, cursor == 4, false, false)
		events := 0
		if cursor <= 4 {
			events = 1
		}
		if x["ended"] != true || x["state_version"] != "3" || x["event_cursor"] != "6" || x["next_cursor"] != strconv.Itoa(cursor) || x["more"] != (cursor < 6) || len(x["events"].([]any)) != events || len(x["view"].(map[string]any)) != 1 || x["view"].(map[string]any)["kind"] != "table" {
			t.Fatal("ended filtered page lost final counters, empty projection or raw cursor progress")
		}
	}
	own := endedExport(t, n, n.participant, fields("personal", "0", 8))
	endedLogMarkers(t, own, true, true, false, false)
	policy.mu.Lock()
	policy.publicOnly = true
	policy.mu.Unlock()
	// A current policy can restrict a former participant to public-only reads;
	// no observer identity, public admission or workspace privilege is invented.
	limited := endedExport(t, n, n.participant, fields("personal", "0", 8))
	endedLogMarkers(t, limited, true, false, false, false)
	if len(limited["events"].([]any)) != 3 {
		t.Fatal("current public-only participant policy retained private event access")
	}
	public := endedExport(t, n, n.owner, fields("public", "0", 8))
	endedLogMarkers(t, public, true, false, false, false)
	if n.denied(t, unseated, "export", n.w, n.room, fields("personal", "0", 8)) != "DENIED" {
		t.Fatal("unseated account obtained ended export")
	}
	wrongRoom := n.create(t)
	if n.denied(t, n.participant, "export", n.w, wrongRoom, fields("personal", "0", 8)) != "DENIED" {
		t.Fatal("ended export accepted another room binding")
	}
	forged := fields("personal", "0", 8)
	forged["seat_id"] = "gm"
	if n.denied(t, n.participant, "export", n.w, n.room, forged) != "INVALID_REQUEST" {
		t.Fatal("ended export accepted a client-selected seat")
	}
	if after := endedDurableInspection(t, n); after != before {
		t.Fatal("ended marker exports or denials mutated authoritative state/history/checkpoint/outbox")
	}
	noSessionVM()
}

func endedExport(t *testing.T, n *playerFixture, a actor, fields map[string]any) map[string]any {
	t.Helper()
	response, status := n.wire(t, a, "export", n.w, n.room, fmt.Sprintf("ended-export-read-%08d", sequence.Add(1)), fields)
	if status != 200 {
		code := "unknown"
		if e, ok := response["error"].(map[string]any); ok {
			code, _ = e["code"].(string)
		}
		t.Fatalf("ended HTTPS export after durable End returned HTTP %d code=%s; private body withheld", status, code)
	}
	data, ok := response["data"].(map[string]any)
	if !ok {
		t.Fatal("ended HTTPS export data missing")
	}
	return data
}
