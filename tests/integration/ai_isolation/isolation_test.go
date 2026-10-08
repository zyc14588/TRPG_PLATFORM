//go:build linux && integration

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package ai_isolation_test

import (
	"bytes"
	"context"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"strings"
	"sync"
	"sync/atomic"
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
	return aicontext.Policy{Binding: v.Binding, SeatID: v.SeatID, Tuple: v.Tuple, Role: role, GeneratorVersion: "memory-v1", Views: command.ViewPolicy{ViewFields: []string{"public", "nested"}, EventFields: map[string][]string{"visible": {"public"}}}, Declaration: declaration, TrustLevel: capability.TrustOfficial, Trust: trust, Execution: execution, Tools: []aicontext.Tool{{ID: "read-rules", Capability: capability.HostRules, Mode: "read"}, {ID: "propose-action", Capability: capability.HostState, Mode: "propose"}}}, nil
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
	v := budget.Units{Calls: 4, Tokens: 20000, CostMicros: 40000, LatencyMillis: 4000, Tools: 16, Subagents: 4, ContextBytes: 16384, LocalComputeMillis: 4000}
	return budget.Caps{Workspace: v, Room: v, Session: v, Seat: v, Task: v}
}
func newIsolated(t *testing.T, change func(*budget.Caps), fault func(context.Context, string) error) *isolated {
	t.Helper()
	f := newFixture(t, fault)
	f.prep.Slots = append(f.prep.Slots, launch.Slot{ID: "ai-other", Mode: "ai", ModelSelection: "selected"})
	f.inspect(t, f.owner, func(tx auth.Transaction) error {
		lt, e := f.lt.Bind(tx.Core())
		if e != nil {
			return e
		}
		return lt.PutPreparation(f.ctx, auth.RoomSecret(f.prep))
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

type providerFunc func(context.Context, aicontext.Prompt, budget.Units) (budget.Response, error)

func (f providerFunc) Call(ctx context.Context, p aicontext.Prompt, u budget.Units) (budget.Response, error) {
	return f(ctx, p, u)
}
func goodProvider(_ context.Context, p aicontext.Prompt, _ budget.Units) (budget.Response, error) {
	return auth.RoomSecret(budget.ResponseData{Advice: checkpoint.Value{Kind: "string", String: "synthetic-advice-only"}, Usage: budget.Units{Calls: 1, Tokens: 10, ContextBytes: uint64(p.Bytes())}, Determinate: true}), nil
}

func TestContextFiltersStateEventsPrivateMemoryAndToolsBeforeProvider(t *testing.T) {
	f := newIsolated(t, nil, nil)
	for _, seat := range []string{"ai", "ai-other"} {
		text := "synthetic-own-seat-memory"
		if seat == "ai-other" {
			text = "synthetic-other-seat-memory"
		}
		need(t, f.contexts.Remember(f.ctx, f.caller(f.owner, token(t)), f.target(seat), auth.RoomSecret(aicontext.MemoryData{ID: "same-note", Text: text, FactLevel: "summary", GeneratorVersion: "memory-v1", Sources: []uint64{1}})))
	}
	p, e := f.contexts.Build(f.ctx, f.caller(f.owner, ""), f.target("ai"))
	need(t, e)
	var raw []byte
	need(t, p.Use(func(b []byte) error { raw = append([]byte(nil), b...); return nil }))
	defer clear(raw)
	for _, marker := range []string{"synthetic-human-private-state", "synthetic-nested-private-state", "synthetic-other-human-event", "synthetic-other-seat-memory", "owned-fixture-private-provider-key-329874"} {
		if bytes.Contains(raw, []byte(marker)) {
			t.Fatal("forbidden source reached provider boundary")
		}
	}
	if !bytes.Contains(raw, []byte("synthetic-own-seat-memory")) || !bytes.Contains(raw, []byte("visible-state")) {
		t.Fatal("allowed context lost")
	}
	var payload aicontext.PayloadData
	need(t, json.Unmarshal(raw, &payload))
	if len(payload.Events) != 1 || len(payload.Tools) != 2 || len(payload.Memories) != 1 || payload.SeatID != "ai" {
		t.Fatal("context isolation identity mismatch")
	}
	f.policy.mu.Lock()
	f.policy.grants = false
	f.policy.mu.Unlock()
	p, e = f.contexts.Build(f.ctx, f.caller(f.owner, ""), f.target("ai"))
	need(t, e)
	need(t, p.Use(func(b []byte) error {
		var x aicontext.PayloadData
		if json.Unmarshal(b, &x) != nil || len(x.Tools) != 0 {
			return auth.ErrDenied
		}
		return nil
	}))
}
func TestCrossTenantHumanSeatAndUnadmittedAccountAreDenied(t *testing.T) {
	f := newIsolated(t, nil, nil)
	other := newIsolated(t, nil, nil)
	_, e := f.contexts.Build(f.ctx, f.caller(f.owner, ""), other.target("ai"))
	want(t, e, auth.ErrDenied)
	_, e = f.contexts.Build(f.ctx, f.caller(f.owner, ""), f.target("human"))
	want(t, e, auth.ErrDenied)
	outsider := f.account(t)
	_, e = f.contexts.Build(f.ctx, f.caller(outsider, ""), f.target("ai"))
	want(t, e, auth.ErrDenied)
	_, e = f.budgets.Start(f.ctx, f.caller(f.owner, token(t)), f.target("ai"), true)
	want(t, e, auth.ErrDenied)
}
func TestMemoryCannotUseInvisibleEventsOrBecomeWorldFact(t *testing.T) {
	f := newIsolated(t, nil, nil)
	for _, m := range []aicontext.MemoryData{{ID: "bad-source", Text: "synthetic-note", FactLevel: "summary", GeneratorVersion: "memory-v1", Sources: []uint64{2}}, {ID: "bad-level", Text: "synthetic-note", FactLevel: "world-fact", GeneratorVersion: "memory-v1", Sources: []uint64{1}}} {
		want(t, f.contexts.Remember(f.ctx, f.caller(f.owner, token(t)), f.target("ai"), auth.RoomSecret(m)), auth.ErrDenied)
	}
}
func TestAllFiveBudgetLevelsHaveDurableHardStops(t *testing.T) {
	for _, level := range []string{"workspace", "room", "session", "seat", "task"} {
		t.Run(level, func(t *testing.T) {
			f := newIsolated(t, func(c *budget.Caps) {
				switch level {
				case "workspace":
					c.Workspace.Calls = 0
				case "room":
					c.Room.Calls = 0
				case "session":
					c.Session.Calls = 0
				case "seat":
					c.Seat.Calls = 0
				case "task":
					c.Task.Calls = 0
				}
			}, nil)
			ticket, e := f.budgets.Reserve(f.ctx, f.caller(f.owner, token(t)), f.task(t), amount())
			if e != budget.ErrPaused || ticket.Status() != "paused" {
				t.Fatal("hierarchical zero limit did not pause")
			}
			used, held := f.counter(t, level)
			if used.Calls != 0 || held.Calls != 0 {
				t.Fatal("rejected reservation consumed or created credit")
			}
			var paused bool
			f.inspect(t, f.owner, func(tx auth.Transaction) error {
				st, e := f.bs.Bind(tx.Core())
				if e != nil {
					return e
				}
				p, e := f.contexts.BuildWithin(f.ctx, tx.Core(), f.target("ai"))
				if e != nil {
					return e
				}
				paused, e = st.Paused(f.ctx, p.Subject().StorageValue())
				return e
			})
			if !paused {
				t.Fatal("hard stop not persisted")
			}
		})
	}
}
func TestConcurrentReservationsAndDuplicateReplayRespectWorkspaceCap(t *testing.T) {
	f := newIsolated(t, func(c *budget.Caps) { c.Workspace.Calls = 3 }, nil)
	tasks := make([]budget.Task, 8)
	for i := range tasks {
		tasks[i] = f.task(t)
	}
	var wg sync.WaitGroup
	var allowed, paused atomic.Int32
	var wrong atomic.Bool
	for _, task := range tasks {
		wg.Add(1)
		go func(task budget.Task) {
			defer wg.Done()
			_, e := f.budgets.Reserve(f.ctx, f.caller(f.owner, token(t)), task, amount())
			if e == nil {
				allowed.Add(1)
			} else if e == budget.ErrPaused {
				paused.Add(1)
			} else {
				wrong.Store(true)
			}
		}(task)
	}
	wg.Wait()
	if wrong.Load() || allowed.Load() != 3 || paused.Load() != 5 {
		t.Fatal("concurrent reservations exceeded or lost hard cap")
	}
	used, held := f.counter(t, "workspace")
	if used.Calls != 0 || held.Calls != 3 {
		t.Fatal("concurrent durable accounting mismatch")
	}
	g := newIsolated(t, nil, nil)
	task := g.task(t)
	key := token(t)
	caller := g.caller(g.owner, key)
	first, e := g.budgets.Reserve(g.ctx, caller, task, amount())
	need(t, e)
	again, e := g.budgets.Reserve(g.ctx, caller, task, amount())
	need(t, e)
	if first.Status() != again.Status() {
		t.Fatal("duplicate reservation changed result")
	}
	_, held = g.counter(t, "workspace")
	if held.Calls != 1 {
		t.Fatal("duplicate reservation charged twice")
	}
}
func TestKnownDispatchSettlesOnceWithoutWritingGameState(t *testing.T) {
	f := newIsolated(t, nil, nil)
	before := sqlCapture(t, fmt.Sprintf("SELECT state::text||'|'||version::text||'|'||event_sequence::text FROM host_command.sessions WHERE workspace='%s' AND session='%s'", f.binding.Workspace, f.binding.Session))
	ticket, e := f.budgets.Reserve(f.ctx, f.caller(f.owner, token(t)), f.task(t), amount())
	need(t, e)
	var calls atomic.Int32
	provider := providerFunc(func(c context.Context, p aicontext.Prompt, u budget.Units) (budget.Response, error) {
		calls.Add(1)
		return goodProvider(c, p, u)
	})
	_, e = f.budgets.Run(f.ctx, f.caller(f.owner, token(t)), ticket, provider)
	need(t, e)
	_, e = f.budgets.Run(f.ctx, f.caller(f.owner, token(t)), ticket, provider)
	if e == nil || calls.Load() != 1 {
		t.Fatal("duplicate dispatch repeated provider work")
	}
	used, held := f.counter(t, "workspace")
	if used.Calls != 1 || held.Calls != 0 || used.Tokens != 10 {
		t.Fatal("known settlement failed to retain actual cost")
	}
	after := sqlCapture(t, fmt.Sprintf("SELECT state::text||'|'||version::text||'|'||event_sequence::text FROM host_command.sessions WHERE workspace='%s' AND session='%s'", f.binding.Workspace, f.binding.Session))
	if !bytes.Equal(before, after) {
		t.Fatal("advice mutated authoritative game state")
	}
	clear(before)
	clear(after)
}
func TestTimeoutUncertainBillingAndExcessUsageRetainEveryHeldDimension(t *testing.T) {
	for _, kind := range []string{"timeout", "ambiguous", "excess"} {
		t.Run(kind, func(t *testing.T) {
			f := newIsolated(t, nil, nil)
			u := amount()
			if kind == "timeout" {
				u.LatencyMillis = 10
				u.LocalComputeMillis = 10
			}
			ticket, e := f.budgets.Reserve(f.ctx, f.caller(f.owner, token(t)), f.task(t), u)
			need(t, e)
			p := providerFunc(func(c context.Context, p aicontext.Prompt, max budget.Units) (budget.Response, error) {
				if kind == "timeout" {
					<-c.Done()
					return budget.Response{}, errors.New("synthetic-private-provider-error")
				}
				v, e := goodProvider(c, p, max)
				x := v.StorageValue()
				if kind == "ambiguous" {
					x.Determinate = false
				}
				if kind == "excess" {
					x.Usage.Tokens = max.Tokens + 1
				}
				return auth.RoomSecret(x), e
			})
			_, e = f.budgets.Run(f.ctx, f.caller(f.owner, token(t)), ticket, p)
			if e != budget.ErrPaused {
				t.Fatal("uncertain external consumption forged success")
			}
			used, held := f.counter(t, "workspace")
			if used != (budget.Units{}) || held != u {
				t.Fatal("unknown consumption released reserved resources")
			}
		})
	}
}
func TestPrivateHandlesAreRedactedInMalformedDiagnosticsAndOrdinaryJSON(t *testing.T) {
	f := newIsolated(t, nil, nil)
	task := f.task(t)
	ticket, e := f.budgets.Reserve(f.ctx, f.caller(f.owner, token(t)), task, amount())
	need(t, e)
	p, e := f.contexts.Build(f.ctx, f.caller(f.owner, ""), f.target("ai"))
	need(t, e)
	for _, v := range []any{task, &task, ticket, &ticket, p, &p, f.contexts, *f.contexts, f.budgets, *f.budgets, f.bs, *f.bs} {
		for _, format := range []string{"%v", "%+v", "%#v", "%s", "%f", "%x", "%[0]v", "%*v"} {
			text := fmt.Sprintf(format, v)
			for _, marker := range []string{f.scope.WorkspaceID, f.binding.Session, "synthetic-human-private-state", "owned-fixture-private-provider-key-329874"} {
				if strings.Contains(text, marker) {
					t.Fatal("ordinary diagnostics exposed private data")
				}
			}
		}
		if _, e = json.Marshal(v); e == nil {
			t.Fatal("ordinary JSON exposed protected handle")
		}
	}
}
func TestCurrentRevocationAndStaleStatePreventDispatch(t *testing.T) {
	for _, kind := range []string{"qualification", "version"} {
		t.Run(kind, func(t *testing.T) {
			f := newIsolated(t, nil, nil)
			ticket, e := f.budgets.Reserve(f.ctx, f.caller(f.owner, token(t)), f.task(t), amount())
			need(t, e)
			if kind == "qualification" {
				need(t, f.models.RevokeCertification(f.scope.WorkspaceID, "default-model"))
			} else {
				sqlCapture(t, fmt.Sprintf("UPDATE host_command.sessions SET version=version+1 WHERE workspace='%s' AND session='%s'", f.binding.Workspace, f.binding.Session))
			}
			var calls atomic.Int32
			_, e = f.budgets.Run(f.ctx, f.caller(f.owner, token(t)), ticket, providerFunc(func(c context.Context, p aicontext.Prompt, u budget.Units) (budget.Response, error) {
				calls.Add(1)
				return goodProvider(c, p, u)
			}))
			if e == nil || calls.Load() != 0 {
				t.Fatal("stale authority reached provider")
			}
			_, held := f.counter(t, "workspace")
			if held.Calls != 1 {
				t.Fatal("denied dispatch lost durable reservation")
			}
		})
	}
}
func TestReservationRollbackAndRestartDoNotCreateBudgetCredit(t *testing.T) {
	var armed atomic.Bool
	f := newIsolated(t, nil, func(_ context.Context, point string) error {
		if armed.Load() && point == "budget-after-reservation" {
			return errors.New("synthetic-private-storage-error")
		}
		return nil
	})
	task := f.task(t)
	key := token(t)
	armed.Store(true)
	_, e := f.budgets.Reserve(f.ctx, f.caller(f.owner, key), task, amount())
	want(t, e, auth.ErrUnavailable)
	armed.Store(false)
	_, e = f.budgets.Reserve(f.ctx, f.caller(f.owner, key), task, amount())
	need(t, e)
	_, held := f.counter(t, "workspace")
	if held.Calls != 1 {
		t.Fatal("rollback charged or freed resources")
	}
	// A new composition does not reset the physical counters. Old handles are
	// unforgeable and cannot be used by a different service instance.
	b, e := budget.New(budget.Options{Authority: f.authority, Contexts: f.contexts, Storage: f.bs, WorkspaceCaps: map[string]budget.Caps{f.scope.WorkspaceID: f.caps}})
	need(t, e)
	_, e = b.Reserve(f.ctx, f.caller(f.owner, token(t)), task, amount())
	want(t, e, auth.ErrDenied)
	newTask, e := b.Start(f.ctx, f.caller(f.owner, token(t)), f.target("ai"), false)
	need(t, e)
	_, e = b.Reserve(f.ctx, f.caller(f.owner, token(t)), newTask, amount())
	need(t, e)
	_, held = f.counter(t, "workspace")
	if held.Calls != 2 {
		t.Fatal("restart reset durable budget")
	}
}
