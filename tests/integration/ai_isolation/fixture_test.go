//go:build linux && integration

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package ai_isolation_test

import (
	"bytes"
	"context"
	"crypto/rand"
	"crypto/sha256"
	"encoding/base64"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/credential"
	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/model"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/launch"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/room"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/postgres"
	"net/url"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"strings"
	"sync/atomic"
	"testing"
	"time"
)

var dsn, prefix string
var authSchema []byte
var sequence atomic.Uint64
var cookieKey = bytes.Repeat([]byte{11}, 32)
var replayKey = bytes.Repeat([]byte{22}, 32)
var inviteKey = bytes.Repeat([]byte{33}, 32)

func TestMain(m *testing.M) {
	dsn = os.Getenv("M2B008_POSTGRES_DSN")
	u, e := url.Parse(dsn)
	if e != nil || u.Scheme != "postgres" || u.Hostname() != "127.0.0.1" || u.Path != "/m2_b008_fixture" || os.Getenv("M2B008_OWNED_DATABASE") != "1" || !ownedDatabase(u) {
		fmt.Fprintln(os.Stderr, "M2-B008 owned physical PostgreSQL required; integration NOT_RUN")
		os.Exit(1)
	}
	var nonce [6]byte
	if _, e = rand.Read(nonce[:]); e != nil {
		fmt.Fprintln(os.Stderr, "M2-B008 isolation unavailable; integration NOT_RUN")
		os.Exit(1)
	}
	prefix = hex.EncodeToString(nonce[:])
	authSchema, e = os.ReadFile("../../../schemas/platform/platform-auth-api-v1.schema.json")
	if e != nil {
		fmt.Fprintln(os.Stderr, "M2-B008 approved auth Schema absent; integration NOT_RUN")
		os.Exit(1)
	}
	ctx, cancel := context.WithTimeout(context.Background(), 20*time.Second)
	r, e := postgres.OpenPlatformAuthRepository(ctx, dsn, nil)
	if e == nil {
		e = r.Bootstrap(ctx)
	}
	if e == nil {
		var s *postgres.PlatformRoomStorage
		s, e = postgres.NewPlatformRoomStorage(r)
		if e == nil {
			e = s.Bootstrap(ctx)
		}
	}
	if e == nil {
		var s *postgres.PlatformLaunchStorage
		s, e = postgres.NewPlatformLaunchStorage(r)
		if e == nil {
			e = s.Bootstrap(ctx)
		}
	}
	if e == nil {
		var s *postgres.PlatformModelStorage
		s, e = postgres.NewPlatformModelStorage(r)
		if e == nil {
			e = s.Bootstrap(ctx)
		}
	}
	if e == nil {
		var s *postgres.PlatformBudgetStorage
		s, e = postgres.NewPlatformBudgetStorage(r)
		if e == nil {
			e = s.Bootstrap(ctx)
		}
	}
	if r != nil {
		_ = r.Close()
	}
	cancel()
	if e != nil {
		fmt.Fprintln(os.Stderr, "M2-B008 owned bootstrap unavailable; integration NOT_RUN")
		os.Exit(1)
	}
	os.Exit(m.Run())
}
func ownedDatabase(u *url.URL) bool {
	id := os.Getenv("M2B008_CONTAINER_ID")
	if len(id) != 64 || strings.Trim(id, "0123456789abcdef") != "" || u.User == nil || u.User.Username() != "m2b008" || os.Getenv("M2B008_RUN_ID") == "" {
		return false
	}
	p, e := strconv.Atoi(u.Port())
	if e != nil || p < 1 || p > 65535 {
		return false
	}
	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()
	raw, e := exec.CommandContext(ctx, "docker", "inspect", id).Output()
	if e != nil {
		return false
	}
	defer clear(raw)
	var cs []struct {
		ID              string
		Image           string
		State           struct{ Running bool }
		Config          struct{ Labels map[string]string }
		NetworkSettings struct {
			Ports map[string][]struct{ HostIP, HostPort string }
		}
	}
	if json.Unmarshal(raw, &cs) != nil || len(cs) != 1 {
		return false
	}
	c := cs[0]
	ps := c.NetworkSettings.Ports["5432/tcp"]
	return c.ID == id && c.Image == "sha256:3a82e1f56c8f0f5616a11103ac3d47e632c3938698946a7ad26da0df1334744a" && c.State.Running && len(c.Config.Labels) == 3 && c.Config.Labels["codex.task"] == "01a10c5c-1d16-7a91-8c2f-1909e2af4f43" && c.Config.Labels["codex.batch"] == "M2-B008" && c.Config.Labels["codex.run"] == os.Getenv("M2B008_RUN_ID") && len(ps) == 1 && ps[0].HostIP == "127.0.0.1" && ps[0].HostPort == u.Port()
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
		t.Fatalf("expected %v received %v", expected, auth.SafeError(e))
	}
}
func token(t *testing.T) string {
	t.Helper()
	var b [32]byte
	_, e := rand.Read(b[:])
	need(t, e)
	return base64.RawURLEncoding.EncodeToString(b[:])
}
func hashed(s string) string {
	h := sha256.Sum256([]byte(s))
	return base64.RawURLEncoding.EncodeToString(h[:])
}
func evidence(s string) string   { h := sha256.Sum256([]byte(s)); return hex.EncodeToString(h[:]) }
func unique(label string) string { return fmt.Sprintf("%s_%s_%d", label, prefix, sequence.Add(1)) }

type actorData struct {
	ID, CSRF  string
	Cookie    auth.BrowserCredential
	Kind      string
	Scope     core.Scope
	ExpiresAt time.Time
}
type actor = auth.Secret[actorData]
type fixture struct {
	ctx        context.Context
	r          *postgres.PlatformAuthRepository
	rt         *postgres.PlatformRoomStorage
	lt         *postgres.PlatformLaunchStorage
	mt         *postgres.PlatformModelStorage
	authority  *auth.RoomAuthority
	models     *model.Service
	vault      *credential.Vault
	scope      core.Scope
	owner      actor
	prep       launch.PreparationData
	ack        launch.AcknowledgmentData
	endpoints  []model.Endpoint
	certs      []model.Certification
	masterFile string
}

func (f *fixture) account(t *testing.T) actor {
	t.Helper()
	id := unique("a")
	raw := token(t)
	now := time.Now().UTC().Truncate(time.Microsecond)
	expires := now.Add(8 * time.Hour)
	need(t, f.r.Transact(f.ctx, func(tx auth.Transaction) error {
		if e := tx.Core().InsertAccount(f.ctx, core.Account{ID: id, DisplayName: "Fixture account"}); e != nil {
			return e
		}
		return tx.PutSession(f.ctx, auth.StoredSession(auth.SessionData{Hash: hashed(raw), Kind: "account", AccountID: id, ExpiresAt: expires, LastSeen: now}))
	}))
	return f.context(t, actorData{ID: id, Cookie: auth.BrowserCookie(raw), Kind: "account", ExpiresAt: expires})
}
func (f *fixture) context(t *testing.T, a actorData) actor {
	t.Helper()
	out, e := f.authority.Authentication().Context(f.ctx, a.Cookie, unique("net"))
	need(t, e)
	var v struct {
		Data struct {
			CSRFToken string `json:"csrf_token"`
		}
	}
	if json.Unmarshal(out.StorageValue().Body, &v) != nil || v.Data.CSRFToken == "" {
		t.Fatal("authenticated context missing")
	}
	a.CSRF = v.Data.CSRFToken
	return auth.RoomSecret(a)
}
func (f *fixture) caller(a actor, key string) model.Caller {
	if key != "" {
		key = "m2-" + key
	}
	x := a.StorageValue()
	return auth.RoomSecret(model.CallerData{Credential: x.Cookie, CSRF: x.CSRF, IdempotencyKey: key, Network: "owned-fixture"})
}
func (f *fixture) inspect(t *testing.T, a actor, fn func(auth.Transaction) error) {
	t.Helper()
	x := a.StorageValue()
	need(t, f.authority.Inspect(f.ctx, x.Cookie, x.CSRF, true, func(_ context.Context, tx auth.Transaction, _ auth.SessionData) error { return fn(tx) }))
}
func newFixture(t *testing.T, fault func(context.Context, string) error) *fixture {
	t.Helper()
	ctx, cancel := context.WithTimeout(context.Background(), 2*time.Minute)
	t.Cleanup(cancel)
	r, e := postgres.OpenPlatformAuthRepository(ctx, dsn, fault)
	need(t, e)
	t.Cleanup(func() { need(t, r.Close()) })
	rt, e := postgres.NewPlatformRoomStorage(r)
	need(t, e)
	lt, e := postgres.NewPlatformLaunchStorage(r)
	need(t, e)
	mt, e := postgres.NewPlatformModelStorage(r)
	need(t, e)
	v, e := room.NewAdmissionVerifier(rt, inviteKey)
	need(t, e)
	authority, e := auth.NewRoomAuthority(r, cookieKey, replayKey, inviteKey, authSchema, v, v)
	need(t, e)
	f := &fixture{ctx: ctx, r: r, rt: rt, lt: lt, mt: mt, authority: authority, scope: core.Scope{WorkspaceID: unique("w"), RoomID: unique("r"), GameID: unique("g")}}
	f.owner = f.account(t)
	id := f.owner.StorageValue().ID
	part := unique("p")
	f.prep = launch.PreparationData{Scope: f.scope, ConfigurationID: "fixture-game", ConfigurationHash: checkpoint.Hash([]byte("synthetic-configuration")), GraphHash: checkpoint.Hash([]byte("synthetic-model-game")), Revision: 1, Slots: []launch.Slot{{ID: "human", Mode: "human", ParticipantID: part}, {ID: "ai", Mode: "ai", ModelSelection: "selected"}}}
	f.ack = launch.AcknowledgmentData{Scope: f.scope, ParticipantID: part, ConfigurationHash: f.prep.ConfigurationHash, GraphHash: f.prep.GraphHash, Revision: 1, Consent: true, Ready: true, SafetyConfirmed: true}
	f.inspect(t, f.owner, func(tx auth.Transaction) error {
		if e := tx.Core().InsertWorkspace(ctx, core.Workspace{ID: f.scope.WorkspaceID, Name: "Owned model fixture", OwnerID: id}); e != nil {
			return e
		}
		if e := tx.Core().PutMembership(ctx, core.Membership{WorkspaceID: f.scope.WorkspaceID, AccountID: id, Role: core.Owner}); e != nil {
			return e
		}
		roomTx, e := rt.Bind(tx.Core())
		if e != nil {
			return e
		}
		if e = roomTx.InsertRoom(ctx, auth.RoomSecret(room.RoomData{Scope: f.scope, ID: f.scope.RoomID, Name: "Private fixture", Owner: id, State: "lobby"})); e != nil {
			return e
		}
		if e = roomTx.PutParticipant(ctx, auth.RoomSecret(room.ParticipantData{Scope: f.scope, ID: part, AccountID: id, Name: "Host", Active: true, Host: true})); e != nil {
			return e
		}
		launchTx, e := lt.Bind(tx.Core())
		if e != nil {
			return e
		}
		if e = launchTx.PutPreparation(ctx, auth.RoomSecret(f.prep)); e != nil {
			return e
		}
		return launchTx.PutAcknowledgment(ctx, auth.RoomSecret(f.ack))
	})
	f.masterFile = filepath.Join(t.TempDir(), "master")
	raw := make([]byte, 32)
	_, e = rand.Read(raw)
	need(t, e)
	need(t, os.WriteFile(f.masterFile, raw, 0400))
	clear(raw)
	f.vault, e = credential.New(f.masterFile)
	need(t, e)
	t.Cleanup(f.vault.Close)
	f.endpoints = []model.Endpoint{model.NewEndpoint(model.EndpointData{ID: "local-approved", URL: "http://127.0.0.1:8081/v1", Adapter: "openai-compatible", Models: []string{"fixture:small", "fixture:other"}, AllowLANHTTP: true})}
	// Synthetic, reviewed test records prove registry binding, not a real
	// provider qualification. Actual provider qualification and the mixed game loop belong to B012; these fixtures exercise real SQL isolation and budgeting.
	c := model.CertificationData{ID: "default-model", WorkspaceID: f.scope.WorkspaceID, Tuple: model.Tuple{Model: "fixture:small", Endpoint: "http://127.0.0.1:8081/v1", Adapter: "openai-compatible", PromptTemplate: "safe-v1", ToolMode: "structured", TestVersion: "fixture-v1"}, Level: 3, Capabilities: []string{"structured-actions", "ai-player"}, Games: []model.GameEvidence{{GraphHash: f.prep.GraphHash, TestVersion: "fixture-v1", EvidenceHash: evidence("synthetic-game-fixture-v1"), Capabilities: []string{"structured-actions", "ai-player"}}}, EvidenceHash: evidence("synthetic-model-fixture-v1"), ExpiresAt: time.Now().UTC().Add(time.Hour)}
	other := c
	other.ID = "approved-fallback"
	other.Tuple.Model = "fixture:other"
	f.certs = []model.Certification{model.NewCertification(c), model.NewCertification(other)}
	f.models = f.service(t, f.endpoints, f.certs)
	return f
}
func limits() model.Limits {
	return model.Limits{Calls: 4, Tokens: 4096, CostMicros: 10000, LatencyMillis: 1000, Tools: 4, Subagents: 1, ContextBytes: 4096, LocalComputeMillis: 1000}
}
func (f *fixture) service(t *testing.T, es []model.Endpoint, cs []model.Certification) *model.Service {
	t.Helper()
	s, e := model.New(model.Options{Authority: f.authority, Rooms: f.rt, Launches: f.lt, Storage: f.mt, Vault: f.vault, Endpoints: es, Certifications: cs, Defaults: map[string]string{f.scope.WorkspaceID: "default-model"}, WorkspaceLimits: map[string]model.Limits{f.scope.WorkspaceID: limits()}})
	need(t, e)
	return s
}
func (f *fixture) store(t *testing.T, a actor, seat, id string, lifetime credential.Lifetime, expiry time.Time) error {
	t.Helper()
	key, e := credential.NewKey([]byte("owned-fixture-private-provider-key-329874"))
	need(t, e)
	defer key.Close()
	return f.models.StoreCredential(f.ctx, f.caller(a, token(t)), auth.RoomSecret(model.CredentialRequestData{Scope: f.scope, SeatID: seat, ID: id, Lifetime: lifetime, ExpiresAt: expiry, Key: key}))
}
func (f *fixture) configured(t *testing.T) (model.Configuration, model.ConfigureRequest) {
	t.Helper()
	need(t, f.store(t, f.owner, "ai", "byok", credential.Retained, time.Time{}))
	r := model.NewConfigureRequest(model.ConfigureRequestData{Scope: f.scope, SeatID: "ai", Selection: "selected", CredentialID: "byok", Budget: limits()})
	v, e := f.models.Configure(f.ctx, f.caller(f.owner, token(t)), r)
	need(t, e)
	return v, r
}
func (f *fixture) proof() launch.ModelRequirement {
	return auth.RoomSecret(launch.ModelRequirementData{Scope: f.scope, ConfigurationID: f.prep.ConfigurationID, ConfigurationHash: f.prep.ConfigurationHash, GraphHash: f.prep.GraphHash, SeatID: "ai", Selection: "selected", Revision: f.prep.Revision, Capabilities: []string{"structured-actions", "ai-player"}})
}
func (f *fixture) check(t *testing.T, s *model.Service, proof launch.ModelRequirement) error {
	t.Helper()
	x := f.owner.StorageValue()
	return f.authority.Inspect(f.ctx, x.Cookie, x.CSRF, false, func(ctx context.Context, tx auth.Transaction, _ auth.SessionData) error {
		return s.Check(ctx, tx.Core(), proof, []launch.Acknowledgment{auth.RoomSecret(f.ack)})
	})
}
func sqlCapture(t *testing.T, query string) []byte {
	t.Helper()
	ctx, cancel := context.WithTimeout(context.Background(), 8*time.Second)
	defer cancel()
	raw, e := exec.CommandContext(ctx, "docker", "exec", os.Getenv("M2B008_CONTAINER_ID"), "psql", "-U", "m2b008", "-d", "m2_b008_fixture", "-v", "ON_ERROR_STOP=1", "-At", "-c", query).Output()
	if e != nil {
		t.Fatal("bounded owned SQL operation failed")
	}
	return raw
}
