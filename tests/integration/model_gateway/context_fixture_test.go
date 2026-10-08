//go:build linux && integration

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package model_gateway_test

import (
	"context"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/budget"
	aicontext "github.com/zyc14588/TRPG_PLATFORM/internal/ai/context"
	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/model"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/capability"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/launch"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/command"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/postgres"
)

type policyProvider struct {
	mu           sync.RWMutex
	host, grants bool
	native       bool
}

func (p *policyProvider) Current(_ context.Context, _ core.Transaction, subject aicontext.Subject) (aicontext.Policy, error) {
	p.mu.RLock()
	defer p.mu.RUnlock()
	v := subject.StorageValue()
	declaration, e := capability.NewDeclaration(nil, []capability.OptionalSpec{{Name: "host.rules", Fallback: "pause"}, {Name: "host.state", Fallback: "pause"}})
	if e != nil {
		return aicontext.Policy{}, auth.ErrDenied
	}
	trust, e := capability.NewTrustPolicy(map[capability.TrustLevel][]string{capability.TrustOfficial: {"host.rules", "host.state"}, capability.TrustSigned: {}, capability.TrustPrivateUnverified: {}, capability.TrustDevelopment: {}})
	if e != nil {
		return aicontext.Policy{}, auth.ErrDenied
	}
	var granted []string
	if p.grants {
		granted = []string{"host.rules", "host.state"}
	}
	execution, e := capability.NewGrantSet(granted)
	if e != nil {
		return aicontext.Policy{}, auth.ErrDenied
	}
	role := "player"
	if p.host {
		role = "host"
	}
	views := command.ViewPolicy{ViewFields: []string{"public", "nested"}, EventFields: map[string][]string{"visible": {"public"}}}
	if p.native {
		views = command.ViewPolicy{ViewFields: []string{"counter"}, EventFields: map[string][]string{loopPackageID + "/change": {"counter"}}}
	}
	return aicontext.Policy{Binding: v.Binding, SeatID: v.SeatID, Tuple: v.Tuple, Role: role, GeneratorVersion: "memory-v1", Views: views, Declaration: declaration, TrustLevel: capability.TrustOfficial, Trust: trust, Execution: execution, Tools: []aicontext.Tool{{ID: "read-rules", Capability: capability.HostRules, Mode: "read"}, {ID: "propose-action", Capability: capability.HostState, Mode: "propose"}}}, nil
}

type isolated struct {
	*fixture
	contexts *aicontext.Service
	budgets  *budget.Service
	bs       *postgres.PlatformBudgetStorage
	policy   *policyProvider
	binding  data.Binding
	caps     budget.Caps
}

func allCaps() budget.Caps {
	v := budget.Units{Calls: 8, Tokens: 20000, CostMicros: 40000, LatencyMillis: 8000, Tools: 16, Subagents: 4, ContextBytes: 65536, LocalComputeMillis: 8000}
	return budget.Caps{Workspace: v, Room: v, Session: v, Seat: v, Task: v}
}
func newIsolated(t *testing.T, change func(*budget.Caps), fault func(context.Context, string) error) *isolated {
	t.Helper()
	f := newFixture(t, fault)
	f.prep.Slots = append(f.prep.Slots, launch.Slot{ID: "ai-other", Mode: "ai", ModelSelection: "selected"})
	f.prep.Revision++
	f.ack.Revision = f.prep.Revision
	f.inspect(t, f.owner, func(tx auth.Transaction) error {
		lt, e := f.lt.Bind(tx.Core())
		if e != nil {
			return e
		}
		if e = lt.PutPreparation(f.ctx, auth.RoomSecret(f.prep)); e != nil {
			return e
		}
		return lt.PutAcknowledgment(f.ctx, auth.RoomSecret(f.ack))
	})
	f.configured(t)
	need(t, f.store(t, f.owner, "ai-other", "byok-other", "retained", time.Time{}))
	_, e := f.models.Configure(f.ctx, f.caller(f.owner, token(t)), model.NewConfigureRequest(model.ConfigureRequestData{Scope: f.scope, SeatID: "ai-other", Selection: "selected", CredentialID: "byok-other", Budget: limits()}))
	need(t, e)
	b := data.Binding{Workspace: f.scope.WorkspaceID, Session: unique("session"), GraphHash: f.prep.GraphHash}
	state := checkpoint.Value{Kind: "table", Table: map[string]checkpoint.Value{"public": {Kind: "string", String: "visible-state"}, "hidden": {Kind: "string", String: "synthetic-human-private-state"}, "nested": {Kind: "table", Table: map[string]checkpoint.Value{"secret": {Kind: "string", String: "synthetic-nested-private-state"}}}}}
	raw, _ := json.Marshal(state)
	sqlCapture(t, fmt.Sprintf("INSERT INTO host_command.sessions(workspace,session,graph_hash,version,state,schema_hash,event_sequence) VALUES('%s','%s','%s',1,convert_from(decode('%s','hex'),'UTF8')::jsonb,'%s',2)", b.Workspace, b.Session, b.GraphHash, hex.EncodeToString(raw), checkpoint.Hash([]byte("synthetic-state-schema"))))
	f.inspect(t, f.owner, func(tx auth.Transaction) error {
		lt, e := f.lt.Bind(tx.Core())
		if e != nil {
			return e
		}
		return lt.PutSession(f.ctx, auth.RoomSecret(launch.SessionData{Scope: f.scope, Binding: b, ConfigurationID: f.prep.ConfigurationID, ConfigurationHash: f.prep.ConfigurationHash, Revision: f.prep.Revision}))
	})
	for i, kind := range []string{"visible", "hidden"} {
		payload := checkpoint.Value{Kind: "table", Table: map[string]checkpoint.Value{"public": {Kind: "string", String: "visible-event"}, "hidden": {Kind: "string", String: "synthetic-other-human-event"}}}
		raw, _ := json.Marshal(payload)
		id := fmt.Sprintf("command-%d", i+1)
		sqlCapture(t, fmt.Sprintf("INSERT INTO host_command.requests(workspace,session,command_id,principal,fingerprint,receipt) VALUES('%s','%s','%s','fixture','fixture',decode('7b7d','hex')); INSERT INTO host_command.events(workspace,session,sequence,version,command_id,event_id,event_type,payload,schema_version,schema_hash) VALUES('%s','%s',%d,1,'%s','event-%d','%s',decode('%s','hex'),1,'%s')", b.Workspace, b.Session, id, b.Workspace, b.Session, i+1, id, i+1, kind, hex.EncodeToString(raw), checkpoint.Hash([]byte("synthetic-event-schema"))))
	}
	cs, e := postgres.NewPlatformAIContextStorage(f.r)
	need(t, e)
	bs, e := postgres.NewPlatformBudgetStorage(f.r)
	need(t, e)
	policy := &policyProvider{grants: true}
	ai, e := aicontext.New(aicontext.Options{Authority: f.authority, Rooms: f.rt, Launches: f.lt, Models: f.models, ModelStorage: f.mt, Storage: cs, Policies: policy})
	need(t, e)
	caps := allCaps()
	if change != nil {
		change(&caps)
	}
	broker, e := budget.New(budget.Options{Authority: f.authority, Contexts: ai, Storage: bs, WorkspaceCaps: map[string]budget.Caps{f.scope.WorkspaceID: caps}})
	need(t, e)
	return &isolated{fixture: f, contexts: ai, budgets: broker, bs: bs, policy: policy, binding: b, caps: caps}
}
func (f *isolated) target(seat string) aicontext.Target {
	return auth.RoomSecret(aicontext.TargetData{Scope: f.scope, SeatID: seat})
}
func amount() budget.Units {
	return budget.Units{Calls: 1, Tokens: 512, CostMicros: 1000, LatencyMillis: 300, Tools: 2, Subagents: 1, ContextBytes: 4096, LocalComputeMillis: 300}
}
func (f *isolated) task(t *testing.T) budget.Task {
	t.Helper()
	v, e := f.budgets.Start(f.ctx, f.caller(f.owner, token(t)), f.target("ai"), false)
	need(t, e)
	return v
}
func (f *isolated) counter(t *testing.T, level string) (budget.Units, budget.Units) {
	t.Helper()
	raw := sqlCapture(t, fmt.Sprintf("SELECT encode(used,'hex')||'|'||encode(held,'hex') FROM platform_budget.counters WHERE workspace_id='%s' AND level='%s' ORDER BY node_id LIMIT 1", f.scope.WorkspaceID, level))
	parts := strings.Split(strings.TrimSpace(string(raw)), "|")
	if len(parts) != 2 {
		t.Fatal("counter read unavailable")
	}
	var used, held budget.Units
	u, e := hex.DecodeString(parts[0])
	need(t, e)
	h, e := hex.DecodeString(parts[1])
	need(t, e)
	if json.Unmarshal(u, &used) != nil || json.Unmarshal(h, &held) != nil {
		t.Fatal("counter decode failed")
	}
	return used, held
}
