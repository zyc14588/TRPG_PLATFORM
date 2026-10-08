//go:build linux && integration

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package model_gateway_test

import (
	"bytes"
	"context"
	"crypto/rand"
	"encoding/hex"
	"net/http"
	"net/http/httptest"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/credential"
	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/gateway"
	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/model"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/launch"
	platformsession "github.com/zyc14588/TRPG_PLATFORM/internal/platform/session"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/postgres"
	"github.com/zyc14588/TRPG_PLATFORM/internal/task"
)

type actorGateway struct {
	*isolated
	native     *loopNativeFixture
	g          *gateway.Service
	tasks      *postgres.PlatformTaskStorage
	worker     task.Worker
	policies   []task.Policy
	completion *platformsession.Continuations
	connection *platformsession.Connection
}

func newActorGateway(t *testing.T, handler http.HandlerFunc) *actorGateway {
	t.Helper()
	l := loopNewLaunchFixture(t, nil)
	room, host := l.hostRoom(t)
	pre := l.configure(t, room, host, "ai-required")
	l.ready(t, room, pre)
	mt, e := postgres.NewPlatformModelStorage(l.r)
	need(t, e)
	v := l.owner.StorageValue()
	prep := pre.StorageValue()
	f := &fixture{ctx: l.ctx, r: l.r, rt: l.store, lt: l.storage, mt: mt, authority: l.authority, scope: prep.Scope, owner: auth.RoomSecret(actorData{ID: v.ID, Cookie: v.Cookie, CSRF: v.CSRF, Kind: "account"}), prep: prep, ack: launch.AcknowledgmentData{Scope: prep.Scope, ParticipantID: host, ConfigurationHash: prep.ConfigurationHash, GraphHash: prep.GraphHash, Revision: prep.Revision, Consent: true, Ready: true, SafetyConfirmed: true}}
	f.masterFile = filepath.Join(t.TempDir(), "master")
	raw := make([]byte, 32)
	_, e = rand.Read(raw)
	need(t, e)
	need(t, os.WriteFile(f.masterFile, raw, 0400))
	clear(raw)
	f.vault, e = credential.New(f.masterFile)
	need(t, e)
	t.Cleanup(f.vault.Close)
	server := httptest.NewServer(handler)
	t.Cleanup(server.Close)
	url := server.URL + "/v1"
	f.endpoints = []model.Endpoint{model.NewEndpoint(model.EndpointData{ID: "approved-local", URL: url, Adapter: "openai-compatible", Models: []string{"fixture:small", "fixture:other"}, AllowLANHTTP: true})}
	c := model.CertificationData{ID: "default-model", WorkspaceID: f.scope.WorkspaceID, Tuple: model.Tuple{Model: "fixture:small", Endpoint: url, Adapter: "openai-compatible", PromptTemplate: "safe-v1", ToolMode: "structured", TestVersion: "fixture-v1"}, Level: 3, Capabilities: []string{"structured-actions", "ai-player"}, Games: []model.GameEvidence{{GraphHash: prep.GraphHash, TestVersion: "fixture-v1", EvidenceHash: evidence("synthetic actual native game"), Capabilities: []string{"structured-actions", "ai-player"}}}, EvidenceHash: evidence("synthetic local provider binding"), ExpiresAt: time.Now().Add(time.Hour)}
	f.certs = []model.Certification{model.NewCertification(c)}
	f.models = f.service(t, f.endpoints, f.certs)
	f.configured(t)
	l.models = f.models
	l.recomposeLaunch(t, l.configs)
	launched, e := l.start(t, l.owner, room, prep.Revision, "ai-native-launch")
	need(t, e)
	n := &loopNativeFixture{loopLaunchFixture: l, policy: &loopNativePolicy{private: true, commands: true, points: true, export: true}, room: room, hostPart: host, launched: launched}
	n.sessions, e = postgres.NewPlatformSessionStorage(l.r)
	need(t, e)
	n.compose(t)
	bs, e := postgres.NewPlatformBudgetStorage(l.r)
	need(t, e)
	isolated := &isolated{fixture: f, bs: bs, policy: &policyProvider{grants: true, native: true}, binding: launched.StorageValue().Binding, caps: allCaps()}
	composeContext(t, isolated)
	g := gatewayFor(t, isolated, time.Second, 3)
	storage, e := postgres.NewPlatformTaskStorage(postgres.PlatformTaskOptions{Repository: l.r, Lease: 15 * time.Second, Lifetime: time.Minute})
	need(t, e)
	need(t, storage.Bootstrap(l.ctx))
	workerCredential, e := task.NewCredential(bytes.Repeat([]byte{77}, 32))
	need(t, e)
	authority, e := task.NewAuthority([]task.WorkerGrant{{ID: "owned-model-worker", Credential: workerCredential, Workspaces: []string{f.scope.WorkspaceID}, Expires: time.Now().Add(time.Hour)}})
	need(t, e)
	worker, e := authority.Authenticate(f.ctx, workerCredential)
	need(t, e)
	policy, e := g.Policy(gateway.PolicyOptions{GraphHash: isolated.binding.GraphHash, ConfigurationHash: prep.ConfigurationHash, PackageID: loopPackageID, Caller: func(ctx context.Context, scope core.Scope, _ string) (model.Caller, error) {
		if scope != f.scope || ctx.Err() != nil {
			return model.Caller{}, auth.ErrDenied
		}
		return f.caller(f.owner, ""), nil
	}, ValidateInput: func(v checkpoint.Value) error {
		if v.Kind != "table" || len(v.Table) != 3 || v.Table["seat_id"].String != "ai" || v.Table["selection"].String != "selected" || (v.Table["mode"].String != "proposal" && v.Table["mode"].String != "narrative") {
			return task.ErrInvalid
		}
		return nil
	}})
	need(t, e)
	completion, e := platformsession.NewContinuations(platformsession.ContinuationOptions{Launch: l.service, Storage: storage, Policies: []task.Policy{policy}})
	need(t, e)
	return &actorGateway{isolated: isolated, native: n, g: g, tasks: storage, worker: worker, policies: []task.Policy{policy}, completion: completion, connection: n.connect(t, l.owner, 0)}
}
func (f *actorGateway) runtime(t *testing.T) *task.Runtime {
	t.Helper()
	storage, e := gateway.BindTasks(f.tasks)
	need(t, e)
	r, e := task.NewWorker(task.WorkerOptions{Identity: f.worker, Storage: storage, Policies: f.policies, Post: f.completion.Post, MaxActive: 2, ExternalTimeout: 2 * time.Second, PostTimeout: 3 * time.Second, Poll: 50 * time.Millisecond})
	need(t, e)
	return r
}
func (f *actorGateway) source(t *testing.T) {
	t.Helper()
	_, e := f.connection.Submit(f.ctx, f.native.envelope("ai-source", "gm", "increment", 1))
	need(t, e)
}
func (f *actorGateway) version(t *testing.T) string {
	t.Helper()
	return strings.TrimSpace(string(sqlCapture(t, "SELECT version FROM host_command.sessions WHERE workspace='"+f.scope.WorkspaceID+"'")))
}
func (f *actorGateway) unlocked(ctx context.Context) error {
	scope := f.scope.WorkspaceID
	session := f.binding.Session
	if !strings.HasPrefix(scope, "w_") || session == "" {
		return task.ErrDenied
	}
	q := "BEGIN; SET LOCAL lock_timeout='100ms'; SELECT 1 FROM host_command.sessions WHERE workspace='" + scope + "' AND session='" + session + "' FOR UPDATE NOWAIT; ROLLBACK;"
	raw, e := exec.CommandContext(ctx, "docker", "exec", os.Getenv("M2B009_CONTAINER_ID"), "psql", "-U", "m2b009", "-d", "m2_b009_fixture", "-v", "ON_ERROR_STOP=1", "-Atq", "-c", q).Output()
	if e != nil || strings.TrimSpace(string(raw)) != "1" {
		return task.ErrBusy
	}
	return nil
}
func (f *actorGateway) counterValue(t *testing.T) string {
	t.Helper()
	frame, e := f.connection.Reconnect(f.ctx, 0)
	need(t, e)
	return frame.StorageValue().View.Table["counter"].Number
}
func (f *actorGateway) receipt(t *testing.T, version string) []byte {
	t.Helper()
	value := strings.TrimSpace(string(sqlCapture(t, "SELECT encode(receipt,'hex') FROM host_command.requests WHERE workspace='"+f.scope.WorkspaceID+"' AND principal='task-system' AND (convert_from(receipt,'UTF8')::jsonb->>'version')='"+version+"'")))
	raw, e := hex.DecodeString(value)
	need(t, e)
	return raw
}
