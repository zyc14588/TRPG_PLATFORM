//go:build integration

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package platform_auth_test

import (
	"bytes"
	"context"
	"crypto/rand"
	"crypto/sha256"
	"database/sql"
	"encoding/base64"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"net/http/cookiejar"
	"net/http/httptest"
	"net/url"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"sync/atomic"
	"testing"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/httpapi"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/postgres"
)

var dsn, prefix string
var schema []byte
var sequence atomic.Uint64

const password = "SYNTHETIC_PRIVATE_PASSWORD_SENTINEL"

func TestMain(m *testing.M) {
	dsn = os.Getenv("M2B002_POSTGRES_DSN")
	u, e := url.Parse(dsn)
	if e != nil || u.Scheme != "postgres" || u.Hostname() != "127.0.0.1" || u.Path != "/m2_b002_fixture" || os.Getenv("M2B002_OWNED_DATABASE") != "1" || os.Getenv("M2B002_RUN_ID") == "" {
		fmt.Fprintln(os.Stderr, "M2-B002 owned local database required; integration NOT_RUN")
		os.Exit(1)
	}
	var nonce [6]byte
	if _, e := rand.Read(nonce[:]); e != nil {
		fmt.Fprintln(os.Stderr, "M2-B002 isolation unavailable; integration NOT_RUN")
		os.Exit(1)
	}
	prefix = hex.EncodeToString(nonce[:])
	schema, e = os.ReadFile("../../../schemas/platform/platform-auth-api-v1.schema.json")
	if e != nil {
		fmt.Fprintln(os.Stderr, "M2-B002 schema absent; integration NOT_RUN")
		os.Exit(1)
	}
	ctx, cancel := context.WithTimeout(context.Background(), 20*time.Second)
	r, e := postgres.OpenPlatformAuthRepository(ctx, dsn, nil)
	if e == nil {
		e = r.Bootstrap(ctx)
	}
	if e == nil {
		var s *auth.Service
		s, e = newService(r, nil)
		if e == nil {
			var dir string
			dir, e = os.MkdirTemp("", "trpg-m2b002-seed-")
			if e == nil {
				p := filepath.Join(dir, "seed")
				raw, _ := json.Marshal(map[string]any{"login_name": "operator_seed", "password": password, "display_name": "Operator"})
				e = os.WriteFile(p, raw, 0400)
				clear(raw)
				if e == nil {
					e = s.BootstrapFromFiles(ctx, p, "")
				}
				_ = os.RemoveAll(dir)
			}
		}
	}
	if r != nil {
		_ = r.Close()
	}
	cancel()
	if e != nil {
		fmt.Fprintln(os.Stderr, "M2-B002 bootstrap unavailable; integration NOT_RUN")
		os.Exit(1)
	}
	os.Exit(m.Run())
}
func newService(r auth.Repository, v auth.AdmissionVerifier) (*auth.Service, error) {
	return auth.NewService(r, bytes.Repeat([]byte{11}, 32), bytes.Repeat([]byte{22}, 32), schema, v)
}
func need(t *testing.T, e error) {
	t.Helper()
	if e != nil {
		t.Fatalf("bounded operation failed: %v", auth.SafeError(e))
	}
}
func want(t *testing.T, e, expected error) {
	t.Helper()
	if e != expected {
		t.Fatalf("expected %v, received %v", expected, auth.SafeError(e))
	}
}
func randomToken(t *testing.T) string {
	t.Helper()
	var b [32]byte
	_, e := rand.Read(b[:])
	need(t, e)
	return base64.RawURLEncoding.EncodeToString(b[:])
}
func hashed(raw string) string {
	v := sha256.Sum256([]byte(raw))
	return base64.RawURLEncoding.EncodeToString(v[:])
}

type fixture struct {
	r     *postgres.PlatformAuthRepository
	s     *auth.Service
	core  *core.Service
	ctx   context.Context
	name  string
	grant string
}

func newFixture(t *testing.T, fault func(context.Context, string) error) *fixture {
	t.Helper()
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	t.Cleanup(cancel)
	r, e := postgres.OpenPlatformAuthRepository(ctx, dsn, fault)
	need(t, e)
	t.Cleanup(func() { need(t, r.Close()) })
	s, e := newService(r, nil)
	need(t, e)
	cr, e := postgres.OpenPlatformCoreRepository(ctx, dsn, nil)
	need(t, e)
	t.Cleanup(func() { need(t, cr.Close()) })
	c, e := core.NewService(cr)
	need(t, e)
	f := &fixture{r: r, s: s, core: c, ctx: ctx, name: fmt.Sprintf("u_%s_%d", prefix, sequence.Add(1)), grant: randomToken(t)}
	f.grants(t, f.grant)
	return f
}
func (f *fixture) grants(t *testing.T, tokens ...string) {
	t.Helper()
	p := filepath.Join(t.TempDir(), "grants")
	values := []map[string]any{}
	for _, token := range tokens {
		values = append(values, map[string]any{"token": token, "expires_at": time.Now().Add(time.Hour).UTC().Format(time.RFC3339Nano)})
	}
	data, _ := json.Marshal(values)
	need(t, os.WriteFile(p, data, 0400))
	clear(data)
	need(t, f.s.BootstrapFromFiles(f.ctx, "", p))
}
func body(out auth.Outcome) map[string]any {
	var value map[string]any
	_ = json.Unmarshal(out.StorageValue().Body, &value)
	return value
}
func csrf(out auth.Outcome) string {
	v := body(out)["data"].(map[string]any)
	if c, ok := v["context"].(map[string]any); ok {
		v = c
	}
	return v["csrf_token"].(string)
}
func principal(out auth.Outcome) map[string]any {
	v := body(out)["data"].(map[string]any)
	if c, ok := v["context"].(map[string]any); ok {
		v = c
	}
	return v["principal"].(map[string]any)
}
func (f *fixture) anonymous(t *testing.T) auth.Outcome {
	t.Helper()
	out, e := f.s.Context(f.ctx, auth.BrowserCredential{}, f.name)
	need(t, e)
	return out
}
func (f *fixture) request(t *testing.T, action, w, a string, fields map[string]any) auth.Request {
	t.Helper()
	fields["schema_version"] = 1
	data, _ := json.Marshal(fields)
	defer clear(data)
	r, e := f.s.Decode(action, w, a, data)
	need(t, e)
	return r
}
func (f *fixture) mutate(t *testing.T, current auth.Outcome, key string, r auth.Request) (auth.Outcome, error) {
	t.Helper()
	return f.s.Mutate(f.ctx, current.StorageValue().Cookie, csrf(current), key, f.name, r)
}
func (f *fixture) register(t *testing.T, name string) auth.Outcome {
	t.Helper()
	token := randomToken(t)
	f.grants(t, token)
	anon := f.anonymous(t)
	out, e := f.mutate(t, anon, "registration-key-123", f.request(t, "register", "", "", map[string]any{"login_name": name, "password": password, "display_name": "Synthetic player", "registration_token": token}))
	need(t, e)
	return out
}
func (f *fixture) countLogin(t *testing.T, name string) int {
	t.Helper()
	db, e := sql.Open("pgx", dsn)
	need(t, e)
	defer db.Close()
	var n int
	e = db.QueryRowContext(f.ctx, `SELECT count(*) FROM platform_auth.credentials WHERE login_name=$1`, name).Scan(&n)
	need(t, auth.SafeError(e))
	return n
}

func TestOperatorSeedClosedRegistrationAndConsumedGrant(t *testing.T) {
	f := newFixture(t, nil)
	a := f.anonymous(t)
	login := f.request(t, "login", "", "", map[string]any{"login_name": "operator_seed", "password": password})
	out, e := f.mutate(t, a, "operator-login-key", login)
	need(t, e)
	if principal(out)["kind"] != "account" {
		t.Fatal("operator seed not usable")
	}
	a = f.anonymous(t)
	request := f.request(t, "register", "", "", map[string]any{"login_name": f.name, "password": password, "display_name": "Player", "registration_token": randomToken(t)})
	_, e = f.mutate(t, a, "closed-registration", request)
	want(t, e, auth.ErrDenied)
	if f.countLogin(t, f.name) != 0 {
		t.Fatal("closed registration persisted account")
	}
	request = f.request(t, "register", "", "", map[string]any{"login_name": f.name, "password": password, "display_name": "Player", "registration_token": f.grant})
	out, e = f.mutate(t, a, "allowed-register-key", request)
	need(t, e)
	if f.countLogin(t, f.name) != 1 {
		t.Fatal("account not persisted once")
	}
	f.grants(t, f.grant)
	other := f.anonymous(t)
	request = f.request(t, "register", "", "", map[string]any{"login_name": f.name + "x", "password": password, "display_name": "Other", "registration_token": f.grant})
	_, e = f.mutate(t, other, "consumed-register-key", request)
	want(t, e, auth.ErrDenied)
}
func TestUnknownDisabledAndBadPasswordUseSameError(t *testing.T) {
	f := newFixture(t, nil)
	account := f.register(t, f.name)
	id := principal(account)["account_id"].(string)
	for _, test := range []struct {
		name, login, password string
		disable               bool
	}{{"unknown", f.name + "x", password, false}, {"wrong_password", f.name, "wrong", false}, {"disabled", f.name, password, true}} {
		t.Run(test.name, func(t *testing.T) {
			if test.disable {
				need(t, f.core.DisableAccount(f.ctx, id))
			}
			a := f.anonymous(t)
			r := f.request(t, "login", "", "", map[string]any{"login_name": test.login, "password": test.password})
			out, e := f.mutate(t, a, "uniform-login-key", r)
			want(t, e, auth.ErrUnauthenticated)
			if len(out.StorageValue().Body) != 0 || out.StorageValue().Cookie.StorageValue() != "" {
				t.Fatal("failed login produced credential")
			}
		})
	}
}
func TestSessionRotationExpiryIdleAndRevocation(t *testing.T) {
	for _, kind := range []string{"idle", "absolute", "revoked"} {
		t.Run(kind, func(t *testing.T) {
			f := newFixture(t, nil)
			account := f.register(t, f.name)
			raw := account.StorageValue().Cookie
			_, e := f.s.Context(f.ctx, raw, f.name)
			need(t, e)
			if kind == "revoked" {
				need(t, f.s.Revoke(f.ctx, raw))
			} else {
				db, e := sql.Open("pgx", dsn)
				need(t, e)
				statement := `UPDATE platform_auth.sessions SET last_seen=clock_timestamp()-interval '31 minutes' WHERE token_hash=$1`
				if kind == "absolute" {
					statement = `UPDATE platform_auth.sessions SET expires_at=clock_timestamp()-interval '1 second' WHERE token_hash=$1`
				}
				_, e = db.ExecContext(f.ctx, statement, hashed(raw.StorageValue()))
				need(t, auth.SafeError(e))
				need(t, db.Close())
			}
			_, e = f.s.Context(f.ctx, raw, f.name)
			want(t, e, auth.ErrUnauthenticated)
		})
	}
	f := newFixture(t, nil)
	a := f.anonymous(t)
	request := f.request(t, "register", "", "", map[string]any{"login_name": f.name, "password": password, "display_name": "Player", "registration_token": f.grant})
	out, e := f.mutate(t, a, "rotation-register-key", request)
	need(t, e)
	if out.StorageValue().Cookie.StorageValue() == a.StorageValue().Cookie.StorageValue() {
		t.Fatal("registration did not rotate")
	}
	_, e = f.s.Context(f.ctx, a.StorageValue().Cookie, f.name)
	want(t, e, auth.ErrUnauthenticated)
	logout := f.request(t, "logout", "", "", map[string]any{})
	anonymous, e := f.mutate(t, out, "rotation-logout-key", logout)
	need(t, e)
	if anonymous.StorageValue().CookieKind != "preauth" {
		t.Fatal("logout lacks fresh anonymous context")
	}
	_, e = f.s.Context(f.ctx, out.StorageValue().Cookie, f.name)
	want(t, e, auth.ErrUnauthenticated)
}
func TestRegistrationRollbackAndRetry(t *testing.T) {
	for _, point := range []string{"after-credential", "after-session", "after-receipt", "before-commit"} {
		t.Run(point, func(t *testing.T) {
			var armed atomic.Bool
			f := newFixture(t, func(_ context.Context, p string) error {
				if armed.Load() && p == point {
					return fmt.Errorf("SYNTHETIC_RAW_DATABASE_ERROR")
				}
				return nil
			})
			a := f.anonymous(t)
			request := f.request(t, "register", "", "", map[string]any{"login_name": f.name, "password": password, "display_name": "Player", "registration_token": f.grant})
			armed.Store(true)
			out, e := f.mutate(t, a, "rollback-register-key", request)
			want(t, e, auth.ErrUnavailable)
			if out.StorageValue().Cookie.StorageValue() != "" {
				t.Fatal("rollback exported cookie")
			}
			armed.Store(false)
			if f.countLogin(t, f.name) != 0 {
				t.Fatal("credential survived rollback")
			}
			out, e = f.mutate(t, a, "rollback-register-key", request)
			need(t, e)
			if f.countLogin(t, f.name) != 1 || principal(out)["kind"] != "account" {
				t.Fatal("atomic retry failed")
			}
		})
	}
}
func TestUnknownCommitDurableRecoveryDoesNotResurrectSession(t *testing.T) {
	var armed atomic.Bool
	f := newFixture(t, func(_ context.Context, p string) error {
		if p == "after-commit-unknown" && armed.CompareAndSwap(true, false) {
			return auth.ErrOutcomeUnknown
		}
		return nil
	})
	a := f.anonymous(t)
	request := f.request(t, "register", "", "", map[string]any{"login_name": f.name, "password": password, "display_name": "Player", "registration_token": f.grant})
	armed.Store(true)
	out, e := f.mutate(t, a, "unknown-register-key", request)
	want(t, e, auth.ErrOutcomeUnknown)
	if out.StorageValue().Cookie.StorageValue() != "" || f.countLogin(t, f.name) != 1 {
		t.Fatal("unknown outcome not qualified")
	}
	var service *auth.Service
	service, e = newService(f.r, nil)
	need(t, e)
	f.s = service
	recovered, e := f.mutate(t, a, "unknown-register-key", request)
	need(t, e)
	again, e := f.mutate(t, a, "unknown-register-key", request)
	need(t, e)
	if !bytes.Equal(recovered.StorageValue().Body, again.StorageValue().Body) || recovered.StorageValue().Cookie.StorageValue() != again.StorageValue().Cookie.StorageValue() {
		t.Fatal("durable retry changed result")
	}
	changed := f.request(t, "register", "", "", map[string]any{"login_name": f.name, "password": password + "x", "display_name": "Player", "registration_token": f.grant})
	_, e = f.mutate(t, a, "unknown-register-key", changed)
	want(t, e, auth.ErrConflict)
	need(t, f.s.Revoke(f.ctx, recovered.StorageValue().Cookie))
	_, e = f.mutate(t, a, "unknown-register-key", request)
	want(t, e, auth.ErrUnauthenticated)
	if f.countLogin(t, f.name) != 1 {
		t.Fatal("retry duplicated account")
	}
}

type verifiedAdmission struct {
	token string
	guest core.Guest
}

func (v verifiedAdmission) Verify(ctx context.Context, tx core.Transaction, cookie auth.BrowserCredential) (core.Guest, error) {
	if cookie.StorageValue() != v.token {
		return core.Guest{}, auth.ErrDenied
	}
	return tx.Guest(ctx, v.guest.Scope, v.guest.ID)
}
func (f *fixture) guest(t *testing.T) auth.Outcome {
	t.Helper()
	owner := f.register(t, f.name+"o")
	ownerID := principal(owner)["account_id"].(string)
	a, e := f.core.AccountActor(f.ctx, ownerID)
	need(t, e)
	w := f.name + "-workspace"
	need(t, f.core.CreateWorkspace(f.ctx, a, w, "Guest workspace"))
	scope := core.Scope{WorkspaceID: w, RoomID: "room", GameID: "game"}
	g := core.Guest{Scope: scope, ID: f.name + "-participation", ExpiresAt: time.Now().Add(time.Hour)}
	need(t, f.core.ProvisionGuest(f.ctx, a, scope, g.ID, g.ExpiresAt))
	admission := randomToken(t)
	anonymous := f.anonymous(t)
	request := f.request(t, "exchange", "", "", map[string]any{"admission_token": admission})
	_, e = f.mutate(t, anonymous, "missing-verifier-key", request)
	want(t, e, auth.ErrDenied)
	f.s, e = newService(f.r, verifiedAdmission{token: admission, guest: g})
	need(t, e)
	out, e := f.mutate(t, anonymous, "verified-exchange-key", request)
	need(t, e)
	return out
}
func TestGuestClaimPreservesScopeWithoutMembership(t *testing.T) {
	for _, mode := range []string{"new_account", "existing_account"} {
		t.Run(mode, func(t *testing.T) {
			f := newFixture(t, nil)
			guest := f.guest(t)
			fields := map[string]any{"mode": mode, "login_name": f.name, "password": password}
			if mode == "new_account" {
				fields["display_name"] = "Claimed player"
			} else {
				_ = f.register(t, f.name)
			}
			login := f.request(t, "login", "", "", map[string]any{"login_name": f.name, "password": password})
			_, e := f.mutate(t, guest, "guest-login-denial", login)
			want(t, e, auth.ErrClaimRequired)
			r := f.request(t, "claim", "", "", fields)
			claimed, e := f.mutate(t, guest, "claim-participation", r)
			need(t, e)
			p := body(claimed)["data"].(map[string]any)["participation"].(map[string]any)
			gp := principal(guest)
			if p["participation_id"] != gp["participation_id"] || fmt.Sprint(p["scope"]) != fmt.Sprint(gp["scope"]) {
				t.Fatal("claim changed participation boundary")
			}
			workspace := p["scope"].(map[string]any)["workspace_id"].(string)
			_, e = f.s.Workspace(f.ctx, claimed.StorageValue().Cookie, workspace)
			want(t, e, auth.ErrDenied)
			_, e = f.s.Context(f.ctx, guest.StorageValue().Cookie, f.name)
			want(t, e, auth.ErrUnauthenticated)
			replay, e := f.mutate(t, guest, "claim-participation", r)
			need(t, e)
			if !bytes.Equal(replay.StorageValue().Body, claimed.StorageValue().Body) {
				t.Fatal("lost claim response cannot replay")
			}
			need(t, f.s.Revoke(f.ctx, claimed.StorageValue().Cookie))
			_, e = f.mutate(t, guest, "claim-participation", r)
			want(t, e, auth.ErrUnauthenticated)
		})
	}
}
func TestGuestClaimRollbackAndCurrentGuestRevocation(t *testing.T) {
	for _, point := range []string{"after-credential", "after-claim", "after-session", "after-receipt"} {
		t.Run(point, func(t *testing.T) {
			var armed atomic.Bool
			f := newFixture(t, func(_ context.Context, p string) error {
				if armed.Load() && p == point {
					return auth.ErrUnavailable
				}
				return nil
			})
			guest := f.guest(t)
			r := f.request(t, "claim", "", "", map[string]any{"mode": "new_account", "login_name": f.name, "password": password, "display_name": "Player"})
			armed.Store(true)
			_, e := f.mutate(t, guest, "claim-rollback-key", r)
			want(t, e, auth.ErrUnavailable)
			armed.Store(false)
			if f.countLogin(t, f.name) != 0 {
				t.Fatal("claim rollback retained credential")
			}
			_, e = f.s.Context(f.ctx, guest.StorageValue().Cookie, f.name)
			need(t, e)
			_, e = f.mutate(t, guest, "claim-rollback-key", r)
			need(t, e)
		})
	}
	f := newFixture(t, nil)
	guest := f.guest(t)
	gp := principal(guest)
	scope := gp["scope"].(map[string]any)
	boundary := core.Scope{WorkspaceID: scope["workspace_id"].(string), RoomID: scope["room_id"].(string), GameID: scope["game_id"].(string)}
	need(t, f.r.Transact(f.ctx, func(tx auth.Transaction) error {
		return tx.Core().DisableGuest(f.ctx, boundary, gp["participation_id"].(string))
	}))
	_, e := f.s.Context(f.ctx, guest.StorageValue().Cookie, f.name)
	want(t, e, auth.ErrUnauthenticated)
}
func TestTenantMembershipAndReplayRevalidateCurrentAuthority(t *testing.T) {
	f := newFixture(t, nil)
	owner := f.register(t, f.name)
	admin := f.register(t, f.name+"a")
	outsider := f.register(t, f.name+"x")
	adminID := principal(admin)["account_id"].(string)
	outID := principal(outsider)["account_id"].(string)
	create := f.request(t, "create_workspace", "", "", map[string]any{"name": "Workspace"})
	w, e := f.mutate(t, owner, "create-workspace-key", create)
	need(t, e)
	workspace := body(w)["data"].(map[string]any)["workspace_id"].(string)
	grant := f.request(t, "set_member", workspace, adminID, map[string]any{"role": "admin"})
	_, e = f.mutate(t, owner, "grant-admin-role-key", grant)
	need(t, e)
	grant = f.request(t, "set_member", workspace, outID, map[string]any{"role": "member"})
	_, e = f.mutate(t, admin, "admin-member-change", grant)
	need(t, e)
	_, e = f.s.Workspace(f.ctx, outsider.StorageValue().Cookie, workspace)
	need(t, e)
	remove := f.request(t, "remove_member", workspace, outID, map[string]any{})
	_, e = f.mutate(t, owner, "remove-member-key", remove)
	need(t, e)
	_, e = f.s.Workspace(f.ctx, outsider.StorageValue().Cookie, workspace)
	want(t, e, auth.ErrDenied)
	demote := f.request(t, "set_member", workspace, adminID, map[string]any{"role": "member"})
	_, e = f.mutate(t, owner, "demote-admin-role-key", demote)
	need(t, e)
	_, e = f.mutate(t, admin, "admin-member-change", grant)
	want(t, e, auth.ErrDenied)
	ownerID := principal(owner)["account_id"].(string)
	remove = f.request(t, "remove_member", workspace, ownerID, map[string]any{})
	_, e = f.mutate(t, owner, "immutable-owner-key", remove)
	want(t, e, auth.ErrDenied)
	_, e = f.s.Workspace(f.ctx, owner.StorageValue().Cookie, "another-workspace")
	want(t, e, auth.ErrDenied)
}
func TestConcurrentIdempotencyProducesOneCredentialAndSuccessor(t *testing.T) {
	f := newFixture(t, nil)
	a := f.anonymous(t)
	request := f.request(t, "register", "", "", map[string]any{"login_name": f.name, "password": password, "display_name": "Player", "registration_token": f.grant})
	var results [2]auth.Outcome
	var failures [2]error
	var wg sync.WaitGroup
	for i := range results {
		wg.Add(1)
		go func(i int) {
			defer wg.Done()
			results[i], failures[i] = f.s.Mutate(f.ctx, a.StorageValue().Cookie, csrf(a), "concurrent-register", f.name, request)
		}(i)
	}
	wg.Wait()
	for _, e := range failures {
		need(t, e)
	}
	if results[0].StorageValue().Cookie.StorageValue() != results[1].StorageValue().Cookie.StorageValue() || f.countLogin(t, f.name) != 1 {
		t.Fatal("concurrent receipt duplicated result")
	}
}
func TestActualTLSCookieOriginCSRFAndNoBodyCredential(t *testing.T) {
	f := newFixture(t, nil)
	server := httptest.NewUnstartedServer(nil)
	origin := "https://" + server.Listener.Addr().String()
	h, e := httpapi.NewHandler(f.s, origin)
	need(t, e)
	server.Config.Handler = h
	server.StartTLS()
	defer server.Close()
	client := server.Client()
	client.Jar, _ = cookiejar.New(nil)
	get, e := client.Get(origin + "/api/v1/auth/context")
	if e != nil {
		t.Fatal("owned TLS request failed")
	}
	data, e := io.ReadAll(get.Body)
	need(t, auth.SafeError(e))
	_ = get.Body.Close()
	if get.StatusCode != 200 {
		t.Fatal("anonymous HTTPS failed")
	}
	var contextBody map[string]any
	need(t, json.Unmarshal(data, &contextBody))
	csrfToken := contextBody["data"].(map[string]any)["csrf_token"].(string)
	for _, c := range get.Cookies() {
		if !c.Secure || !c.HttpOnly || c.Path != "/" || c.Domain != "" || c.SameSite != http.SameSiteStrictMode {
			t.Fatal("anonymous cookie policy differs")
		}
	}
	requestBody, _ := json.Marshal(map[string]any{"schema_version": 1, "login_name": f.name, "password": password, "display_name": "Player", "registration_token": f.grant})
	defer clear(requestBody)
	for _, test := range []struct {
		name, origin, csrf string
		status             int
	}{{"wrong_origin", "https://attacker.example", csrfToken, 403}, {"wrong_csrf", origin, strings.Repeat("A", 43), 403}, {"valid", origin, csrfToken, 200}} {
		t.Run(test.name, func(t *testing.T) {
			r, e := http.NewRequestWithContext(f.ctx, http.MethodPost, origin+"/api/v1/auth/register", bytes.NewReader(requestBody))
			need(t, e)
			r.Header.Set("Origin", test.origin)
			r.Header.Set("X-CSRF-Token", test.csrf)
			r.Header.Set("Idempotency-Key", "actual-tls-register")
			r.Header.Set("Content-Type", "application/json")
			response, e := client.Do(r)
			if e != nil {
				t.Fatal("owned TLS write failed")
			}
			defer response.Body.Close()
			data, e := io.ReadAll(response.Body)
			need(t, auth.SafeError(e))
			if response.StatusCode != test.status {
				t.Fatalf("unexpected bounded HTTP status %d", response.StatusCode)
			}
			for _, c := range response.Cookies() {
				if c.Name == httpapi.SessionCookie && c.Value != "" {
					if !c.Secure || !c.HttpOnly || c.SameSite != http.SameSiteStrictMode || c.Domain != "" || c.Path != "/" {
						t.Fatal("session cookie policy differs")
					}
					if bytes.Contains(data, []byte(c.Value)) {
						t.Fatal("raw cookie in response body")
					}
				}
			}
			if bytes.Contains(data, []byte(password)) {
				t.Fatal("password in response")
			}
		})
	}
}
