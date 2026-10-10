//go:build linux && integration

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package player_test

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"log"
	"net/http"
	"net/http/httptest"
	"os"
	"testing"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/httpapi"
	playerapi "github.com/zyc14588/TRPG_PLATFORM/internal/platform/player"
	platformsession "github.com/zyc14588/TRPG_PLATFORM/internal/platform/session"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/postgres"
)

type playerFixture struct {
	*launchFixture
	players                                                                 *playerapi.Service
	native                                                                  *platformsession.Service
	policy                                                                  *nativePolicy
	server                                                                  *httptest.Server
	room, hostPart, playerPart, hostConnection, playerConnection, sessionID string
	participant                                                             actor
	participantLogin                                                        string
}

func newPlayerFixture(t *testing.T, guest bool) *playerFixture {
	t.Helper()
	l := newLaunchFixture(t, nil)
	return playerFixtureWithLaunch(t, l, guest)
}
func playerFixtureWithLaunch(t *testing.T, l *launchFixture, guest bool) *playerFixture {
	t.Helper()
	n := &playerFixture{launchFixture: l, policy: &nativePolicy{private: true, commands: true, points: true, export: true}}
	n.room, n.hostPart = l.hostRoom(t)
	invite := l.invite(t, n.room, false, 4)
	if guest {
		n.participant, _, _ = l.guest(t, n.room, invite)
		n.playerPart = l.sql(t, `SELECT id FROM platform_room.participants WHERE workspace_id='`+l.w+`' AND room_id='`+n.room+`' AND guest_id IS NOT NULL`)
	} else {
		n.participant, n.participantLogin = l.register(t, "旅人")
		n.playerPart = l.join(t, n.participant, invite)["participant_id"].(string)
	}
	n.composePlayer(t)
	return n
}
func (n *playerFixture) composePlayer(t *testing.T) {
	t.Helper()
	if n.players != nil {
		n.players.Close()
	}
	if n.server != nil {
		n.server.Close()
	}
	ss, e := postgres.NewPlatformSessionStorage(n.r)
	need(t, e)
	n.native, e = platformsession.New(platformsession.Options{Launch: n.service, Storage: ss, Policies: n.policy})
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
	origin := "https://" + server.Listener.Addr().String()
	handler, e := httpapi.NewPlayerHandler(n.players, n.rooms, origin)
	need(t, e)
	server.Config.Handler = handler
	server.Config.ErrorLog = log.New(io.Discard, "", 0)
	server.StartTLS()
	server.Client().Timeout = 7 * time.Second
	n.server = server
	t.Cleanup(server.Close)
}
func (n *playerFixture) path(action, w, r string) (method, path string) {
	prefix := "/api/v1/workspaces/" + w
	if action == "catalog" {
		return "GET", prefix + "/games"
	}
	prefix += "/rooms/" + r + "/"
	switch action {
	case "lobby":
		return "GET", prefix + "preparation"
	case "readiness":
		return "GET", prefix + "readiness"
	case "configure":
		return "POST", prefix + "preparation"
	case "consent", "launch":
		return "POST", prefix + action
	case "command":
		return "POST", prefix + "session/commands"
	case "create_point":
		return "POST", prefix + "session/recovery-point"
	case "read_point":
		return "GET", prefix + "session/recovery-point"
	default:
		return "POST", prefix + "session/" + action
	}
}
func (n *playerFixture) wire(t *testing.T, a actor, action, w, r, key string, fields map[string]any) (map[string]any, int) {
	t.Helper()
	method, path := n.path(action, w, r)
	var raw []byte
	if method != "GET" {
		if fields == nil {
			fields = map[string]any{}
		}
		fields["schema_version"] = 1
		var e error
		raw, e = json.Marshal(fields)
		need(t, e)
		defer clear(raw)
	}
	request, e := http.NewRequestWithContext(n.ctx, method, n.server.URL+path, bytes.NewReader(raw))
	need(t, e)
	v := a.StorageValue()
	request.AddCookie(&http.Cookie{Name: httpapi.SessionCookie, Value: v.Cookie.StorageValue()})
	if method != "GET" {
		request.Header.Set("Origin", n.server.URL)
		request.Header.Set("X-CSRF-Token", v.CSRF)
		request.Header.Set("Idempotency-Key", key)
		request.Header.Set("Content-Type", "application/json")
	}
	request.Header.Set("Sec-Fetch-Site", "same-origin")
	client := n.server.Client()
	response, e := client.Do(request)
	need(t, e)
	defer response.Body.Close()
	body, e := io.ReadAll(io.LimitReader(response.Body, playerapi.MaxResponseBytes+1))
	need(t, e)
	defer clear(body)
	if len(body) > playerapi.MaxResponseBytes || response.Header.Get("Cache-Control") != "no-store" || response.Header.Get("Referrer-Policy") != "no-referrer" {
		t.Fatal("private response bound or headers missing")
	}
	var envelope map[string]any
	if json.Unmarshal(body, &envelope) != nil {
		t.Fatal("public player envelope invalid; body withheld")
	}
	return envelope, response.StatusCode
}
func (n *playerFixture) api(t *testing.T, a actor, action string, fields map[string]any) map[string]any {
	t.Helper()
	envelope, status := n.wire(t, a, action, n.w, n.room, fmt.Sprintf("player-owned-request-%08d", sequence.Add(1)), fields)
	if status != http.StatusOK {
		code := "unknown"
		if x, ok := envelope["error"].(map[string]any); ok {
			code, _ = x["code"].(string)
		}
		t.Fatalf("player operation %s failed: %s; private body withheld", action, code)
	}
	data, ok := envelope["data"].(map[string]any)
	if !ok {
		t.Fatal("public player data missing")
	}
	return data
}
func (n *playerFixture) denied(t *testing.T, a actor, action, w, r string, fields map[string]any) string {
	t.Helper()
	envelope, status := n.wire(t, a, action, w, r, fmt.Sprintf("player-denied-request-%08d", sequence.Add(1)), fields)
	if status == http.StatusOK {
		t.Fatal("unauthorized player operation accepted")
	}
	e, ok := envelope["error"].(map[string]any)
	if !ok {
		t.Fatal("safe player error missing")
	}
	code, _ := e["code"].(string)
	return code
}
func (n *playerFixture) configureAndLaunch(t *testing.T) {
	t.Helper()
	n.api(t, n.owner, "configure", map[string]any{"configuration_id": "minimal", "slots": n.humanSlots()})
	n.launchConfigured(t)
}
func (n *playerFixture) humanSlots() []map[string]any {
	return []map[string]any{{"id": "gm", "mode": "human", "participant_id": n.hostPart}, {"id": "player", "mode": "human", "participant_id": n.playerPart}}
}
func (n *playerFixture) launchConfigured(t *testing.T) {
	t.Helper()
	for _, a := range []actor{n.owner, n.participant} {
		n.api(t, a, "consent", map[string]any{"revision": "1", "consent": true, "ready": true, "safety_confirmed": true, "boundaries": []string{}})
	}
	if !n.api(t, n.owner, "readiness", nil)["readiness"].(map[string]any)["ready"].(bool) {
		t.Fatal("fully confirmed lobby was not ready")
	}
	n.sessionID = n.api(t, n.owner, "launch", map[string]any{"revision": "1"})["session_id"].(string)
	n.hostConnection = n.api(t, n.owner, "connect", map[string]any{"after_cursor": "0"})["connection_id"].(string)
	n.playerConnection = n.api(t, n.participant, "connect", map[string]any{"after_cursor": "0"})["connection_id"].(string)
	ctl := n.api(t, n.owner, "snapshot", map[string]any{"connection_id": n.hostConnection, "after_cursor": "0", "limit": 8})["control"].(map[string]any)
	ctl = n.api(t, n.owner, "resume", map[string]any{"expected_control_revision": ctl["revision"]})
	if !ctl["paused"].(bool) {
		t.Fatal("host confirmed another participant")
	}
	ctl = n.api(t, n.participant, "resume", map[string]any{"expected_control_revision": ctl["revision"]})
	if ctl["paused"].(bool) {
		t.Fatal("unanimous current resume rejected")
	}
}
func (n *playerFixture) commandFields(connection, id, version string) map[string]any {
	return map[string]any{"connection_id": connection, "command_id": id, "expected_state_version": version, "type": "increment", "payload": checkpoint.Object(map[string]checkpoint.Value{"delta": checkpoint.Int(1)}), "correlation_id": "owned-correlation"}
}
func (n *playerFixture) snapshot(t *testing.T, a actor, connection, after string, limit int) map[string]any {
	t.Helper()
	return n.api(t, a, "snapshot", map[string]any{"connection_id": connection, "after_cursor": after, "limit": limit})
}
func private(t *testing.T, value any, expected bool) {
	t.Helper()
	raw, e := json.Marshal(value)
	need(t, e)
	defer clear(raw)
	if bytes.Contains(raw, []byte(PrivateValue)) != expected {
		t.Fatal("seat privacy mismatch; output withheld")
	}
}
func (n *playerFixture) direct(t *testing.T, a actor, action, key string, fields map[string]any) (auth.Outcome, error) {
	t.Helper()
	raw, e := json.Marshal(fields)
	need(t, e)
	defer clear(raw)
	request, e := n.players.Decode(action, n.w, n.room, raw)
	need(t, e)
	return n.players.Do(n.ctx, n.caller(a, key), request)
}
func (n *playerFixture) restart(t *testing.T) {
	t.Helper()
	n.players.Close()
	need(t, n.service.Close())
	n.recomposeLaunch(t, n.configs)
	n.composePlayer(t)
}
