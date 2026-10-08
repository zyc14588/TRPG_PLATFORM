//go:build linux && integration

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package model_gateway_test

import (
	"bytes"
	"context"
	"crypto/rand"
	"crypto/sha256"
	"encoding/base64"
	"encoding/json"
	"fmt"
	"net/url"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"strings"
	"sync/atomic"
	"testing"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/room"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/postgres"
)

var loopDsn, loopPrefix, loopRunner, loopRunnerHash string
var loopAuthSchema, loopRoomSchema []byte
var loopSequence atomic.Uint64
var loopCookieKey = bytes.Repeat([]byte{11}, 32)
var loopReplayKey = bytes.Repeat([]byte{22}, 32)
var loopInvitationKey = bytes.Repeat([]byte{33}, 32)

func loopOwnedDatabase(u *url.URL) bool {
	id := os.Getenv("M2B009_CONTAINER_ID")
	if len(id) != 64 || strings.Trim(id, "0123456789abcdef") != "" || u.User == nil || u.User.Username() != "m2b009" {
		return false
	}
	port, e := strconv.Atoi(u.Port())
	if e != nil || port < 1 || port > 65535 {
		return false
	}
	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()
	raw, e := exec.CommandContext(ctx, "docker", "inspect", id).Output()
	if e != nil {
		return false
	}
	defer clear(raw)
	var containers []struct {
		ID              string
		Image           string
		State           struct{ Running bool }
		Config          struct{ Labels map[string]string }
		NetworkSettings struct {
			Ports map[string][]struct {
				HostIP   string
				HostPort string
			}
		}
	}
	if json.Unmarshal(raw, &containers) != nil || len(containers) != 1 {
		return false
	}
	c := containers[0]
	labels := c.Config.Labels
	ports := c.NetworkSettings.Ports["5432/tcp"]
	return c.ID == id && c.Image == "sha256:3a82e1f56c8f0f5616a11103ac3d47e632c3938698946a7ad26da0df1334744a" && c.State.Running && labels["codex.task"] == "01a10c5c-1d16-7a91-8c2f-1909e2af4f43" && labels["codex.batch"] == "M2-B009" && labels["codex.run"] == os.Getenv("M2B009_RUN_ID") && len(ports) == 1 && ports[0].HostIP == "127.0.0.1" && ports[0].HostPort == u.Port()
}
func loopNeed(t *testing.T, e error) {
	t.Helper()
	if e != nil {
		t.Fatalf("bounded operation failed: %v", auth.SafeError(e))
	}
}
func loopWant(t *testing.T, e, expected error) {
	t.Helper()
	if auth.SafeError(e) != expected {
		t.Fatalf("expected %v, received %v", expected, auth.SafeError(e))
	}
}
func loopToken(t *testing.T) string {
	t.Helper()
	var b [32]byte
	_, e := rand.Read(b[:])
	loopNeed(t, e)
	return base64.RawURLEncoding.EncodeToString(b[:])
}
func loopHashed(v string) string {
	h := sha256.Sum256([]byte(v))
	return base64.RawURLEncoding.EncodeToString(h[:])
}
func loopPublic(t *testing.T, out auth.Outcome) map[string]any {
	t.Helper()
	var envelope map[string]any
	if json.Unmarshal(out.StorageValue().Body, &envelope) != nil {
		t.Fatal("loopPublic envelope invalid")
	}
	data, ok := envelope["data"].(map[string]any)
	if !ok {
		t.Fatal("loopPublic data missing")
	}
	return data
}

type loopActorData struct {
	ID, CSRF, Network string
	Cookie            auth.BrowserCredential
}
type loopActor = auth.Secret[loopActorData]
type loopFixture struct {
	ctx       context.Context
	r         *postgres.PlatformAuthRepository
	store     *postgres.PlatformRoomStorage
	authority *auth.RoomAuthority
	rooms     *room.Service
	w         string
	owner     loopActor
}

func loopNewFixture(t *testing.T, fault func(context.Context, string) error) *loopFixture {
	t.Helper()
	ctx, cancel := context.WithTimeout(context.Background(), 2*time.Minute)
	t.Cleanup(cancel)
	r, e := postgres.OpenPlatformAuthRepository(ctx, loopDsn, fault)
	loopNeed(t, e)
	t.Cleanup(func() { loopNeed(t, r.Close()) })
	store, e := postgres.NewPlatformRoomStorage(r)
	loopNeed(t, e)
	v, e := room.NewAdmissionVerifier(store, loopInvitationKey)
	loopNeed(t, e)
	a, e := auth.NewRoomAuthority(r, loopCookieKey, loopReplayKey, loopInvitationKey, loopAuthSchema, v, v)
	loopNeed(t, e)
	s, e := room.NewService(a, store, loopInvitationKey, loopRoomSchema)
	loopNeed(t, e)
	f := &loopFixture{ctx: ctx, r: r, store: store, authority: a, rooms: s, w: fmt.Sprintf("w_%s_%d", loopPrefix, loopSequence.Add(1))}
	f.owner = f.account(t)
	loopNeed(t, r.Transact(ctx, func(tx auth.Transaction) error {
		if e := tx.Core().InsertWorkspace(ctx, core.Workspace{ID: f.w, Name: "Owned workspace", OwnerID: f.owner.StorageValue().ID}); e != nil {
			return e
		}
		return tx.Core().PutMembership(ctx, core.Membership{WorkspaceID: f.w, AccountID: f.owner.StorageValue().ID, Role: core.Owner})
	}))
	return f
}
func (f *loopFixture) account(t *testing.T) loopActor {
	t.Helper()
	id := fmt.Sprintf("a_%s_%d", loopPrefix, loopSequence.Add(1))
	raw := loopToken(t)
	now := time.Now().UTC().Truncate(time.Microsecond)
	loopNeed(t, f.r.Transact(f.ctx, func(tx auth.Transaction) error {
		if e := tx.Core().InsertAccount(f.ctx, core.Account{ID: id, DisplayName: "Server nickname"}); e != nil {
			return e
		}
		return tx.PutSession(f.ctx, auth.StoredSession(auth.SessionData{Hash: loopHashed(raw), Kind: "account", AccountID: id, LastSeen: now, ExpiresAt: now.Add(8 * time.Hour)}))
	}))
	out, e := f.authority.Authentication().Context(f.ctx, auth.BrowserCookie(raw), id)
	loopNeed(t, e)
	csrf, _ := loopPublic(t, out)["csrf_token"].(string)
	if csrf == "" {
		t.Fatal("account CSRF missing")
	}
	return auth.RoomSecret(loopActorData{ID: id, CSRF: csrf, Cookie: auth.BrowserCookie(raw)})
}
func (f *loopFixture) anonymous(t *testing.T) loopActor {
	t.Helper()
	network := fmt.Sprintf("network_%d", loopSequence.Add(1))
	out, e := f.authority.Authentication().Context(f.ctx, auth.BrowserCredential{}, network)
	loopNeed(t, e)
	csrf, _ := loopPublic(t, out)["csrf_token"].(string)
	return auth.RoomSecret(loopActorData{Cookie: out.StorageValue().Cookie, CSRF: csrf, Network: network})
}
func (f *loopFixture) request(t *testing.T, action, w, r, id string, fields map[string]any) room.Request {
	t.Helper()
	var b []byte
	if fields != nil {
		fields["schema_version"] = 1
		var e error
		b, e = json.Marshal(fields)
		loopNeed(t, e)
		defer clear(b)
	}
	q, e := f.rooms.Decode(action, w, r, id, b)
	loopNeed(t, e)
	return q
}
func (f *loopFixture) do(t *testing.T, a loopActor, key string, q room.Request) (auth.Outcome, error) {
	t.Helper()
	d := a.StorageValue()
	network := d.Network
	if network == "" {
		network = "owned-network"
	}
	return f.rooms.Do(f.ctx, d.Cookie, d.CSRF, key, network, q)
}
func (f *loopFixture) call(t *testing.T, a loopActor, action, w, r, id string, fields map[string]any) map[string]any {
	t.Helper()
	out, e := f.do(t, a, fmt.Sprintf("owned-request-%06d", loopSequence.Add(1)), f.request(t, action, w, r, id, fields))
	loopNeed(t, e)
	return loopPublic(t, out)
}
func (f *loopFixture) create(t *testing.T) string {
	t.Helper()
	return f.call(t, f.owner, "create_room", f.w, "", "", map[string]any{"name": "Private lobby", "game_id": "game"})["room_id"].(string)
}
func (f *loopFixture) invite(t *testing.T, r string, approval bool, uses int) map[string]any {
	t.Helper()
	return f.call(t, f.owner, "create_invite", f.w, r, "", map[string]any{"approval_required": approval, "expires_in_seconds": 3600, "max_uses": uses})
}
func (f *loopFixture) join(t *testing.T, a loopActor, i map[string]any) map[string]any {
	t.Helper()
	return f.call(t, a, "request_admission", "", "", "", map[string]any{"mode": "account", "invite_token": i["invite_token"]})
}
func (f *loopFixture) guest(t *testing.T, r string, i map[string]any) (loopActor, loopActor, string) {
	t.Helper()
	anon := f.anonymous(t)
	adm := f.call(t, anon, "request_admission", "", "", "", map[string]any{"mode": "guest", "invite_token": i["invite_token"], "display_name": "Guest nickname"})
	id := adm["admission_id"].(string)
	if adm["status"] == "pending" {
		f.call(t, f.owner, "decide_admission", f.w, r, id, map[string]any{"decision": "approve"})
	}
	minted := f.call(t, anon, "guest_token", "", "", id, map[string]any{})
	raw := minted["admission_token"].(string)
	fields := map[string]any{"schema_version": 1, "admission_token": raw}
	b, _ := json.Marshal(fields)
	defer clear(b)
	q, e := f.authority.Authentication().Decode("exchange", "", "", b)
	loopNeed(t, e)
	out, e := f.authority.Authentication().Mutate(f.ctx, anon.StorageValue().Cookie, anon.StorageValue().CSRF, fmt.Sprintf("guest-exchange-%06d", loopSequence.Add(1)), "owned-network", q)
	loopNeed(t, e)
	csrf, _ := loopPublic(t, out)["csrf_token"].(string)
	return auth.RoomSecret(loopActorData{Cookie: out.StorageValue().Cookie, CSRF: csrf}), anon, id
}
func (f *loopFixture) prepareGuest(t *testing.T, r string, i map[string]any) (loopActor, string, auth.BrowserCredential) {
	t.Helper()
	anon := f.anonymous(t)
	a := f.call(t, anon, "request_admission", "", "", "", map[string]any{"mode": "guest", "invite_token": i["invite_token"], "display_name": "Guest nickname"})
	id := a["admission_id"].(string)
	if a["status"] == "pending" {
		f.call(t, f.owner, "decide_admission", f.w, r, id, map[string]any{"decision": "approve"})
	}
	minted := f.call(t, anon, "guest_token", "", "", id, map[string]any{})
	return anon, id, auth.BrowserCookie(minted["admission_token"].(string))
}
func (f *loopFixture) authRequest(t *testing.T, action string, fields map[string]any) auth.Request {
	t.Helper()
	fields["schema_version"] = 1
	b, e := json.Marshal(fields)
	loopNeed(t, e)
	defer clear(b)
	q, e := f.authority.Authentication().Decode(action, "", "", b)
	loopNeed(t, e)
	return q
}
func (f *loopFixture) exchange(t *testing.T, a loopActor, loopToken auth.BrowserCredential, key string) (auth.Outcome, error) {
	t.Helper()
	return f.authority.Authentication().Mutate(f.ctx, a.StorageValue().Cookie, a.StorageValue().CSRF, key, "owned-network", f.authRequest(t, "exchange", map[string]any{"admission_token": loopToken.StorageValue()}))
}
func (f *loopFixture) recompose(t *testing.T) {
	t.Helper()
	v, e := room.NewAdmissionVerifier(f.store, loopInvitationKey)
	loopNeed(t, e)
	a, e := auth.NewRoomAuthority(f.r, loopCookieKey, loopReplayKey, loopInvitationKey, loopAuthSchema, v, v)
	loopNeed(t, e)
	s, e := room.NewService(a, f.store, loopInvitationKey, loopRoomSchema)
	loopNeed(t, e)
	f.authority = a
	f.rooms = s
}
func (f *loopFixture) register(t *testing.T, name string) (loopActor, string) {
	t.Helper()
	login := fmt.Sprintf("u_%s_%d", loopPrefix, loopSequence.Add(1))
	grant := loopToken(t)
	p := filepath.Join(t.TempDir(), "grants")
	b, e := json.Marshal([]map[string]any{{"loopToken": grant, "expires_at": time.Now().UTC().Add(time.Hour).Format(time.RFC3339Nano)}})
	loopNeed(t, e)
	loopNeed(t, os.WriteFile(p, b, 0400))
	clear(b)
	loopNeed(t, f.authority.Authentication().BootstrapFromFiles(f.ctx, "", p))
	a := f.anonymous(t)
	q := f.authRequest(t, "register", map[string]any{"login_name": login, "password": "synthetic-claim-password", "display_name": name, "registration_token": grant})
	out, e := f.authority.Authentication().Mutate(f.ctx, a.StorageValue().Cookie, a.StorageValue().CSRF, "owned-register-key", "owned-network", q)
	loopNeed(t, e)
	d := loopPublic(t, out)
	return auth.RoomSecret(loopActorData{ID: d["principal"].(map[string]any)["account_id"].(string), Cookie: out.StorageValue().Cookie, CSRF: d["csrf_token"].(string)}), login
}
func (f *loopFixture) sql(t *testing.T, statement string) string {
	t.Helper()
	id := os.Getenv("M2B009_CONTAINER_ID")
	if len(id) != 64 || strings.Trim(id, "0123456789abcdef") != "" {
		t.Fatal("owned container identity missing")
	}
	raw, e := exec.CommandContext(f.ctx, "docker", "inspect", "--format", "{{json .Config.Labels}}", id).Output()
	if e != nil {
		t.Fatal("owned labels unavailable")
	}
	var labels map[string]string
	if json.Unmarshal(raw, &labels) != nil || labels["codex.batch"] != "M2-B009" || labels["codex.task"] != "01a10c5c-1d16-7a91-8c2f-1909e2af4f43" || labels["codex.run"] != os.Getenv("M2B009_RUN_ID") {
		t.Fatal("owned labels mismatch")
	}
	cmd := exec.CommandContext(f.ctx, "docker", "exec", id, "psql", "-U", "m2b009", "-d", "m2_b009_fixture", "-v", "ON_ERROR_STOP=1", "-Atq", "-c", statement)
	b, e := cmd.Output()
	if e != nil {
		t.Fatal("owned SQL loopFixture operation failed")
	}
	return strings.TrimSpace(string(b))
}
