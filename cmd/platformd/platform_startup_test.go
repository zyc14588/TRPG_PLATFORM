//go:build linux && integration

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package main

import (
	"bytes"
	"context"
	"encoding/hex"
	"encoding/json"
	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/action"
	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/budget"
	aicontext "github.com/zyc14588/TRPG_PLATFORM/internal/ai/context"
	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/gateway"
	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/model"
	"github.com/zyc14588/TRPG_PLATFORM/internal/deployment/m2"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/capability"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/install"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/httpapi"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/launch"
	platformsession "github.com/zyc14588/TRPG_PLATFORM/internal/platform/session"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/command"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/postgres"
	"github.com/zyc14588/TRPG_PLATFORM/internal/task"
	"github.com/zyc14588/TRPG_PLATFORM/tests/integration/player_presentation/fixture"
	m2smoke "github.com/zyc14588/TRPG_PLATFORM/tests/smoke/m2"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"testing"
	"time"
)

func TestM2ExplicitArchivesRejectReplacementSymlinksWritableAndIdentityDrift(t *testing.T) {
	root, dep, _, e := m2smoke.BuildCarrierWithDependency(install.RuntimeConfig{SHA256: checkpoint.Hash([]byte("archive-reader fixture only")), Limits: profile.DefaultLimits()})
	if e != nil {
		t.Fatal(e)
	}
	archiveBytes, e := root.Export()
	if e != nil {
		t.Fatal(e)
	}
	raw := archiveBytes.Bytes()
	defer clear(raw)
	id := string(root.ArtifactIdentity().Digest())
	newFile := func(t *testing.T) (string, string) {
		t.Helper()
		base := t.TempDir()
		path := filepath.Join(base, "root.zip")
		if e := os.WriteFile(path, raw, 0400); e != nil {
			t.Fatal(e)
		}
		return base, path
	}
	ownedFDs := func(base string) int {
		entries, e := os.ReadDir("/proc/self/fd")
		if e != nil {
			t.Fatal(e)
		}
		n := 0
		for _, entry := range entries {
			value, e := os.Readlink(filepath.Join("/proc/self/fd", entry.Name()))
			if e == nil && strings.HasPrefix(value, base+string(filepath.Separator)) {
				n++
			}
		}
		return n
	}
	t.Run("regular-readonly-original-bytes-close", func(t *testing.T) {
		base, path := newFile(t)
		r, e := openM2Archive(context.Background(), path, id)
		if e != nil || ownedFDs(base) != 1 {
			t.Fatal("owned read-only descriptor unavailable")
		}
		actual, e := io.ReadAll(r)
		defer clear(actual)
		if e != nil || !bytes.Equal(actual, raw) {
			t.Fatal("original archive bytes changed")
		}
		if e = r.file.Close(); e != nil || ownedFDs(base) != 0 {
			t.Fatal("owned archive descriptor not closed")
		}
	})
	for _, fault := range []string{"wrong-identity", "writable", "leaf-symlink", "parent-symlink", "directory", "zero-size", "oversized"} {
		t.Run("open-"+fault, func(t *testing.T) {
			base, path := newFile(t)
			wantID := id
			switch fault {
			case "wrong-identity":
				wantID = string(dep.ArtifactIdentity().Digest())
			case "writable":
				e = os.Chmod(path, 0600)
			case "leaf-symlink":
				original := path + ".original"
				e = os.Rename(path, original)
				if e == nil {
					e = os.Symlink(original, path)
				}
			case "parent-symlink":
				alias := filepath.Join(base, "alias")
				e = os.Symlink(base, alias)
				path = filepath.Join(alias, "root.zip")
			case "directory":
				path = base
			case "zero-size":
				e = os.Chmod(path, 0600)
				if e == nil {
					e = os.Truncate(path, 0)
				}
				if e == nil {
					e = os.Chmod(path, 0400)
				}
			case "oversized":
				e = os.Chmod(path, 0600)
				if e == nil {
					e = os.Truncate(path, 80<<20+1)
				}
				if e == nil {
					e = os.Chmod(path, 0400)
				}
			}
			if e != nil {
				t.Fatal(e)
			}
			for range 8 {
				r, e := openM2Archive(context.Background(), path, wantID)
				if e == nil || r != nil || ownedFDs(base) != 0 {
					t.Fatal("unsafe archive accepted or failed descriptor leaked")
				}
			}
		})
	}
	for _, fault := range []string{"inode-replacement", "writable-after-open", "bytes-with-original-metadata", "parent-symlink-after-open"} {
		t.Run("read-"+fault, func(t *testing.T) {
			base, path := newFile(t)
			if fault == "parent-symlink-after-open" {
				dir := filepath.Join(base, "parent")
				if e := os.Mkdir(dir, 0700); e != nil {
					t.Fatal(e)
				}
				path = filepath.Join(dir, "root.zip")
				if e := os.WriteFile(path, raw, 0400); e != nil {
					t.Fatal(e)
				}
			}
			r, e := openM2Archive(context.Background(), path, id)
			if e != nil {
				t.Fatal(e)
			}
			defer r.file.Close()
			switch fault {
			case "inode-replacement":
				new := path + ".new"
				e = os.WriteFile(new, raw, 0400)
				if e == nil {
					e = os.Rename(new, path)
				}
			case "writable-after-open":
				e = os.Chmod(path, 0600)
			case "bytes-with-original-metadata":
				changed := append([]byte(nil), raw...)
				changed[len(changed)/2] ^= 1
				e = os.Chmod(path, 0600)
				if e == nil {
					e = os.WriteFile(path, changed, 0600)
				}
				clear(changed)
				if e == nil {
					e = os.Chmod(path, 0400)
				}
				if e == nil {
					e = os.Chtimes(path, r.info.ModTime(), r.info.ModTime())
				}
			case "parent-symlink-after-open":
				parent := filepath.Dir(path)
				e = os.Rename(parent, parent+".moved")
				if e == nil {
					e = os.Symlink(parent+".moved", parent)
				}
			}
			if e != nil {
				t.Fatal(e)
			}
			actual, e := io.ReadAll(r)
			clear(actual)
			if e == nil {
				t.Fatal("post-open archive identity drift accepted")
			}
			if e = r.file.Close(); e != nil || ownedFDs(base) != 0 {
				t.Fatal("owned descriptor leaked")
			}
		})
	}
	t.Run("dependency-failure-closes-previous-root", func(t *testing.T) {
		base, path := newFile(t)
		bad := filepath.Join(base, "wrong-dependency.zip")
		if e := os.WriteFile(bad, raw, 0400); e != nil {
			t.Fatal(e)
		}
		depID := string(dep.ArtifactIdentity().Digest())
		g := m2.GamePlan{ArchiveFile: path, Root: id, Dependencies: []string{depID}, DependencyArchiveFiles: map[string]string{depID: bad}}
		for range 8 {
			_, e := installM2Game(context.Background(), nil, g, "owned-test-only")
			if e == nil || ownedFDs(base) != 0 {
				t.Fatal("failed dependency leaked an earlier archive handle")
			}
		}
	})
}

func realOwnerActionFixture(t *testing.T) (*fixture.Harness, m2.Config, m2.OperatorPlan, ownerModelAction, string) {
	t.Helper()
	h := fixture.New(t, false)
	var cookie string
	h.Serve(t, func(string) http.Handler {
		return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			c, e := r.Cookie(httpapi.SessionCookie)
			if e != nil {
				t.Error("actual account cookie missing")
			} else {
				cookie = c.Value
			}
			w.Header().Set("Content-Type", "application/json")
			_, _ = w.Write([]byte(`{"schema_version":1,"data":{}}`))
		})
	})
	h.GET(t, "owner", "ai-required", nil)
	h.Compose(t)
	out, e := h.Authority().Authentication().Context(h.Context(), auth.BrowserCookie(cookie), "owned-startup-test")
	if e != nil {
		t.Fatal(auth.SafeError(e))
	}
	var envelope struct {
		Data struct {
			CSRF string `json:"csrf_token"`
		}
	}
	if json.Unmarshal(out.StorageValue().Body, &envelope) != nil || envelope.Data.CSRF == "" {
		t.Fatal("real session CSRF missing")
	}
	dir := t.TempDir()
	write := func(name string, b []byte) string {
		p := filepath.Join(dir, name)
		if e := os.WriteFile(p, b, 0400); e != nil {
			t.Fatal(e)
		}
		return p
	}
	c := m2.Config{DeploymentID: "m2-owner-one-time-test", Source: "sha256:" + strings.Repeat("1", 64)}
	plan := m2.OperatorPlan{Games: []m2.GamePlan{{Workspace: h.Workspace(), Game: h.Scope().GameID, Seats: []launch.SeatRule{{ID: "ai", Modes: []string{"ai"}}}}}}
	a := ownerModelAction{DeploymentID: c.DeploymentID, Source: c.Source, ExpiresAt: time.Now().Add(4 * time.Minute), Scope: h.Scope(), SeatID: "ai", Selection: "selected", CredentialID: "explicit-key", CredentialIdempotencyKey: "m2-explicit-key-once", ConfigurationIdempotencyKey: "m2-explicit-config-once", ExpectedVersion: 1, Budget: fixture.Limits(), CookieFile: write("cookie", []byte(cookie)), CSRFFile: write("csrf", []byte(envelope.Data.CSRF)), KeyFile: write("key", []byte("owned-B011-one-time-synthetic-key"))}
	path := filepath.Join(dir, "action.json")
	return h, c, plan, a, path
}
func saveOwnerAction(t *testing.T, path string, a ownerModelAction) {
	t.Helper()
	b, e := json.Marshal(a)
	if e != nil {
		t.Fatal(e)
	}
	_ = os.Remove(path)
	if e = os.WriteFile(path, b, 0400); e != nil {
		t.Fatal(e)
	}
}
func ownerCaller(t *testing.T, a ownerModelAction, key string) model.Caller {
	t.Helper()
	cookie, e := m2.ReadSecret(a.CookieFile, 4096)
	if e != nil {
		t.Fatal(e)
	}
	defer clear(cookie)
	csrf, e := m2.ReadSecret(a.CSRFFile, 256)
	if e != nil {
		t.Fatal(e)
	}
	defer clear(csrf)
	return auth.RoomSecret(model.CallerData{Credential: auth.BrowserCookie(string(cookie)), CSRF: string(csrf), IdempotencyKey: key})
}
func TestOneTimeOwnerActionCurrentSessionAndIdempotency(t *testing.T) {
	h, c, plan, a, path := realOwnerActionFixture(t)
	saveOwnerAction(t, path, a)
	before := h.RoomModel(t)
	if e := applyOwnerModelAction(h.Context(), c, plan, h.Models, path); e != nil {
		t.Fatal(auth.SafeError(e))
	}
	after := h.RoomModel(t)
	if after.Version != before.Version+1 || after.CredentialID != a.CredentialID {
		t.Fatal("explicit typed configuration not applied")
	}
	baseline := h.Baseline(t)
	if e := applyOwnerModelAction(h.Context(), c, plan, h.Models, path); e != nil {
		t.Fatal(auth.SafeError(e))
	}
	if baseline != h.Baseline(t) || h.ProviderCalls.Load() != 0 {
		t.Fatal("repeated explicit action mutated or dispatched")
	}
	target := auth.RoomSecret(model.TargetData{Scope: a.Scope, SeatID: a.SeatID, ID: a.CredentialID})
	if e := h.Models.RevokeCredential(h.Context(), ownerCaller(t, a, "m2-revoke-explicit"), target); e != nil {
		t.Fatal(auth.SafeError(e))
	}
	baseline = h.Baseline(t)
	if e := applyOwnerModelAction(h.Context(), c, plan, h.Models, path); e == nil {
		t.Fatal("explicit action revived revoked credential")
	}
	if baseline != h.Baseline(t) || h.ProviderCalls.Load() != 0 {
		t.Fatal("revocation denial mutated or dispatched")
	}
}
func TestOwnerStoreSuccessConfigureFailureKeepsOriginalTwoStepSemantics(t *testing.T) {
	h, c, plan, a, path := realOwnerActionFixture(t)
	a.ExpectedVersion = 999
	saveOwnerAction(t, path, a)
	before := h.RoomModel(t)
	old := h.SQL(t, "SELECT md5(row_to_json(c)::text) FROM platform_model.credentials c WHERE workspace_id='"+h.Workspace()+"' AND id='byok'")
	if e := applyOwnerModelAction(h.Context(), c, plan, h.Models, path); e == nil {
		t.Fatal("failed CAS reported completed setup")
	}
	if h.RoomModel(t).Version != before.Version || h.RoomModel(t).CredentialID != before.CredentialID {
		t.Fatal("failed configure changed old configuration")
	}
	if h.SQL(t, "SELECT count(*) FROM platform_model.credentials WHERE workspace_id='"+h.Workspace()+"' AND id='explicit-key' AND NOT revoked") != "1" {
		t.Fatal("successful original StoreCredential lost")
	}
	if h.SQL(t, "SELECT md5(row_to_json(c)::text) FROM platform_model.credentials c WHERE workspace_id='"+h.Workspace()+"' AND id='byok'") != old || h.ProviderCalls.Load() != 0 {
		t.Fatal("partial failure deleted/regranted old key or dispatched")
	}
}
func TestOwnerActionDenialsDoNotRestoreAuthority(t *testing.T) {
	for _, fault := range []string{"expired", "source", "csrf", "unknown-field", "writable-key", "cookie-revoked", "membership", "host-withdrawn", "preparation-changed", "canceled"} {
		t.Run(fault, func(t *testing.T) {
			h, c, plan, a, path := realOwnerActionFixture(t)
			ctx := h.Context()
			switch fault {
			case "expired":
				a.ExpiresAt = time.Now().Add(-time.Second)
			case "source":
				a.Source = "sha256:" + strings.Repeat("2", 64)
			case "csrf":
				_ = os.Remove(a.CSRFFile)
				if e := os.WriteFile(a.CSRFFile, []byte("invalid-csrf"), 0400); e != nil {
					t.Fatal(e)
				}
			case "writable-key":
				if e := os.Chmod(a.KeyFile, 0600); e != nil {
					t.Fatal(e)
				}
			case "cookie-revoked":
				h.RevokeCookie(t)
			case "membership":
				var memberCookie string
				h.Serve(t, func(string) http.Handler {
					return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
						if c, e := r.Cookie(httpapi.SessionCookie); e == nil {
							memberCookie = c.Value
						}
						w.Header().Set("Content-Type", "application/json")
						_, _ = w.Write([]byte(`{"schema_version":1,"data":{}}`))
					})
				})
				h.GET(t, "participant", "ai-required", nil)
				h.Compose(t)
				out, e := h.Authority().Authentication().Context(ctx, auth.BrowserCookie(memberCookie), "owned-membership-test")
				if e != nil {
					t.Fatal(auth.SafeError(e))
				}
				var data struct {
					Data struct {
						CSRF      string `json:"csrf_token"`
						Principal struct {
							Account string `json:"account_id"`
						} `json:"principal"`
					}
				}
				if json.Unmarshal(out.StorageValue().Body, &data) != nil || data.Data.Principal.Account == "" || data.Data.CSRF == "" {
					t.Fatal("actual administrative account session missing")
				}
				_ = h.SQL(t, "INSERT INTO platform_core.memberships(workspace_id,account_id,role) VALUES('"+h.Workspace()+"','"+data.Data.Principal.Account+"','admin')")
				for p, b := range map[string][]byte{a.CookieFile: []byte(memberCookie), a.CSRFFile: []byte(data.Data.CSRF)} {
					if e := os.Remove(p); e != nil {
						t.Fatal(e)
					}
					if e := os.WriteFile(p, b, 0400); e != nil {
						t.Fatal(e)
					}
				}
				_ = h.SQL(t, "DELETE FROM platform_core.memberships WHERE workspace_id='"+h.Workspace()+"' AND account_id='"+data.Data.Principal.Account+"'")
			case "host-withdrawn":
				h.UnseatHost(t)
			case "preparation-changed":
				h.ChangeConfiguration(t, "minimal")
			case "canceled":
				cancelled, cancel := context.WithCancel(ctx)
				cancel()
				ctx = cancelled
			}
			saveOwnerAction(t, path, a)
			if fault == "unknown-field" {
				b, e := os.ReadFile(path)
				if e != nil {
					t.Fatal(e)
				}
				_ = os.Remove(path)
				if e = os.WriteFile(path, append(b[:len(b)-1], []byte(`,"Endpoint":"https://other.invalid"}`)...), 0400); e != nil {
					t.Fatal(e)
				}
			}
			before := h.Baseline(t)
			if e := applyOwnerModelAction(ctx, c, plan, h.Models, path); e == nil {
				t.Fatal("owner action denial bypassed", fault)
			}
			if before != h.Baseline(t) || h.ProviderCalls.Load() != 0 {
				t.Fatal("denied action mutated or dispatched", fault)
			}
		})
	}
}

// Native guard fixtures use an actually installed, launched Lua carrier, real
// account/CSRF, original Claim/Reserve/Dispatch/Post and owned PostgreSQL.
type m2GuardContextPolicy struct{}

func (m2GuardContextPolicy) Current(_ context.Context, _ core.Transaction, subject aicontext.Subject) (aicontext.Policy, error) {
	v := subject.StorageValue()
	declaration, e := capability.NewDeclaration(nil, nil)
	if e != nil {
		return aicontext.Policy{}, e
	}
	trust, e := capability.NewTrustPolicy(map[capability.TrustLevel][]string{capability.TrustOfficial: {}, capability.TrustSigned: {}, capability.TrustPrivateUnverified: {}, capability.TrustDevelopment: {}})
	if e != nil {
		return aicontext.Policy{}, e
	}
	execution, e := capability.NewGrantSet(nil)
	if e != nil {
		return aicontext.Policy{}, e
	}
	return aicontext.Policy{Binding: v.Binding, SeatID: v.SeatID, Tuple: v.Tuple, Role: "player", GeneratorVersion: "m2-read-guard-fixture", Views: command.ViewPolicy{ViewFields: []string{"counter"}, EventFields: map[string][]string{fixture.PackageID + "/change": {"counter"}}}, Declaration: declaration, TrustLevel: capability.TrustDevelopment, Trust: trust, Execution: execution}, nil
}

type m2ReadGuardFixture struct {
	h           *fixture.Harness
	players     *platformPlayerComponents
	caller      model.Caller
	peer        model.Caller
	contexts    *aicontext.Service
	storage     *postgres.PlatformTaskStorage
	worker      task.Worker
	job         task.Job
	reservation budget.Reservation
	caps        budget.Caps
	amount      budget.Units
	prompt      aicontext.Prompt
	request     gateway.RequestData
	execution   *m2Execution
	ctx         context.Context
	callers     *browserCallers
	connection  string
}

func m2BrowserFixtureCaller(t *testing.T, h *fixture.Harness, which string) model.Caller {
	t.Helper()
	var cookie string
	h.Serve(t, func(string) http.Handler {
		return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			if c, e := r.Cookie(httpapi.SessionCookie); e == nil {
				cookie = c.Value
			}
			w.Header().Set("Content-Type", "application/json")
			_, _ = w.Write([]byte(`{"schema_version":1,"data":{}}`))
		})
	})
	h.GET(t, which, "ai-required", nil)
	h.Compose(t)
	out, e := h.Authority().Authentication().Context(h.Context(), auth.BrowserCookie(cookie), "m2-native-guard-fixture")
	if e != nil {
		t.Fatal(auth.SafeError(e))
	}
	var result struct {
		Data struct {
			CSRF string `json:"csrf_token"`
		}
	}
	if json.Unmarshal(out.StorageValue().Body, &result) != nil || result.Data.CSRF == "" {
		t.Fatal("actual browser CSRF missing")
	}
	return auth.RoomSecret(model.CallerData{Credential: auth.BrowserCookie(cookie), CSRF: result.Data.CSRF, Network: "m2-native-guard-fixture"})
}
func (f *m2ReadGuardFixture) playerCall(t *testing.T, caller model.Caller, action string, fields map[string]any) map[string]any {
	t.Helper()
	fields["schema_version"] = 1
	raw, e := json.Marshal(fields)
	if e != nil {
		t.Fatal(e)
	}
	defer clear(raw)
	q, e := f.players.state().players.Decode(action, f.h.Scope().WorkspaceID, f.h.Scope().RoomID, raw)
	if e != nil {
		t.Fatal(auth.SafeError(e))
	}
	c := caller.StorageValue()
	out, e := f.players.state().players.Do(f.h.Context(), auth.RoomSecret(launch.CallerData{Credential: c.Credential, CSRF: c.CSRF, IdempotencyKey: "m2-fixture-" + action + "-" + strconv.FormatInt(time.Now().UnixNano(), 10), Network: c.Network}), q)
	if e != nil {
		t.Fatal(action, auth.SafeError(e))
	}
	var result struct {
		Data map[string]any `json:"data"`
	}
	if json.Unmarshal(out.StorageValue().Body, &result) != nil || result.Data == nil {
		t.Fatal("actual player result missing")
	}
	return result.Data
}
func newM2ReadGuardFixture(t *testing.T) *m2ReadGuardFixture {
	h := fixture.New(t, false)
	f := &m2ReadGuardFixture{h: h, caller: m2BrowserFixtureCaller(t, h, "owner"), peer: m2BrowserFixtureCaller(t, h, "participant")}
	schema, e := fixture.ReadProductFile("schemas/platform/platform-player-api-v1.schema.json")
	if e != nil {
		t.Fatal(e)
	}
	f.players, e = platformPlayers(platformPlayerOptions{ctx: h.Context(), repo: h.Repository(), authority: h.Authority(), roomStorage: h.RoomStorage(), rooms: h.Rooms(), configurations: h.Configurations(), models: h.Models, policies: h.Policies(), descriptions: h.Descriptions(), schema: schema, origin: "https://owned-guard.invalid", maxSessions: 8, maxConnections: 64})
	if e != nil {
		t.Fatal(auth.SafeError(e))
	}
	t.Cleanup(func() {
		if e := f.players.close(); e != nil {
			t.Error(auth.SafeError(e))
		}
	})
	h.Confirm(t)
	c := f.caller.StorageValue()
	lobby, e := f.players.state().launches.PlayerLobby(h.Context(), auth.RoomSecret(launch.CallerData{Credential: c.Credential}), h.Workspace(), h.Scope().RoomID)
	if e != nil {
		t.Fatal(auth.SafeError(e))
	}
	_, e = f.players.state().launches.Launch(h.Context(), auth.RoomSecret(launch.CallerData{Credential: c.Credential, CSRF: c.CSRF, IdempotencyKey: "m2-native-launch", Network: c.Network}), auth.RoomSecret(launch.LaunchData{WorkspaceID: h.Workspace(), RoomID: h.Scope().RoomID, Revision: lobby.StorageValue().Preparation.Revision}))
	if e != nil {
		t.Fatal(auth.SafeError(e))
	}
	connection := f.playerCall(t, f.caller, "connect", map[string]any{"after_cursor": "0"})["connection_id"].(string)
	f.connection = connection
	f.playerCall(t, f.peer, "connect", map[string]any{"after_cursor": "0"})
	control := f.playerCall(t, f.caller, "snapshot", map[string]any{"connection_id": connection, "after_cursor": "0", "limit": 8})["control"].(map[string]any)
	control = f.playerCall(t, f.caller, "resume", map[string]any{"expected_control_revision": control["revision"]})
	control = f.playerCall(t, f.peer, "resume", map[string]any{"expected_control_revision": control["revision"]})
	if control["paused"].(bool) {
		t.Fatal("actual personal unanimous resume missing")
	}
	f.playerCall(t, f.caller, "command", map[string]any{"connection_id": connection, "command_id": "m2-guard-human-source", "expected_state_version": "1", "type": "increment", "payload": checkpoint.Object(map[string]checkpoint.Value{"delta": checkpoint.Int(1)}), "correlation_id": "m2-guard-correlation"})
	f.storage, e = postgres.NewPlatformTaskStorage(postgres.PlatformTaskOptions{Repository: h.Repository(), Lease: 15 * time.Second, Lifetime: 10 * time.Minute})
	if e != nil {
		t.Fatal(e)
	}
	if e = f.storage.Bootstrap(h.Context()); e != nil {
		t.Fatal(e)
	}
	credential, e := task.NewCredential(bytes.Repeat([]byte{17}, 32))
	if e != nil {
		t.Fatal(e)
	}
	authority, e := task.NewAuthority([]task.WorkerGrant{{ID: "m2-native-guard-worker", Credential: credential, Workspaces: []string{h.Workspace()}, Expires: time.Now().Add(time.Minute)}})
	if e != nil {
		t.Fatal(e)
	}
	f.worker, e = authority.Authenticate(h.Context(), credential)
	if e != nil {
		t.Fatal(e)
	}
	f.job, e = f.storage.Claim(h.Context(), f.worker)
	if e != nil {
		t.Fatal(task.SafeError(e))
	}
	f.prepareGuard(t, "proposal")
	return f
}
func (f *m2ReadGuardFixture) baseline(t *testing.T) string {
	t.Helper()
	hashes := []string{f.h.Baseline(t)}
	for _, table := range []string{"platform_player.control", "platform_player.leases"} {
		hashes = append(hashes, f.h.SQL(t, "SELECT md5(coalesce(string_agg(row_to_json(v)::text,'' ORDER BY row_to_json(v)::text),'')) FROM "+table+" v WHERE workspace_id='"+f.h.Workspace()+"'"))
	}
	for _, table := range []string{"platform_task.jobs", "host_command.requests", "host_command.replay_effects", "host_command.tasks", "host_command.continuations", "host_command.outbox"} {
		hashes = append(hashes, f.h.SQL(t, "SELECT md5(coalesce(string_agg(row_to_json(v)::text,'' ORDER BY row_to_json(v)::text),'')) FROM "+table+" v WHERE workspace='"+f.h.Workspace()+"'"))
	}
	accounts := "SELECT account_id FROM platform_room.participants WHERE workspace_id='" + f.h.Workspace() + "' AND account_id IS NOT NULL UNION SELECT account_id FROM platform_core.memberships WHERE workspace_id='" + f.h.Workspace() + "'"
	whereByTable := map[string]string{"platform_core.memberships": "workspace_id='" + f.h.Workspace() + "'", "platform_core.accounts": "id IN(" + accounts + ")", "platform_auth.sessions": "account_id IN(" + accounts + ")", "platform_auth.receipts": "owner_hash IN(SELECT token_hash FROM platform_auth.sessions WHERE account_id IN(" + accounts + "))"}
	for _, table := range []string{"platform_core.memberships", "platform_core.accounts", "platform_auth.sessions", "platform_auth.receipts"} {
		where := whereByTable[table]
		hashes = append(hashes, f.h.SQL(t, "SELECT md5(coalesce(string_agg(row_to_json(v)::text,'' ORDER BY row_to_json(v)::text),'')) FROM "+table+" v WHERE "+where))
	}
	return checkpoint.Hash([]byte(strings.Join(hashes, "/")))
}
func (f *m2ReadGuardFixture) begin() error {
	c := f.caller.StorageValue()
	return f.h.Authority().Inspect(f.ctx, c.Credential, c.CSRF, false, func(ctx context.Context, tx auth.Transaction, _ auth.SessionData) error {
		finish, e := m2LiveDispatchGuard(ctx, tx.Core(), f.request, map[string]budget.Caps{f.h.Workspace(): f.caps})
		if e != nil {
			return e
		}
		if finish == nil {
			return auth.ErrDenied
		}
		fresh, e := f.contexts.BuildWithin(ctx, tx.Core(), auth.RoomSecret(aicontext.TargetData{Scope: f.request.Scope, SeatID: f.request.SeatID}))
		if e != nil {
			return e
		}
		return finish(ctx, fresh.Subject().StorageValue(), f.amount, uint64(fresh.Bytes()))
	})
}

// Count actual fixed-endpoint provider bytes only after the guard transaction
// commits. Production Dispatch.Begin uses this same two-stage read group.
func (f *m2ReadGuardFixture) release(t *testing.T) error {
	t.Helper()
	if e := f.begin(); e != nil {
		return e
	}
	m := f.h.RoomModel(t)
	endpoint := model.NewEndpoint(model.EndpointData{ID: "m2-owned-synthetic", URL: m.Tuple.Endpoint, Adapter: m.Tuple.Adapter, Models: []string{m.Tuple.Model}, AllowLANHTTP: true})
	egress, e := gateway.NewEgress(gateway.AdapterOptions{Endpoint: endpoint, Timeout: time.Second, ResponseBytes: 4096, MicrosPerToken: 1, MaxActive: 1})
	if e != nil {
		t.Fatal(e)
	}
	defer egress.Close()
	var prompt []byte
	if e = f.prompt.Use(func(b []byte) error { prompt = bytes.Clone(b); return nil }); e != nil {
		t.Fatal(e)
	}
	defer clear(prompt)
	type message struct {
		Role    string `json:"role"`
		Content string `json:"content"`
	}
	body, e := json.Marshal(struct {
		Model     string    `json:"model"`
		Messages  []message `json:"messages"`
		MaxTokens uint64    `json:"max_tokens"`
		Stream    bool      `json:"stream"`
	}{m.Tuple.Model, []message{{"system", "bounded read-guard test"}, {"user", string(prompt)}}, f.amount.Tokens, false})
	if e != nil {
		t.Fatal(e)
	}
	defer clear(body)
	reply, e := egress.Execute(f.h.Context(), gateway.ProviderBytes{Body: body, Authorization: []byte("owned-B014-synthetic-private-provider-key"), Binding: egress.Binding(), Bound: f.amount})
	clear(reply)
	return e
}
func TestM2BeginOwnDispatchedReservationAndFiveHoldsReadOnly(t *testing.T) {
	f := newM2ReadGuardFixture(t)
	before := f.baseline(t)
	if e := f.begin(); e != nil {
		t.Fatal(auth.SafeError(e))
	}
	if before != f.baseline(t) {
		t.Fatal("read guard wrote durable state")
	}
	_ = f.release(t)
	if f.h.ProviderCalls.Load() != 1 || before != f.baseline(t) {
		t.Fatal("own dispatched reservation was mistaken for another uncertainty or guard wrote counters")
	}
}
func TestM2BeginDenialsReleaseZeroProviderBytesAndPreserveAllRecords(t *testing.T) {
	for _, fault := range []string{"missing-capture", "missing-reservation", "closed-capture", "request-source", "request-scope", "request-graph", "lease-replaced", "lease-expired", "job-cancelled", "source-tampered", "reservation-missing", "reservation-hash", "reservation-uncertain", "other-dispatched", "other-uncertain", "hold-missing", "cap-changed", "explicit-pause", "control-paused", "lease-disconnected", "peer-cookie-revoked", "source-cookie-revoked", "host-withdrawn", "model-revoked", "credential-revoked", "consent-withdrawn", "wrong-caller", "wrong-cached-principal"} {
		t.Run(fault, func(t *testing.T) {
			f := newM2ReadGuardFixture(t)
			w := f.h.Workspace()
			j, _ := f.job.StorageValue()
			sql := func(q string) { f.h.SQL(t, q) }
			switch fault {
			case "missing-capture":
				f.ctx = f.h.Context()
			case "missing-reservation":
				f.execution.reservation = nil
			case "closed-capture":
				f.execution.close()
			case "request-source":
				f.request.OriginPrincipal = "forged-human"
			case "request-scope":
				f.request.Scope.RoomID = "other-room"
			case "request-graph":
				f.request.Binding.GraphHash = checkpoint.Hash([]byte("other-graph"))
			case "lease-replaced":
				sql("UPDATE platform_task.jobs SET lease_digest='" + strings.Repeat("a", 64) + "' WHERE workspace='" + w + "'")
			case "lease-expired":
				sql("UPDATE platform_task.jobs SET lease_until=clock_timestamp()-interval '1 second' WHERE workspace='" + w + "'")
			case "job-cancelled":
				if e := f.storage.Cancel(f.h.Context(), f.worker, j.Binding, j.TaskID); e != nil {
					t.Fatal(e)
				}
			case "source-tampered":
				sql("UPDATE host_command.replay_effects SET evidence=decode('7b7d','hex') WHERE workspace='" + w + "' AND command_id='" + j.SourceCommand + "'")
			case "reservation-missing":
				copy := f.reservation.StorageValue()
				copy.Task.ID = strings.Repeat("e", 32)
				f.execution.reservation = &copy
			case "reservation-hash":
				copy := f.reservation.StorageValue()
				copy.RequestHash = strings.Repeat("a", 64)
				f.execution.reservation = &copy
			case "reservation-uncertain":
				sql("UPDATE platform_budget.reservations SET status='uncertain' WHERE workspace_id='" + w + "'")
			case "other-dispatched", "other-uncertain":
				status := "dispatched"
				if fault == "other-uncertain" {
					status = "uncertain"
				}
				sql("INSERT INTO platform_budget.tasks SELECT workspace_id,room_id,game_id,session_id,seat_id,'ffffffffffffffffffffffffffffffff',controller,graph_hash,body,'" + status + "' FROM platform_budget.tasks WHERE workspace_id='" + w + "'")
				sql("INSERT INTO platform_budget.reservations SELECT workspace_id,room_id,game_id,session_id,seat_id,'ffffffffffffffffffffffffffffffff',body,spent,'" + status + "' FROM platform_budget.reservations WHERE workspace_id='" + w + "'")
			case "hold-missing":
				sql("UPDATE platform_budget.counters SET held=decode('7b7d','hex') WHERE workspace_id='" + w + "' AND level='task'")
			case "cap-changed":
				f.caps.Workspace = budget.Units{}
			case "explicit-pause":
				sql("INSERT INTO platform_budget.pauses(workspace_id,session_id,seat_id,reason) VALUES('" + w + "','" + j.Binding.Session + "','ai','uncertain')")
			case "control-paused":
				control := f.playerCall(t, f.caller, "snapshot", map[string]any{"connection_id": f.connection, "after_cursor": "0", "limit": 8})["control"].(map[string]any)
				f.playerCall(t, f.caller, "pause", map[string]any{"expected_control_revision": control["revision"]})
			case "lease-disconnected":
				sql("UPDATE platform_player.leases SET expires_at=clock_timestamp()-interval '1 second' WHERE workspace_id='" + w + "' AND seat_id='player'")
			case "peer-cookie-revoked":
				sql("UPDATE platform_auth.sessions SET revoked=true WHERE token_hash IN (SELECT cookie_hash FROM platform_player.leases WHERE workspace_id='" + w + "' AND seat_id='player')")
			case "source-cookie-revoked":
				f.h.RevokeCookie(t)
			case "host-withdrawn":
				f.h.UnseatHost(t)
			case "model-revoked":
				f.h.RevokeModel(t)
			case "credential-revoked":
				f.h.RevokeCredential(t)
			case "consent-withdrawn":
				sql("UPDATE platform_launch.acknowledgments SET body=convert_to(jsonb_set(convert_from(body,'UTF8')::jsonb,'{Consent}','false'::jsonb)::text,'UTF8') WHERE workspace_id='" + w + "'")
			case "wrong-caller":
				f.caller = f.peer
			case "wrong-cached-principal":
				f.execution.callerParticipant = "other-human"
			}
			before := f.baseline(t)
			if e := f.release(t); e == nil {
				t.Fatal("denied read guard returned success", fault)
			}
			if f.h.ProviderCalls.Load() != 0 || before != f.baseline(t) {
				t.Fatal("denial emitted provider bytes or changed task/source/budget/control/receipt state", fault)
			}
		})
	}
}
func TestM2BeginLeaseLockSerializesOriginalCancelWithoutDeadlock(t *testing.T) {
	f := newM2ReadGuardFixture(t)
	j, _ := f.job.StorageValue()
	locked := make(chan struct{})
	release := make(chan struct{})
	finished := make(chan error, 1)
	c := f.caller.StorageValue()
	go func() {
		finished <- f.h.Authority().Inspect(f.ctx, c.Credential, c.CSRF, false, func(ctx context.Context, tx auth.Transaction, _ auth.SessionData) error {
			finish, e := m2LiveDispatchGuard(ctx, tx.Core(), f.request, map[string]budget.Caps{f.h.Workspace(): f.caps})
			if e != nil {
				return e
			}
			fresh, e := f.contexts.BuildWithin(ctx, tx.Core(), auth.RoomSecret(aicontext.TargetData{Scope: f.request.Scope, SeatID: f.request.SeatID}))
			if e != nil {
				return e
			}
			if e = finish(ctx, fresh.Subject().StorageValue(), f.amount, uint64(fresh.Bytes())); e != nil {
				return e
			}
			close(locked)
			<-release
			return nil
		})
	}()
	select {
	case <-locked:
	case e := <-finished:
		t.Fatal("live guard did not obtain original locks", e)
	case <-time.After(time.Second):
		t.Fatal("live guard lock timeout")
	}
	cancelled := make(chan error, 1)
	go func() { cancelled <- f.storage.Cancel(f.h.Context(), f.worker, j.Binding, j.TaskID) }()
	select {
	case <-cancelled:
		close(release)
		t.Fatal("cancel bypassed current Job lock")
	case <-time.After(60 * time.Millisecond):
	}
	close(release)
	if e := <-finished; e != nil {
		t.Fatal(e)
	}
	select {
	case e := <-cancelled:
		if e != nil {
			t.Fatal(e)
		}
	case <-time.After(time.Second):
		t.Fatal("original cancel deadlocked with read group")
	}
	before := f.baseline(t)
	if e := f.release(t); e == nil || f.h.ProviderCalls.Load() != 0 || before != f.baseline(t) {
		t.Fatal("cancelled canonical lease released bytes or guard mutated records")
	}
}

func (f *m2ReadGuardFixture) prepareGuard(t *testing.T, mode string) {
	t.Helper()
	v, e := f.job.StorageValue()
	if e != nil {
		t.Fatal(e)
	}
	cs, e := postgres.NewPlatformAIContextStorage(f.h.Repository())
	if e != nil {
		t.Fatal(e)
	}
	ms, e := postgres.NewPlatformModelStorage(f.h.Repository())
	if e != nil {
		t.Fatal(e)
	}
	f.contexts, e = aicontext.New(aicontext.Options{Authority: f.h.Authority(), Rooms: f.h.RoomStorage(), Launches: f.players.state().launchStorage, Models: f.h.Models, ModelStorage: ms, Storage: cs, Policies: m2GuardContextPolicy{}})
	if e != nil {
		t.Fatal(e)
	}
	f.prompt, e = f.contexts.Build(f.h.Context(), f.caller, auth.RoomSecret(aicontext.TargetData{Scope: f.h.Scope(), SeatID: "ai"}))
	if e != nil {
		t.Fatal(auth.SafeError(e))
	}
	f.amount = budget.Units{Calls: 1, Tokens: 2, CostMicros: 2, LatencyMillis: 10, Tools: 1, ContextBytes: 4096, LocalComputeMillis: 10}
	cap := fixture.Limits()
	f.caps = budget.Caps{Workspace: cap, Room: cap, Session: cap, Seat: cap, Task: cap}
	f.request = gateway.RequestData{Scope: v.Scope, Binding: v.Binding, TaskID: v.TaskID, PackageID: v.PackageID, ConfigurationID: v.ConfigurationID, ConfigurationHash: v.ConfigurationHash, OriginPrincipal: v.OriginPrincipal, OriginVersion: v.OriginVersion, SeatID: "ai", Selection: "selected", Mode: mode}
	callerData := f.caller.StorageValue()
	bs, e := postgres.NewPlatformBudgetStorage(f.h.Repository())
	if e != nil {
		t.Fatal(e)
	}
	sourceID, e := postgres.M2ProviderSourcePrincipal(f.h.Context(), f.h.Repository(), f.worker, f.job)
	if e != nil {
		t.Fatal(auth.SafeError(e))
	}
	f.execution = &m2Execution{claim: m2ClaimCapture{f.job, f.worker}}
	f.ctx = context.WithValue(f.h.Context(), m2ExecutionKey{}, f.execution)
	t.Cleanup(f.execution.close)
	f.callers = &browserCallers{authority: f.h.Authority(), rooms: f.h.RoomStorage(), callers: map[string]model.Caller{f.h.Scope().WorkspaceID + "/" + f.h.Scope().RoomID + "/" + f.h.Scope().GameID + "/" + sourceID: f.caller}}
	if _, e = f.callers.forExecution(f.ctx, f.h.Scope(), v.OriginPrincipal, f.h.Repository()); e != nil {
		t.Fatal(auth.SafeError(e))
	}
	stage := "insert-budget-task"
	e = f.h.Authority().Inspect(f.ctx, callerData.Credential, callerData.CSRF, false, func(ctx context.Context, tx auth.Transaction, _ auth.SessionData) error {
		bt, e := (m2ObservedBudgets{bs}).Bind(tx.Core())
		if e != nil {
			return e
		}
		// Budget.Start normally generates its own 32-hex identifier. It differs
		// from the canonical worker TaskID; the production observer binds the
		// original Dispatch to that worker through its exact Subject.
		id := strings.TrimPrefix(checkpoint.Hash([]byte(v.TaskID)), "sha256:")[:32]
		record := budget.TaskData{ID: id, Subject: f.prompt.Subject().StorageValue(), State: "open"}
		if e = bt.InsertTask(ctx, auth.RoomSecret(record)); e != nil {
			return e
		}
		stage = "reserve-five-holds"
		f.reservation, e = bt.Reserve(ctx, auth.RoomSecret(budget.ReservationData{Task: record, Amount: f.amount, PromptBytes: uint64(f.prompt.Bytes()), RequestHash: strings.TrimPrefix(checkpoint.Hash([]byte("m2-real-native-guard-request")), "sha256:"), Status: "reserved"}), f.caps)
		if e != nil {
			return e
		}
		stage = "dispatch-original-reservation"
		return bt.Dispatch(ctx, f.reservation)
	})
	if e != nil {
		t.Fatal(stage, auth.SafeError(e))
	}
}

func (f *m2ReadGuardFixture) narrativeChild(t *testing.T, parentDone bool) task.JobData {
	t.Helper()
	parent, e := f.job.StorageValue()
	if e != nil {
		t.Fatal(task.SafeError(e))
	}
	c := f.caller.StorageValue()
	bs, e := postgres.NewPlatformBudgetStorage(f.h.Repository())
	if e != nil {
		t.Fatal(auth.SafeError(e))
	}
	e = f.h.Authority().Inspect(f.ctx, c.Credential, c.CSRF, false, func(ctx context.Context, tx auth.Transaction, _ auth.SessionData) error {
		st, e := bs.Bind(tx.Core())
		if e != nil {
			return e
		}
		_, e = st.Settle(ctx, f.reservation, budget.Units{}, true)
		return e
	})
	if e != nil {
		t.Fatal(auth.SafeError(e))
	}
	result, e := gateway.ResultValue(auth.RoomSecret(gateway.OutputData{Mode: "proposal", Status: "complete", Action: &action.ProposalData{Type: "increment", ExpectedVersion: 2, Payload: checkpoint.Object(map[string]checkpoint.Value{"delta": checkpoint.Int(1)})}}))
	if e != nil {
		t.Fatal(auth.SafeError(e))
	}
	inputs, e := task.NewInputs(time.Now().UnixMilli(), nil)
	if e != nil {
		t.Fatal(task.SafeError(e))
	}
	delivery, e := f.storage.SaveResult(f.h.Context(), f.worker, f.job, result, inputs)
	if e != nil {
		t.Fatal(task.SafeError(e))
	}
	transport, e := platformsession.NewContinuations(platformsession.ContinuationOptions{Launch: f.players.state().launches, Storage: f.storage, Policies: []task.Policy{{GraphHash: parent.Binding.GraphHash, ConfigurationHash: parent.ConfigurationHash, PackageID: parent.PackageID, ValidateResult: gateway.ValidateResult}}})
	if e != nil {
		t.Fatal(task.SafeError(e))
	}
	receipt, e := transport.Post(f.h.Context(), f.worker, delivery)
	if e != nil || receipt.Version != 3 {
		t.Fatal("original proposal Post", task.SafeError(e))
	}
	if parentDone {
		if e = f.storage.Complete(f.h.Context(), f.worker, delivery, receipt); e != nil {
			t.Fatal(task.SafeError(e))
		}
	}
	child, e := f.storage.Claim(f.h.Context(), f.worker)
	if e != nil {
		t.Fatal(task.SafeError(e))
	}
	v, e := child.StorageValue()
	if e != nil || v.OriginPrincipal != "task-system" || v.OriginVersion != 3 {
		t.Fatal("canonical one-hop narrative child missing")
	}
	f.execution.close()
	f.job = child
	f.prepareGuard(t, "narrative")
	return parent
}
func (f *m2ReadGuardFixture) replaceParent(t *testing.T, parent task.JobData, mutate func(*task.JobData)) {
	t.Helper()
	raw, e := hex.DecodeString(f.h.SQL(t, "SELECT encode(body,'hex') FROM platform_task.jobs WHERE workspace='"+parent.Scope.WorkspaceID+"' AND task_id='"+parent.TaskID+"'"))
	if e != nil {
		t.Fatal("protected parent decode")
	}
	defer clear(raw)
	j, e := task.DecodeStored(raw)
	if e != nil {
		t.Fatal(task.SafeError(e))
	}
	v, e := j.StorageValue()
	if e != nil {
		t.Fatal(task.SafeError(e))
	}
	mutate(&v)
	j, e = task.NewJob(v)
	if e != nil {
		t.Fatal("original Job validation", task.SafeError(e))
	}
	encoded, e := task.EncodeForStorage(j)
	if e != nil {
		t.Fatal(task.SafeError(e))
	}
	defer clear(encoded)
	f.h.SQL(t, "UPDATE platform_task.jobs SET status='"+v.Status+"',body=decode('"+hex.EncodeToString(encoded)+"','hex') WHERE workspace='"+parent.Scope.WorkspaceID+"' AND task_id='"+parent.TaskID+"'")
}
func TestM2NarrativeOneHopOriginalCommittedResumeAndExactCaller(t *testing.T) {
	for _, done := range []bool{false, true} {
		t.Run(strconv.FormatBool(done), func(t *testing.T) {
			f := newM2ReadGuardFixture(t)
			parent := f.narrativeChild(t, done)
			before := f.baseline(t)
			id, e := postgres.M2ProviderSourcePrincipal(f.ctx, f.h.Repository(), f.worker, f.job)
			if e != nil || id != parent.OriginPrincipal || f.request.OriginPrincipal != "task-system" {
				t.Fatal("exact original human association lost", auth.SafeError(e))
			}
			if e = f.begin(); e != nil {
				t.Fatal("same-transaction begin ancestry", auth.SafeError(e))
			}
			if before != f.baseline(t) {
				t.Fatal("one-hop proof wrote original records")
			}
			_ = f.release(t)
			if f.h.ProviderCalls.Load() != 1 || before != f.baseline(t) {
				t.Fatal("one-hop fixed I/O or read-only counter proof failed")
			}
		})
	}
}
func TestM2NarrativeOneHopDenialsZeroBytesAndNoDataChanges(t *testing.T) {
	for _, fault := range []string{"parent-missing", "parent-nonterminal", "parent-system-origin", "parent-other-room", "parent-other-graph", "parent-result-mismatch", "parent-input-mismatch", "parent-continuation-mismatch", "parent-unacknowledged", "duplicate-continuation", "receipt-missing", "receipt-version-mismatch", "source-not-system", "child-other-room", "child-other-graph", "child-forged-principal", "caller-cache-missing", "caller-cache-other-participant", "cookie-expired", "cookie-revoked", "participant-revoked"} {
		t.Run(fault, func(t *testing.T) {
			f := newM2ReadGuardFixture(t)
			parent := f.narrativeChild(t, true)
			child, _ := f.job.StorageValue()
			w := f.h.Workspace()
			switch fault {
			case "parent-missing":
				f.h.SQL(t, "DELETE FROM platform_task.jobs WHERE workspace='"+w+"' AND task_id='"+parent.TaskID+"'")
			case "parent-nonterminal":
				f.replaceParent(t, parent, func(v *task.JobData) { v.Status = task.Queued })
			case "parent-system-origin":
				f.replaceParent(t, parent, func(v *task.JobData) { v.OriginPrincipal = "task-system" })
			case "parent-other-room":
				f.replaceParent(t, parent, func(v *task.JobData) { v.Scope.RoomID = "other-room" })
			case "parent-other-graph":
				f.replaceParent(t, parent, func(v *task.JobData) { v.Binding.GraphHash = checkpoint.Hash([]byte("other-graph")) })
			case "parent-result-mismatch":
				f.replaceParent(t, parent, func(v *task.JobData) {
					v.Result, _ = task.NewValue(checkpoint.Object(map[string]checkpoint.Value{"status": checkpoint.Text("different-result")}))
				})
			case "parent-input-mismatch":
				f.replaceParent(t, parent, func(v *task.JobData) {
					at, random, _ := v.Inputs.StorageValue()
					v.Inputs, _ = task.NewInputs(at+1, random)
				})
			case "parent-continuation-mismatch":
				f.replaceParent(t, parent, func(v *task.JobData) { v.Continuations[0].ID = "other-continuation" })
			case "parent-unacknowledged":
				f.replaceParent(t, parent, func(v *task.JobData) { v.Next = 0 })
			case "duplicate-continuation":
				f.h.SQL(t, "UPDATE platform_task.jobs SET body=convert_to(jsonb_set(convert_from(body,'UTF8')::jsonb,'{continuations}',(convert_from(body,'UTF8')::jsonb->'continuations')||jsonb_build_array(convert_from(body,'UTF8')::jsonb->'continuations'->0))::text,'UTF8') WHERE workspace='"+w+"' AND task_id='"+parent.TaskID+"'")
			case "receipt-missing":
				f.h.SQL(t, "UPDATE host_command.requests SET receipt=decode('7b7d','hex') WHERE workspace='"+w+"' AND command_id='"+child.SourceCommand+"'")
			case "receipt-version-mismatch":
				f.h.SQL(t, "UPDATE host_command.requests SET receipt=convert_to(jsonb_set(convert_from(receipt,'UTF8')::jsonb,'{version}','999'::jsonb)::text,'UTF8') WHERE workspace='"+w+"' AND command_id='"+child.SourceCommand+"'")
			case "source-not-system":
				f.h.SQL(t, "UPDATE host_command.requests SET principal='forged-human' WHERE workspace='"+w+"' AND command_id='"+child.SourceCommand+"'")
			case "child-other-room", "child-other-graph", "child-forged-principal":
				switch fault {
				case "child-other-room":
					child.Scope.RoomID = "other-room"
				case "child-other-graph":
					child.Binding.GraphHash = checkpoint.Hash([]byte("other-graph"))
				case "child-forged-principal":
					child.OriginPrincipal = parent.OriginPrincipal
				}
				j, e := task.NewJob(child)
				if e != nil {
					t.Fatal(task.SafeError(e))
				}
				f.job = j
				f.execution.claim.job = j
			case "caller-cache-missing":
				f.callers.callers = map[string]model.Caller{}
			case "caller-cache-other-participant":
				f.callers.callers[w+"/"+f.h.Scope().RoomID+"/"+f.h.Scope().GameID+"/"+parent.OriginPrincipal] = f.peer
			case "cookie-expired":
				f.h.SQL(t, "UPDATE platform_auth.sessions SET expires_at=clock_timestamp()-interval '1 second' WHERE token_hash IN (SELECT cookie_hash FROM platform_player.leases WHERE workspace_id='"+w+"' AND seat_id='gm')")
			case "cookie-revoked":
				f.h.RevokeCookie(t)
			case "participant-revoked":
				f.h.UnseatHost(t)
			}
			before := f.baseline(t)

			probe := &m2Execution{claim: m2ClaimCapture{f.job, f.worker}}
			probeCtx := context.WithValue(f.h.Context(), m2ExecutionKey{}, probe)
			defer probe.close()
			current, _ := f.job.StorageValue()
			_, err := f.callers.forExecution(probeCtx, current.Scope, current.OriginPrincipal, f.h.Repository())
			if err == nil {
				t.Fatal("forged/uncommitted/withdrawn one-hop caller accepted", fault)
			}
			// Domain revocations are rechecked after a previously genuine capture. A
			// missing current cache only prevents a new capture; it revokes no account.
			if fault == "caller-cache-missing" || fault == "caller-cache-other-participant" {
				f.execution.close()
				f.execution = probe
				f.ctx = probeCtx
			}

			if err = f.release(t); err == nil || f.h.ProviderCalls.Load() != 0 || before != f.baseline(t) {
				t.Fatal("one-hop denial released provider bytes or changed records", fault)
			}
		})
	}
}
