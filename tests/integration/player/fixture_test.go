//go:build linux && integration

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package player_test

import (
	"bytes"
	"context"
	"crypto/rand"
	"crypto/sha256"
	"encoding/base64"
	"encoding/hex"
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

	hostfixture "github.com/zyc14588/TRPG_PLATFORM/internal/hostapi/hostapitest"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/room"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/postgres"
)

var dsn, prefix, runner, runnerHash string
var authSchema, roomSchema []byte
var sequence atomic.Uint64
var cookieKey = bytes.Repeat([]byte{11}, 32)
var replayKey = bytes.Repeat([]byte{22}, 32)
var invitationKey = bytes.Repeat([]byte{33}, 32)

func TestMain(m *testing.M) {
	dsn = os.Getenv("M2B013_POSTGRES_DSN")
	u, e := url.Parse(dsn)
	if e != nil || u.Scheme != "postgres" || u.Hostname() != "127.0.0.1" || u.Path != "/m2_b013_fixture" || os.Getenv("M2B013_OWNED_DATABASE") != "1" || os.Getenv("M2B013_RUN_ID") == "" {
		fmt.Fprintln(os.Stderr, "M2-B013 owned local database required; integration NOT_RUN")
		os.Exit(1)
	}
	if !ownedDatabase(u) {
		fmt.Fprintln(os.Stderr, "M2-B013 physical database ownership not verified; integration NOT_RUN")
		os.Exit(1)
	}
	var nonce [6]byte
	if _, e = rand.Read(nonce[:]); e != nil {
		fmt.Fprintln(os.Stderr, "M2-B013 isolation unavailable; integration NOT_RUN")
		os.Exit(1)
	}
	prefix = hex.EncodeToString(nonce[:])
	authSchema, e = os.ReadFile("../../../schemas/platform/platform-auth-api-v1.schema.json")
	if e != nil {
		fmt.Fprintln(os.Stderr, "M2-B013 auth Schema absent; integration NOT_RUN")
		os.Exit(1)
	}
	roomSchema, e = os.ReadFile("../../../schemas/platform/platform-room-api-v1.schema.json")
	if e != nil {
		fmt.Fprintln(os.Stderr, "M2-B013 room Schema absent; integration NOT_RUN")
		os.Exit(1)
	}
	ctx, cancel := context.WithTimeout(context.Background(), 20*time.Second)
	repo, e := postgres.OpenPlatformAuthRepository(ctx, dsn, nil)
	if e == nil {
		e = repo.Bootstrap(ctx)
	}
	if e == nil {
		var store *postgres.PlatformRoomStorage
		store, e = postgres.NewPlatformRoomStorage(repo)
		if e == nil {
			e = store.Bootstrap(ctx)
		}
	}
	if repo != nil {
		_ = repo.Close()
	}
	cancel()
	if e != nil {
		fmt.Fprintln(os.Stderr, "M2-B013 bootstrap unavailable; integration NOT_RUN")
		os.Exit(1)
	}

	runnerDir, e := os.MkdirTemp("", "m2-b013-owned-runner-")
	if e != nil {
		fmt.Fprintln(os.Stderr, "M2-B013 runner directory unavailable; integration NOT_RUN")
		os.Exit(1)
	}
	root, e := filepath.Abs("../../..")
	if e == nil {
		runner, e = hostfixture.BuildRunner(root, runnerDir)
	}
	if e != nil {
		_ = os.RemoveAll(runnerDir)
		fmt.Fprintln(os.Stderr, "M2-B013 production runner unavailable; integration NOT_RUN")
		os.Exit(1)
	}
	raw, e := os.ReadFile(runner)
	if e != nil {
		_ = os.RemoveAll(runnerDir)
		fmt.Fprintln(os.Stderr, "M2-B013 production runner unreadable; integration NOT_RUN")
		os.Exit(1)
	}
	runnerHash = checkpoint.Hash(raw)
	clear(raw)
	code := m.Run()
	_ = os.RemoveAll(runnerDir)
	os.Exit(code)
}
func ownedDatabase(u *url.URL) bool {
	id := os.Getenv("M2B013_CONTAINER_ID")
	if len(id) != 64 || strings.Trim(id, "0123456789abcdef") != "" || u.User == nil || u.User.Username() != "m2b013" {
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
	return c.ID == id && c.State.Running && labels["codex.task"] == "01a10c5c-1d16-7a91-8c2f-1909e2af4f43" && labels["codex.batch"] == "M2-B013" && labels["codex.run"] == os.Getenv("M2B013_RUN_ID") && len(ports) == 1 && ports[0].HostIP == "127.0.0.1" && ports[0].HostPort == u.Port()
}
func need(t *testing.T, e error) {
	t.Helper()
	if e != nil {
		t.Fatalf("bounded operation failed: %v", auth.SafeError(e))
	}
}
func want(t *testing.T, e, expected error) {
	t.Helper()
	if auth.SafeError(e) != expected {
		t.Fatalf("expected %v, received %v", expected, auth.SafeError(e))
	}
}
func token(t *testing.T) string {
	t.Helper()
	var b [32]byte
	_, e := rand.Read(b[:])
	need(t, e)
	return base64.RawURLEncoding.EncodeToString(b[:])
}
func hashed(v string) string {
	h := sha256.Sum256([]byte(v))
	return base64.RawURLEncoding.EncodeToString(h[:])
}
func public(t *testing.T, out auth.Outcome) map[string]any {
	t.Helper()
	var envelope map[string]any
	if json.Unmarshal(out.StorageValue().Body, &envelope) != nil {
		t.Fatal("public envelope invalid")
	}
	data, ok := envelope["data"].(map[string]any)
	if !ok {
		t.Fatal("public data missing")
	}
	return data
}

type actorData struct {
	ID, CSRF, Network string
	Cookie            auth.BrowserCredential
}
type actor = auth.Secret[actorData]
type fixture struct {
	ctx       context.Context
	r         *postgres.PlatformAuthRepository
	store     *postgres.PlatformRoomStorage
	authority *auth.RoomAuthority
	rooms     *room.Service
	w         string
	owner     actor
}

func newFixture(t *testing.T, fault func(context.Context, string) error) *fixture {
	t.Helper()
	ctx, cancel := context.WithTimeout(context.Background(), 2*time.Minute)
	t.Cleanup(cancel)
	r, e := postgres.OpenPlatformAuthRepository(ctx, dsn, fault)
	need(t, e)
	t.Cleanup(func() { need(t, r.Close()) })
	store, e := postgres.NewPlatformRoomStorage(r)
	need(t, e)
	v, e := room.NewAdmissionVerifier(store, invitationKey)
	need(t, e)
	a, e := auth.NewRoomAuthority(r, cookieKey, replayKey, invitationKey, authSchema, v, v)
	need(t, e)
	s, e := room.NewService(a, store, invitationKey, roomSchema)
	need(t, e)
	f := &fixture{ctx: ctx, r: r, store: store, authority: a, rooms: s, w: fmt.Sprintf("w_%s_%d", prefix, sequence.Add(1))}
	f.owner = f.account(t)
	need(t, r.Transact(ctx, func(tx auth.Transaction) error {
		if e := tx.Core().InsertWorkspace(ctx, core.Workspace{ID: f.w, Name: "Owned workspace", OwnerID: f.owner.StorageValue().ID}); e != nil {
			return e
		}
		return tx.Core().PutMembership(ctx, core.Membership{WorkspaceID: f.w, AccountID: f.owner.StorageValue().ID, Role: core.Owner})
	}))
	return f
}
func (f *fixture) account(t *testing.T) actor {
	t.Helper()
	id := fmt.Sprintf("a_%s_%d", prefix, sequence.Add(1))
	raw := token(t)
	now := time.Now().UTC().Truncate(time.Microsecond)
	need(t, f.r.Transact(f.ctx, func(tx auth.Transaction) error {
		if e := tx.Core().InsertAccount(f.ctx, core.Account{ID: id, DisplayName: "Server nickname"}); e != nil {
			return e
		}
		return tx.PutSession(f.ctx, auth.StoredSession(auth.SessionData{Hash: hashed(raw), Kind: "account", AccountID: id, LastSeen: now, ExpiresAt: now.Add(8 * time.Hour)}))
	}))
	out, e := f.authority.Authentication().Context(f.ctx, auth.BrowserCookie(raw), id)
	need(t, e)
	csrf, _ := public(t, out)["csrf_token"].(string)
	if csrf == "" {
		t.Fatal("account CSRF missing")
	}
	return auth.RoomSecret(actorData{ID: id, CSRF: csrf, Cookie: auth.BrowserCookie(raw)})
}
func (f *fixture) anonymous(t *testing.T) actor {
	t.Helper()
	network := fmt.Sprintf("network_%d", sequence.Add(1))
	out, e := f.authority.Authentication().Context(f.ctx, auth.BrowserCredential{}, network)
	need(t, e)
	csrf, _ := public(t, out)["csrf_token"].(string)
	return auth.RoomSecret(actorData{Cookie: out.StorageValue().Cookie, CSRF: csrf, Network: network})
}
func (f *fixture) request(t *testing.T, action, w, r, id string, fields map[string]any) room.Request {
	t.Helper()
	var b []byte
	if fields != nil {
		fields["schema_version"] = 1
		var e error
		b, e = json.Marshal(fields)
		need(t, e)
		defer clear(b)
	}
	q, e := f.rooms.Decode(action, w, r, id, b)
	need(t, e)
	return q
}
func (f *fixture) do(t *testing.T, a actor, key string, q room.Request) (auth.Outcome, error) {
	t.Helper()
	d := a.StorageValue()
	network := d.Network
	if network == "" {
		network = "owned-network"
	}
	return f.rooms.Do(f.ctx, d.Cookie, d.CSRF, key, network, q)
}
func (f *fixture) call(t *testing.T, a actor, action, w, r, id string, fields map[string]any) map[string]any {
	t.Helper()
	out, e := f.do(t, a, fmt.Sprintf("owned-request-%06d", sequence.Add(1)), f.request(t, action, w, r, id, fields))
	need(t, e)
	return public(t, out)
}
func (f *fixture) create(t *testing.T) string {
	t.Helper()
	return f.call(t, f.owner, "create_room", f.w, "", "", map[string]any{"name": "Private lobby", "game_id": "game"})["room_id"].(string)
}
func (f *fixture) invite(t *testing.T, r string, approval bool, uses int) map[string]any {
	t.Helper()
	return f.call(t, f.owner, "create_invite", f.w, r, "", map[string]any{"approval_required": approval, "expires_in_seconds": 3600, "max_uses": uses})
}
func (f *fixture) join(t *testing.T, a actor, i map[string]any) map[string]any {
	t.Helper()
	return f.call(t, a, "request_admission", "", "", "", map[string]any{"mode": "account", "invite_token": i["invite_token"]})
}
func (f *fixture) guest(t *testing.T, r string, i map[string]any) (actor, actor, string) {
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
	need(t, e)
	out, e := f.authority.Authentication().Mutate(f.ctx, anon.StorageValue().Cookie, anon.StorageValue().CSRF, fmt.Sprintf("guest-exchange-%06d", sequence.Add(1)), "owned-network", q)
	need(t, e)
	csrf, _ := public(t, out)["csrf_token"].(string)
	return auth.RoomSecret(actorData{Cookie: out.StorageValue().Cookie, CSRF: csrf}), anon, id
}
func (f *fixture) prepareGuest(t *testing.T, r string, i map[string]any) (actor, string, auth.BrowserCredential) {
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
func (f *fixture) authRequest(t *testing.T, action string, fields map[string]any) auth.Request {
	t.Helper()
	fields["schema_version"] = 1
	b, e := json.Marshal(fields)
	need(t, e)
	defer clear(b)
	q, e := f.authority.Authentication().Decode(action, "", "", b)
	need(t, e)
	return q
}
func (f *fixture) exchange(t *testing.T, a actor, token auth.BrowserCredential, key string) (auth.Outcome, error) {
	t.Helper()
	return f.authority.Authentication().Mutate(f.ctx, a.StorageValue().Cookie, a.StorageValue().CSRF, key, "owned-network", f.authRequest(t, "exchange", map[string]any{"admission_token": token.StorageValue()}))
}
func (f *fixture) recompose(t *testing.T) {
	t.Helper()
	v, e := room.NewAdmissionVerifier(f.store, invitationKey)
	need(t, e)
	a, e := auth.NewRoomAuthority(f.r, cookieKey, replayKey, invitationKey, authSchema, v, v)
	need(t, e)
	s, e := room.NewService(a, f.store, invitationKey, roomSchema)
	need(t, e)
	f.authority = a
	f.rooms = s
}
func (f *fixture) register(t *testing.T, name string) (actor, string) {
	t.Helper()
	login := fmt.Sprintf("u_%s_%d", prefix, sequence.Add(1))
	grant := token(t)
	p := filepath.Join(t.TempDir(), "grants")
	b, e := json.Marshal([]map[string]any{{"token": grant, "expires_at": time.Now().UTC().Add(time.Hour).Format(time.RFC3339Nano)}})
	need(t, e)
	need(t, os.WriteFile(p, b, 0400))
	clear(b)
	need(t, f.authority.Authentication().BootstrapFromFiles(f.ctx, "", p))
	a := f.anonymous(t)
	q := f.authRequest(t, "register", map[string]any{"login_name": login, "password": "synthetic-claim-password", "display_name": name, "registration_token": grant})
	out, e := f.authority.Authentication().Mutate(f.ctx, a.StorageValue().Cookie, a.StorageValue().CSRF, "owned-register-key", "owned-network", q)
	need(t, e)
	d := public(t, out)
	return auth.RoomSecret(actorData{ID: d["principal"].(map[string]any)["account_id"].(string), Cookie: out.StorageValue().Cookie, CSRF: d["csrf_token"].(string)}), login
}
func (f *fixture) sql(t *testing.T, statement string) string {
	t.Helper()
	id := os.Getenv("M2B013_CONTAINER_ID")
	if len(id) != 64 || strings.Trim(id, "0123456789abcdef") != "" {
		t.Fatal("owned container identity missing")
	}
	raw, e := exec.CommandContext(f.ctx, "docker", "inspect", "--format", "{{json .Config.Labels}}", id).Output()
	if e != nil {
		t.Fatal("owned labels unavailable")
	}
	var labels map[string]string
	if json.Unmarshal(raw, &labels) != nil || labels["codex.batch"] != "M2-B013" || labels["codex.task"] != "01a10c5c-1d16-7a91-8c2f-1909e2af4f43" || labels["codex.run"] != os.Getenv("M2B013_RUN_ID") {
		t.Fatal("owned labels mismatch")
	}
	cmd := exec.CommandContext(f.ctx, "docker", "exec", id, "psql", "-U", "m2b013", "-d", "m2_b013_fixture", "-v", "ON_ERROR_STOP=1", "-Atq", "-c", statement)
	b, e := cmd.Output()
	if e != nil {
		t.Fatal("owned SQL fixture operation failed")
	}
	return strings.TrimSpace(string(b))
}
