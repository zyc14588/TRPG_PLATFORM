//go:build linux && integration

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package player_test

import (
	"bytes"
	"context"
	"crypto/rand"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/action"
	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/budget"
	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/certification"
	aicontext "github.com/zyc14588/TRPG_PLATFORM/internal/ai/context"
	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/credential"
	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/gateway"
	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/model"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/capability"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
	playerapi "github.com/zyc14588/TRPG_PLATFORM/internal/platform/player"
	platformsession "github.com/zyc14588/TRPG_PLATFORM/internal/platform/session"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/command"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/postgres"
	"github.com/zyc14588/TRPG_PLATFORM/internal/task"
)

type actualAI struct {
	*playerFixture
	gateway    *gateway.Service
	tasks      *postgres.ControlledPlayerTasks
	storage    task.Storage
	policies   []task.Policy
	worker     task.Worker
	completion *platformsession.Continuations
	scope      core.Scope
	binding    data.Binding
}

// This server-side policy reads the installed native state through the real
// context service. Its role grants only the AI seat's public fields and tools.
type nativeAIContextPolicy struct{}

func (nativeAIContextPolicy) Current(_ context.Context, _ core.Transaction, subject aicontext.Subject) (aicontext.Policy, error) {
	v := subject.StorageValue()
	declaration, e := capability.NewDeclaration(nil, []capability.OptionalSpec{{Name: "host.rules", Fallback: "pause"}, {Name: "host.state", Fallback: "pause"}})
	if e != nil {
		return aicontext.Policy{}, auth.ErrDenied
	}
	trust, e := capability.NewTrustPolicy(map[capability.TrustLevel][]string{capability.TrustOfficial: {"host.rules", "host.state"}, capability.TrustSigned: {}, capability.TrustPrivateUnverified: {}, capability.TrustDevelopment: {}})
	if e != nil {
		return aicontext.Policy{}, auth.ErrDenied
	}
	execution, e := capability.NewGrantSet([]string{"host.rules", "host.state"})
	if e != nil {
		return aicontext.Policy{}, auth.ErrDenied
	}
	return aicontext.Policy{Binding: v.Binding, SeatID: v.SeatID, Tuple: v.Tuple, Role: "player", GeneratorVersion: "player-owned-v1", Views: command.ViewPolicy{ViewFields: []string{"counter"}, EventFields: map[string][]string{PackageID + "/change": {"counter"}}}, Declaration: declaration, TrustLevel: capability.TrustOfficial, Trust: trust, Execution: execution, Tools: []aicontext.Tool{{ID: "read-rules", Capability: capability.HostRules, Mode: "read"}, {ID: "propose-action", Capability: capability.HostState, Mode: "propose"}}}, nil
}
func aiLimits() model.Limits {
	return model.Limits{Calls: 8, Tokens: 8192, CostMicros: 10000, LatencyMillis: 4000, Tools: 8, Subagents: 1, ContextBytes: 16384, LocalComputeMillis: 4000}
}
func newActualAI(t *testing.T, handler http.HandlerFunc) *actualAI {
	t.Helper()
	l := newLaunchFixtureWithBuilder(t, nil, aiBuild)
	n := playerFixtureWithLaunch(t, l, false)
	slots := n.humanSlots()
	slots = append(slots, map[string]any{"id": "ai", "mode": "ai", "model_selection": "selected"})
	n.api(t, n.owner, "configure", map[string]any{"configuration_id": "ai-required", "slots": slots})
	pre, e := l.service.PlayerLobby(l.ctx, l.caller(l.owner, ""), l.w, n.room)
	need(t, e)
	prep := pre.StorageValue().Preparation
	scope := prep.Scope
	mt, e := postgres.NewPlatformModelStorage(l.r)
	need(t, e)
	need(t, mt.Bootstrap(l.ctx))
	master := filepath.Join(t.TempDir(), "master")
	raw := make([]byte, 32)
	_, e = rand.Read(raw)
	need(t, e)
	need(t, os.WriteFile(master, raw, 0400))
	clear(raw)
	vault, e := credential.New(master)
	need(t, e)
	t.Cleanup(vault.Close)
	server := httptest.NewServer(handler)
	t.Cleanup(server.Close)
	endpointURL := server.URL + "/v1"
	endpoints := []model.Endpoint{model.NewEndpoint(model.EndpointData{ID: "approved-local", URL: endpointURL, Adapter: "openai-compatible", Models: []string{"fixture:small"}, AllowLANHTTP: true})}
	cert := model.NewCertification(model.CertificationData{ID: "default-model", WorkspaceID: scope.WorkspaceID, Tuple: model.Tuple{Model: "fixture:small", Endpoint: endpointURL, Adapter: "openai-compatible", PromptTemplate: "safe-v1", ToolMode: "structured", TestVersion: "fixture-v1"}, Level: 3, Capabilities: []string{"structured-actions", "ai-player"}, Games: []model.GameEvidence{{GraphHash: prep.GraphHash, TestVersion: "fixture-v1", EvidenceHash: modelEvidence("synthetic actual player Actor"), Capabilities: []string{"structured-actions", "ai-player"}}}, EvidenceHash: modelEvidence("synthetic owned local provider binding"), ExpiresAt: time.Now().Add(time.Hour)})
	models, e := model.New(model.Options{Authority: l.authority, Rooms: l.store, Launches: l.storage, Storage: mt, Vault: vault, Endpoints: endpoints, Certifications: []model.Certification{cert}, Defaults: map[string]string{l.w: "default-model"}, WorkspaceLimits: map[string]model.Limits{l.w: aiLimits()}})
	need(t, e)
	cv := l.owner.StorageValue()
	caller := func(key string) model.Caller {
		return auth.RoomSecret(model.CallerData{Credential: cv.Cookie, CSRF: cv.CSRF, IdempotencyKey: key, Network: cv.Network})
	}
	key, e := credential.NewKey([]byte("owned-player-private-synthetic-provider-key"))
	need(t, e)
	need(t, models.StoreCredential(l.ctx, caller("player-key-"+token(t)), auth.RoomSecret(model.CredentialRequestData{Scope: scope, SeatID: "ai", ID: "byok", Lifetime: credential.Retained, Key: key})))
	key.Close()
	_, e = models.Configure(l.ctx, caller("player-model-"+token(t)), model.NewConfigureRequest(model.ConfigureRequestData{Scope: scope, SeatID: "ai", Selection: "selected", CredentialID: "byok", Budget: aiLimits()}))
	need(t, e)
	l.models = models
	l.recomposeLaunch(t, l.configs)
	n.composePlayer(t)
	n.launchConfigured(t)
	binding := data.Binding{Workspace: l.w, Session: n.sessionID, GraphHash: prep.GraphHash}
	bs, e := postgres.NewPlatformBudgetStorage(l.r)
	need(t, e)
	need(t, bs.Bootstrap(l.ctx))
	cs, e := postgres.NewPlatformAIContextStorage(l.r)
	need(t, e)
	contexts, e := aicontext.New(aicontext.Options{Authority: l.authority, Rooms: l.store, Launches: l.storage, Models: models, ModelStorage: mt, Storage: cs, Policies: nativeAIContextPolicy{}})
	need(t, e)
	units := budget.Units{Calls: 8, Tokens: 20000, CostMicros: 40000, LatencyMillis: 8000, Tools: 16, Subagents: 4, ContextBytes: 65536, LocalComputeMillis: 8000}
	caps := budget.Caps{Workspace: units, Room: units, Session: units, Seat: units, Task: units}
	broker, e := budget.New(budget.Options{Authority: l.authority, Contexts: contexts, Storage: bs, WorkspaceCaps: map[string]budget.Caps{l.w: caps}})
	need(t, e)
	registry, e := certification.New([]model.Certification{cert})
	need(t, e)
	adapter, e := gateway.NewAdapter(gateway.AdapterOptions{Endpoint: endpoints[0], Timeout: time.Second, ResponseBytes: 4096, MicrosPerToken: 1, MaxActive: 2})
	need(t, e)
	t.Cleanup(adapter.Close)
	g, e := gateway.New(gateway.Options{Authority: l.authority, Contexts: contexts, Budgets: broker, BudgetStorage: bs, Models: models, ModelStorage: mt, Vault: vault, Registry: registry, Adapters: []*gateway.Adapter{adapter}, WorkspaceCaps: map[string]budget.Caps{l.w: caps}, Commands: func(_ context.Context, _ core.Transaction, _ aicontext.Subject) (map[string]func(checkpoint.Value) error, error) {
		return map[string]func(checkpoint.Value) error{"increment": func(v checkpoint.Value) error {
			if v.Kind != "table" || len(v.Table) != 1 || v.Table["delta"].Kind != "integer" || v.Table["delta"].Number != "1" {
				return auth.ErrDenied
			}
			return nil
		}}, nil
	}, Amount: budget.Units{Calls: 1, Tokens: 512, CostMicros: 1000, LatencyMillis: 300, Tools: 1, ContextBytes: 4096, LocalComputeMillis: 300}, MaxCalls: 3})
	need(t, e)
	policy, e := g.Policy(gateway.PolicyOptions{GraphHash: binding.GraphHash, ConfigurationHash: prep.ConfigurationHash, PackageID: PackageID, Caller: func(ctx context.Context, s core.Scope, _ string) (model.Caller, error) {
		if s != scope || ctx.Err() != nil {
			return model.Caller{}, auth.ErrDenied
		}
		return caller(""), nil
	}, ValidateInput: func(v checkpoint.Value) error {
		if v.Kind != "table" || len(v.Table) != 3 || v.Table["seat_id"].String != "ai" || v.Table["selection"].String != "selected" || (v.Table["mode"].String != "proposal" && v.Table["mode"].String != "narrative") {
			return task.ErrInvalid
		}
		return nil
	}})
	need(t, e)
	f := &actualAI{playerFixture: n, gateway: g, scope: scope, binding: binding}
	f.tasks, f.worker = playerWorker(t, l)
	decorated, e := gateway.BindTasks(f.tasks)
	need(t, e)
	f.storage, f.policies, e = playerapi.BindTasks(decorated, l.control, []task.Policy{policy})
	need(t, e)
	f.completion, e = platformsession.NewContinuations(platformsession.ContinuationOptions{Launch: l.service, Storage: f.tasks, Policies: []task.Policy{policy}})
	need(t, e)
	return f
}
func playerWorker(t *testing.T, l *launchFixture) (*postgres.ControlledPlayerTasks, task.Worker) {
	t.Helper()
	base, e := postgres.NewPlatformTaskStorage(postgres.PlatformTaskOptions{Repository: l.r, Lease: 15 * time.Second, Lifetime: time.Minute})
	need(t, e)
	need(t, base.Bootstrap(l.ctx))
	controlled, e := postgres.NewControlledPlayerTasks(base, l.playerStorage)
	need(t, e)
	credential, e := task.NewCredential(bytes.Repeat([]byte{77}, 32))
	need(t, e)
	authority, e := task.NewAuthority([]task.WorkerGrant{{ID: "owned-player-worker", Credential: credential, Workspaces: []string{l.w}, Expires: time.Now().Add(time.Hour)}})
	need(t, e)
	worker, e := authority.Authenticate(l.ctx, credential)
	need(t, e)
	return controlled, worker
}
func (f *actualAI) runtime(t *testing.T) *task.Runtime {
	t.Helper()
	r, e := task.NewWorker(task.WorkerOptions{Identity: f.worker, Storage: f.storage, Policies: f.policies, Post: f.completion.Post, MaxActive: 2, ExternalTimeout: 2 * time.Second, PostTimeout: 3 * time.Second, Poll: 50 * time.Millisecond})
	need(t, e)
	return r
}
func (n *playerFixture) pauseNow(t *testing.T) map[string]any {
	t.Helper()
	ctl := n.snapshot(t, n.owner, n.hostConnection, "0", 8)["control"].(map[string]any)
	return n.api(t, n.participant, "pause", map[string]any{"expected_control_revision": ctl["revision"]})
}
func (n *playerFixture) resumeAll(t *testing.T) {
	t.Helper()
	ctl := n.snapshot(t, n.owner, n.hostConnection, "0", 8)["control"].(map[string]any)
	for _, a := range []actor{n.owner, n.participant} {
		ctl = n.api(t, a, "resume", map[string]any{"expected_control_revision": ctl["revision"]})
	}
	if ctl["paused"].(bool) {
		t.Fatal("current human confirmations did not resume")
	}
}
func (f *actualAI) source(t *testing.T) {
	t.Helper()
	f.api(t, f.owner, "command", f.commandFields(f.hostConnection, "player-ai-source", "1"))
}
func (f *actualAI) version(t *testing.T) string {
	t.Helper()
	return f.sql(t, "SELECT version FROM host_command.sessions WHERE workspace='"+f.w+"'")
}
func (f *actualAI) unlocked(ctx context.Context) error {
	q := "BEGIN; SET LOCAL lock_timeout='100ms'; SELECT 1 FROM host_command.sessions WHERE workspace='" + f.w + "' AND session='" + f.sessionID + "' FOR UPDATE NOWAIT; ROLLBACK;"
	raw, e := exec.CommandContext(ctx, "docker", "exec", os.Getenv("M2B013_CONTAINER_ID"), "psql", "-U", "m2b013", "-d", "m2_b013_fixture", "-v", "ON_ERROR_STOP=1", "-Atq", "-c", q).Output()
	if e != nil || strings.TrimSpace(string(raw)) != "1" {
		return task.ErrBusy
	}
	return nil
}

type localAIRequest struct {
	Model    string                           `json:"model"`
	Messages []struct{ Role, Content string } `json:"messages"`
}

func readAI(t *testing.T, r *http.Request) localAIRequest {
	t.Helper()
	var v localAIRequest
	if json.NewDecoder(r.Body).Decode(&v) != nil || len(v.Messages) != 2 {
		t.Error("owned local provider request invalid")
	}
	return v
}
func writeAI(w http.ResponseWriter, name, text string) {
	_ = json.NewEncoder(w).Encode(map[string]any{"model": name, "choices": []any{map[string]any{"index": 0, "message": map[string]any{"role": "assistant", "content": text}, "finish_reason": "stop"}}, "usage": map[string]uint64{"prompt_tokens": 2, "completion_tokens": 1, "total_tokens": 3}})
}
func proposalText(version uint64) string {
	b, _ := json.Marshal(action.ProposalData{Type: "increment", ExpectedVersion: version, Payload: checkpoint.Object(map[string]checkpoint.Value{"delta": checkpoint.Int(1)})})
	return string(b)
}

func modelEvidence(s string) string { v := sha256.Sum256([]byte(s)); return hex.EncodeToString(v[:]) }
