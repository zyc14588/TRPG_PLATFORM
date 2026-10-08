//go:build linux && integration

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package model_gateway_test

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/action"
	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/budget"
	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/certification"
	aicontext "github.com/zyc14588/TRPG_PLATFORM/internal/ai/context"
	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/gateway"
	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/model"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/postgres"
)

type localRequest struct {
	Model    string                           `json:"model"`
	Messages []struct{ Role, Content string } `json:"messages"`
}

func writeAnswer(w http.ResponseWriter, modelName, text string) {
	b, _ := json.Marshal(map[string]any{"model": modelName, "choices": []any{map[string]any{"index": 0, "message": map[string]any{"role": "assistant", "content": text}, "finish_reason": "stop"}}, "usage": map[string]uint64{"prompt_tokens": 2, "completion_tokens": 1, "total_tokens": 3}})
	_, _ = w.Write(b)
}
func readLocal(t *testing.T, r *http.Request) localRequest {
	t.Helper()
	var v localRequest
	if json.NewDecoder(r.Body).Decode(&v) != nil || len(v.Messages) != 2 {
		t.Error("synthetic local request invalid")
	}
	return v
}
func actionText(version uint64) string {
	b, _ := json.Marshal(action.ProposalData{Type: "increment", ExpectedVersion: version, Payload: checkpoint.Object(map[string]checkpoint.Value{"delta": checkpoint.Int(1)})})
	return string(b)
}
func actionCommands(_ context.Context, _ core.Transaction, _ aicontext.Subject) (map[string]func(checkpoint.Value) error, error) {
	return map[string]func(checkpoint.Value) error{"increment": func(v checkpoint.Value) error {
		if v.Kind != "table" || len(v.Table) != 1 || v.Table["delta"].Kind != "integer" || v.Table["delta"].Number != "1" {
			return auth.ErrDenied
		}
		return nil
	}}, nil
}
func composeContext(t *testing.T, f *isolated) {
	t.Helper()
	cs, e := postgres.NewPlatformAIContextStorage(f.r)
	need(t, e)
	f.contexts, e = aicontext.New(aicontext.Options{Authority: f.authority, Rooms: f.rt, Launches: f.lt, Models: f.models, ModelStorage: f.mt, Storage: cs, Policies: f.policy})
	need(t, e)
	f.budgets, e = budget.New(budget.Options{Authority: f.authority, Contexts: f.contexts, Storage: f.bs, WorkspaceCaps: map[string]budget.Caps{f.scope.WorkspaceID: f.caps}})
	need(t, e)
}
func gatewayFor(t *testing.T, f *isolated, timeout time.Duration, maxCalls int) *gateway.Service {
	t.Helper()
	registry, e := certification.New(f.certs)
	need(t, e)
	adapters := []*gateway.Adapter{}
	for _, endpoint := range f.endpoints {
		a, e := gateway.NewAdapter(gateway.AdapterOptions{Endpoint: endpoint, Timeout: timeout, ResponseBytes: 4096, MicrosPerToken: 1, MaxActive: 2})
		need(t, e)
		t.Cleanup(a.Close)
		adapters = append(adapters, a)
	}
	units := amount()
	units.Subagents = 0
	units.Tools = 1
	g, e := gateway.New(gateway.Options{Authority: f.authority, Contexts: f.contexts, Budgets: f.budgets, BudgetStorage: f.bs, Models: f.models, ModelStorage: f.mt, Vault: f.vault, Registry: registry, Adapters: adapters, WorkspaceCaps: map[string]budget.Caps{f.scope.WorkspaceID: f.caps}, Commands: actionCommands, Amount: units, MaxCalls: maxCalls})
	need(t, e)
	return g
}
func configuredGateway(t *testing.T, handler http.HandlerFunc, fallback bool, timeout time.Duration, maxCalls int) (*isolated, *gateway.Service) {
	t.Helper()
	f := newIsolated(t, nil, nil)
	server := httptest.NewServer(handler)
	t.Cleanup(server.Close)
	url := server.URL + "/v1"
	for i, c := range f.certs {
		v := c.StorageValue()
		v.Tuple.Endpoint = url
		f.certs[i] = model.NewCertification(v)
	}
	f.endpoints = []model.Endpoint{model.NewEndpoint(model.EndpointData{ID: "approved-local", URL: url, Adapter: "openai-compatible", Models: []string{"fixture:small", "fixture:other"}, AllowLANHTTP: true})}
	f.models = f.service(t, f.endpoints, f.certs)
	fallbacks := []string{}
	if fallback {
		fallbacks = []string{"approved-fallback"}
	}
	_, e := f.models.Configure(f.ctx, f.caller(f.owner, token(t)), model.NewConfigureRequest(model.ConfigureRequestData{Scope: f.scope, SeatID: "ai", Selection: "selected", CredentialID: "byok", FallbackIDs: fallbacks, Budget: limits(), ExpectedVersion: 1}))
	need(t, e)
	// The unrelated seat also has the new reviewed tuple, with its own key.
	_, e = f.models.Configure(f.ctx, f.caller(f.owner, token(t)), model.NewConfigureRequest(model.ConfigureRequestData{Scope: f.scope, SeatID: "ai-other", Selection: "selected", CredentialID: "byok-other", Budget: limits(), ExpectedVersion: 1}))
	need(t, e)
	composeContext(t, f)
	return f, gatewayFor(t, f, timeout, maxCalls)
}
func gatewayRequest(t *testing.T, f *isolated, id, mode string) gateway.Request {
	t.Helper()
	p, e := f.contexts.Build(f.ctx, f.caller(f.owner, ""), f.target("ai"))
	need(t, e)
	v := p.Subject().StorageValue()
	return auth.RoomSecret(gateway.RequestData{Scope: v.Scope, Binding: v.Binding, TaskID: id, PackageID: "example.test/host", ConfigurationID: v.ConfigurationID, ConfigurationHash: v.ConfigurationHash, OriginVersion: v.StateVersion, OriginPrincipal: "owned-source", SeatID: v.SeatID, Selection: "selected", Mode: mode})
}
