// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package main

import (
	"context"
	"crypto/rand"
	"fmt"
	"math/big"
	"net/http"
	"os"
	"strconv"
	"strings"
	"sync"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/budget"
	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/certification"
	aicontext "github.com/zyc14588/TRPG_PLATFORM/internal/ai/context"
	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/credential"
	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/gateway"
	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/model"
	"github.com/zyc14588/TRPG_PLATFORM/internal/deployment/m2"
	"github.com/zyc14588/TRPG_PLATFORM/internal/hostapi"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/capability"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/httpapi"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/launch"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/room"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/session"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/command"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/postgres"
	"github.com/zyc14588/TRPG_PLATFORM/internal/task"
)

type installedPolicies struct {
	plan     m2.OperatorPlan
	commands map[string]map[string]func(checkpoint.Value) error
	contexts map[string]installedContextAuthorization
}
type installedContextAuthorization struct {
	declaration capability.Declaration
	level       capability.TrustLevel
}

func newInstalledPolicies(ctx context.Context, p m2.OperatorPlan, packages *platformPackageComponents) (*installedPolicies, error) {
	r := &installedPolicies{plan: p, commands: map[string]map[string]func(checkpoint.Value) error{}, contexts: map[string]installedContextAuthorization{}}
	for _, g := range p.Games {
		raw, e := m2.ReadSecret(g.InstallCredentialFile, 256)
		if e != nil {
			return nil, e
		}
		pkg, e := packages.reader.Load(ctx, stringCredential(raw), g.Workspace, g.Root)
		clear(raw)
		if e != nil {
			return nil, e
		}
		validators := map[string]func(checkpoint.Value) error{}
		for name, ref := range g.Commands {
			schema, e := hostapi.BindSchema(pkg, ref.Path, ref.Digest, ref.Seed)
			if e != nil {
				return nil, e
			}
			validators[name] = func(value checkpoint.Value) error {
				e := schema.Validate(value)
				if e != nil {
					fmt.Fprintln(os.Stderr, "M2_INSTALLED_COMMAND_SCHEMA_DENIED")
				}
				return e
			}
		}
		if len(validators) < 1 || len(validators) > 32 {
			return nil, m2.ErrConfiguration
		}
		r.commands[g.Workspace+"/"+g.Configuration] = validators
		manifest, e := pkg.Manifest()
		if e != nil {
			return nil, e
		}
		level := capability.TrustDevelopment
		if evidence := g.Evidence[g.Root]; evidence.Publisher != nil && evidence.Certification != nil {
			level = capability.TrustSigned
		}
		r.contexts[g.Workspace+"/"+g.Configuration] = installedContextAuthorization{manifest.Package.Capabilities, level}
	}
	return r, nil
}
func (p *installedPolicies) find(workspace, configuration, hash string) (m2.GamePlan, error) {
	for _, g := range p.plan.Games {
		if g.Workspace == workspace && g.Configuration == configuration && g.ConfigurationHash == hash {
			return g, nil
		}
	}
	return m2.GamePlan{}, auth.ErrDenied
}
func (p *installedPolicies) Current(ctx context.Context, a launch.SessionAccess) (session.SeatPolicy, error) {
	v := a.StorageValue()
	g, e := p.find(v.Scope.WorkspaceID, v.ConfigurationID, v.ConfigurationHash)
	if e != nil || ctx.Err() != nil || g.GraphHash != v.Binding.GraphHash {
		fmt.Fprintln(os.Stderr, "M2_INSTALLED_SESSION_POLICY_BINDING_DENIED")
		return session.SeatPolicy{}, auth.ErrDenied
	}
	view, ok := g.Views[v.Seat]
	if !ok {
		fmt.Fprintln(os.Stderr, "M2_INSTALLED_SESSION_POLICY_SEAT_DENIED")
		return session.SeatPolicy{}, auth.ErrDenied
	}
	exports := map[session.ExportKind]command.ViewPolicy{session.Public: g.Views["public"], session.Personal: view}
	if v.Host {
		exports[session.Host] = view
	}
	if v.Administrator {
		exports[session.Administrator] = view
	}
	inputs := func(ctx context.Context, _ command.Envelope) (command.NativeInputs, error) {
		if ctx.Err() != nil {
			return command.NativeInputs{}, auth.ErrUnavailable
		}
		n, e := rand.Int(rand.Reader, big.NewInt(10))
		if e != nil {
			return command.NativeInputs{}, auth.ErrUnavailable
		}
		return command.NativeInputs{Time: time.Now().UnixMilli(), Random: []int64{n.Int64()}}, nil
	}
	return session.SeatPolicy{Commands: p.commands[g.Workspace+"/"+g.Configuration], View: view, Inputs: inputs, Exports: exports, RecoveryPoint: true}, nil
}

type installedContexts struct{ policies *installedPolicies }

func (p installedContexts) Current(ctx context.Context, _ core.Transaction, s aicontext.Subject) (aicontext.Policy, error) {
	v := s.StorageValue()
	g, e := p.policies.find(v.Scope.WorkspaceID, v.ConfigurationID, v.ConfigurationHash)
	if e != nil || g.GraphHash != v.Binding.GraphHash || ctx.Err() != nil {
		return aicontext.Policy{}, auth.ErrDenied
	}
	view, ok := g.ContextViews[v.SeatID]
	if !ok {
		return aicontext.Policy{}, auth.ErrDenied
	}
	authorization, ok := p.policies.contexts[g.Workspace+"/"+g.Configuration]
	host := p.policies.plan.InstallPolicy.Host
	if !ok || host == nil || authorization.declaration.Validate() != nil {
		return aicontext.Policy{}, auth.ErrDenied
	}
	t, e := capability.NewTrustPolicy(host.Trust)
	if e != nil {
		return aicontext.Policy{}, e
	}
	ex, e := capability.NewGrantSet(host.Execution)
	if e != nil {
		return aicontext.Policy{}, e
	}
	return aicontext.Policy{Binding: v.Binding, SeatID: v.SeatID, Tuple: v.Tuple, Role: "player", GeneratorVersion: "m2-installed-v1", Views: view, Declaration: authorization.declaration, TrustLevel: authorization.level, Trust: t, Execution: ex, Tools: []aicontext.Tool{{ID: "read-rules", Capability: capability.HostRules, Mode: "read"}, {ID: "propose-action", Capability: capability.HostState, Mode: "propose"}}}, nil
}

// browserCallers retains only currently authenticated browser credentials from
// this daemon's ordinary same-origin listener. A worker certificate never
// creates a player identity; gateway guards reauthorize every dispatch.
type browserCallers struct {
	mu        sync.Mutex
	authority *auth.RoomAuthority
	rooms     room.Storage
	callers   map[string]model.Caller
}

func (c *browserCallers) capture(r *http.Request) {
	cookie, e := r.Cookie(httpapi.SessionCookie)
	if e != nil || len(cookie.Value) > 4096 {
		return
	}
	credential := auth.BrowserCookie(cookie.Value)
	csrf := r.Header.Get("X-CSRF-Token")
	if csrf == "" || len(csrf) > 256 {
		return
	}
	path := strings.Split(strings.TrimPrefix(r.URL.Path, "/"), "/")
	if len(path) < 7 || path[0] != "api" || path[1] != "v1" || path[2] != "workspaces" || path[4] != "rooms" {
		return
	}
	_ = c.authority.Inspect(r.Context(), credential, csrf, false, func(ctx context.Context, tx auth.Transaction, v auth.SessionData) error {
		if v.Kind != "account" || c.rooms == nil {
			return auth.ErrDenied
		}
		rt, e := c.rooms.Bind(tx.Core())
		if e != nil {
			return auth.ErrDenied
		}
		rm, e := rt.Room(ctx, path[3], path[5])
		if e != nil {
			return auth.ErrDenied
		}
		scope := rm.StorageValue().Scope
		part, e := rt.AccountParticipant(ctx, scope, v.AccountID)
		if e != nil || !part.StorageValue().Active {
			return auth.ErrDenied
		}
		id := scope.WorkspaceID + "/" + scope.RoomID + "/" + scope.GameID + "/" + part.StorageValue().ID
		c.mu.Lock()
		defer c.mu.Unlock()
		if len(c.callers) >= 128 {
			if _, ok := c.callers[id]; !ok {
				return auth.ErrDenied
			}
		}
		c.callers[id] = auth.RoomSecret(model.CallerData{Credential: credential, CSRF: csrf, Network: r.RemoteAddr})
		return nil
	})
}
func (c *browserCallers) current(ctx context.Context, scope core.Scope, id string) (model.Caller, error) {
	c.mu.Lock()
	defer c.mu.Unlock()
	v, ok := c.callers[scope.WorkspaceID+"/"+scope.RoomID+"/"+scope.GameID+"/"+id]
	if !ok || ctx.Err() != nil {
		fmt.Fprintln(os.Stderr, "M2 current browser caller unavailable")
		return model.Caller{}, auth.ErrDenied
	}
	return v, nil
}

func (c *browserCallers) forExecution(ctx context.Context, scope core.Scope, principal string, repo *postgres.PlatformAuthRepository) (model.Caller, error) {
	x, ok := ctx.Value(m2ExecutionKey{}).(*m2Execution)
	if !ok || x == nil || c == nil || c.authority == nil || c.rooms == nil {
		return model.Caller{}, auth.ErrDenied
	}
	x.mu.Lock()
	claim, closed := x.claim, x.closed || x.callerParticipant != ""
	x.mu.Unlock()
	v, e := claim.job.StorageValue()
	if closed || e != nil || v.Scope != scope || v.OriginPrincipal != principal {
		return model.Caller{}, auth.ErrDenied
	}
	id, e := postgres.M2ProviderSourcePrincipal(ctx, repo, claim.worker, claim.job)
	if e != nil {
		return model.Caller{}, auth.ErrDenied
	}
	caller, e := c.current(ctx, scope, id)
	if e != nil {
		return model.Caller{}, auth.ErrDenied
	}
	value := caller.StorageValue()
	var account, hash string
	e = c.authority.Inspect(ctx, value.Credential, value.CSRF, false, func(ctx context.Context, tx auth.Transaction, session auth.SessionData) error {
		if session.Kind != "account" {
			return auth.ErrDenied
		}
		rt, e := c.rooms.Bind(tx.Core())
		if e != nil {
			return auth.ErrDenied
		}
		part, e := rt.AccountParticipant(ctx, scope, session.AccountID)
		if e != nil || !part.StorageValue().Active || part.StorageValue().ID != id || part.StorageValue().Scope != scope {
			return auth.ErrDenied
		}
		account, hash = session.AccountID, session.Hash
		return nil
	})
	if e != nil || account == "" || hash == "" {
		return model.Caller{}, auth.ErrDenied
	}
	x.mu.Lock()
	defer x.mu.Unlock()
	if x.closed || x.callerParticipant != "" {
		return model.Caller{}, auth.ErrDenied
	}
	x.callerParticipant, x.callerAccount, x.callerHash, x.callerRooms = id, account, hash, c.rooms
	return caller, nil
}

type m2GatewayComponents struct {
	adapter *gateway.Adapter
	runtime *task.Runtime
	callers *browserCallers
	worker  task.Worker
}

func openM2Gateway(ctx context.Context, c m2.Config, plan m2.OperatorPlan, rooms *platformRoomComponents, players *platformPlayerComponents, models *model.Service, vault *credential.Vault, policies *installedPolicies, broker *m2.Broker, certs []model.Certification) (*m2GatewayComponents, error) {
	bs, e := postgres.NewPlatformBudgetStorage(rooms.repo)
	if e != nil {
		return nil, e
	}
	if e = bs.Bootstrap(ctx); e != nil {
		return nil, e
	}
	ms, e := postgres.NewPlatformModelStorage(rooms.repo)
	if e != nil {
		return nil, e
	}
	cs, e := postgres.NewPlatformAIContextStorage(rooms.repo)
	if e != nil {
		return nil, e
	}
	contexts, e := aicontext.New(aicontext.Options{Authority: rooms.authority, Rooms: rooms.storage, Launches: players.state().launchStorage, Models: models, ModelStorage: ms, Storage: cs, Policies: installedContexts{policies}})
	if e != nil {
		return nil, e
	}
	observedBudgets := m2ObservedBudgets{Storage: bs}
	budgets, e := budget.New(budget.Options{Authority: rooms.authority, Contexts: contexts, Storage: observedBudgets, WorkspaceCaps: plan.Caps})
	if e != nil {
		return nil, e
	}
	registry, e := certification.New(certs)
	if e != nil {
		return nil, e
	}
	op := c.Provider.Adapter()
	op.Transport = broker
	adapter, e := gateway.NewAdapter(op)
	if e != nil {
		return nil, e
	}
	callers := &browserCallers{authority: rooms.authority, rooms: rooms.storage, callers: map[string]model.Caller{}}
	commands := func(_ context.Context, _ core.Transaction, s aicontext.Subject) (map[string]func(checkpoint.Value) error, error) {
		v := s.StorageValue()
		g, e := policies.find(v.Scope.WorkspaceID, v.ConfigurationID, v.ConfigurationHash)
		if e != nil {
			return nil, e
		}
		return policies.commands[g.Workspace+"/"+g.Configuration], nil
	}
	guard := func(ctx context.Context, tx core.Transaction, request gateway.RequestData) (func(context.Context, aicontext.SubjectData, budget.Units, uint64) error, error) {
		return m2LiveDispatchGuard(ctx, tx, request, plan.Caps)
	}
	service, e := gateway.New(gateway.Options{Authority: rooms.authority, Contexts: contexts, Budgets: budgets, BudgetStorage: observedBudgets, Models: models, ModelStorage: ms, Vault: vault, Registry: registry, Adapters: []*gateway.Adapter{adapter}, WorkspaceCaps: plan.Caps, Commands: commands, Amount: plan.Amount, MaxCalls: plan.MaxCalls, DispatchGuard: guard})
	if e != nil {
		adapter.Close()
		return nil, e
	}
	captures := &m2TaskCaptures{claims: map[string]m2ClaimCapture{}}
	var taskPolicies []task.Policy
	for _, g := range plan.Games {
		policy, e := service.Policy(gateway.PolicyOptions{GraphHash: g.GraphHash, ConfigurationHash: g.ConfigurationHash, PackageID: g.PackageID, Caller: func(ctx context.Context, scope core.Scope, principal string) (model.Caller, error) {
			return callers.forExecution(ctx, scope, principal, rooms.repo)
		}, ValidateInput: func(v checkpoint.Value) error {
			if v.Kind != "table" || len(v.Table) != 3 || v.Table["seat_id"].Kind != "string" || v.Table["selection"].Kind != "string" || (v.Table["mode"].String != "proposal" && v.Table["mode"].String != "narrative") {
				return task.ErrInvalid
			}
			return nil
		}})
		if e != nil {
			adapter.Close()
			return nil, e
		}
		execute := policy.Execute
		policy.Execute = func(ctx context.Context, value task.Value) (task.Value, error) {
			x, e := captures.take(value)
			if e != nil {
				return task.Value{}, task.ErrDenied
			}
			defer x.close()
			out, e := execute(context.WithValue(ctx, m2ExecutionKey{}, x), value)
			if e != nil {
				fmt.Fprintln(os.Stderr, "M2 guarded provider execution failed:", task.SafeError(e).Error())
			}
			return out, e
		}
		taskPolicies = append(taskPolicies, policy)
	}
	storage, continuations, wrapped, e := platformPlayerTasks(ctx, rooms.repo, players, taskPolicies, func(s task.Storage) (task.Storage, error) { return gateway.BindTasks(&m2ObservedTasks{s, captures}) })
	if e != nil {
		adapter.Close()
		return nil, e
	}
	raw, e := m2.ReadSecret(plan.TaskCredentialFile, 256)
	if e != nil {
		adapter.Close()
		return nil, e
	}
	credential, e := task.NewCredential(raw)
	clear(raw)
	if e != nil {
		adapter.Close()
		return nil, e
	}
	spaces := []string{}
	seen := map[string]bool{}
	for _, g := range plan.Games {
		if !seen[g.Workspace] {
			seen[g.Workspace] = true
			spaces = append(spaces, g.Workspace)
		}
	}
	authority, e := task.NewAuthority([]task.WorkerGrant{{ID: "m2-platform-orchestrator", Credential: credential, Workspaces: spaces, Expires: time.Now().Add(23 * time.Hour)}})
	if e != nil {
		adapter.Close()
		return nil, e
	}
	worker, e := authority.Authenticate(ctx, credential)
	if e != nil {
		adapter.Close()
		return nil, e
	}
	runtime, e := task.NewWorker(task.WorkerOptions{Identity: worker, Storage: storage, Policies: wrapped, Post: continuations.Post, MaxActive: 2, ExternalTimeout: 3 * time.Second, PostTimeout: 3 * time.Second, Poll: 50 * time.Millisecond})
	if e != nil {
		adapter.Close()
		return nil, e
	}
	return &m2GatewayComponents{adapter, runtime, callers, worker}, nil
}

// The file's presence grants nothing. Only the explicitly supplied startup
// flag invokes these two original typed operations, before opening a listener.
// Normal startup/restart has no action. These operations are not atomic: if
// Configure fails after StoreCredential, startup fails and its sealed current
// credential remains subject to the original owner's revocation rules.
type ownerModelAction struct {
	DeploymentID, Source                                  string
	ExpiresAt                                             time.Time
	Scope                                                 core.Scope
	SeatID, Selection, CredentialID                       string
	CredentialIdempotencyKey, ConfigurationIdempotencyKey string
	ExpectedVersion                                       uint64
	Budget                                                model.Limits
	CookieFile, CSRFFile, KeyFile                         string
}

func applyOwnerModelAction(ctx context.Context, c m2.Config, plan m2.OperatorPlan, models *model.Service, path string) error {
	raw, e := m2.ReadSecret(path, 16384)
	if e != nil {
		return e
	}
	defer clear(raw)
	var a ownerModelAction
	if checkpoint.StrictDecode(raw, &a, 16384) != nil || a.DeploymentID != c.DeploymentID || a.Source != c.Source || !a.ExpiresAt.After(time.Now()) || a.ExpiresAt.After(time.Now().Add(5*time.Minute)) {
		return m2.ErrConfiguration
	}
	allowed := false
	for _, g := range plan.Games {
		if g.Workspace == a.Scope.WorkspaceID && g.Game == a.Scope.GameID {
			for _, seat := range g.Seats {
				for _, mode := range seat.Modes {
					if seat.ID == a.SeatID && mode == "ai" {
						allowed = true
					}
				}
			}
		}
	}
	if !allowed || a.CredentialIdempotencyKey == "" || a.ConfigurationIdempotencyKey == "" || a.CredentialIdempotencyKey == a.ConfigurationIdempotencyKey {
		return m2.ErrConfiguration
	}
	cookie, e := m2.ReadSecret(a.CookieFile, 4096)
	if e != nil {
		return e
	}
	defer clear(cookie)
	csrf, e := m2.ReadSecret(a.CSRFFile, 256)
	if e != nil {
		return e
	}
	defer clear(csrf)
	keyBytes, e := m2.ReadSecret(a.KeyFile, 4096)
	if e != nil {
		return e
	}
	defer clear(keyBytes)
	key, e := credential.NewKey(keyBytes)
	if e != nil {
		return auth.SafeError(e)
	}
	defer key.Close()
	caller := func(id string) model.Caller {
		return auth.RoomSecret(model.CallerData{Credential: auth.BrowserCookie(string(cookie)), CSRF: string(csrf), IdempotencyKey: id, Network: "m2-explicit-owner-action"})
	}
	e = models.StoreCredential(ctx, caller(a.CredentialIdempotencyKey), auth.RoomSecret(model.CredentialRequestData{Scope: a.Scope, SeatID: a.SeatID, ID: a.CredentialID, Lifetime: credential.Retained, Key: key}))
	if e != nil {
		return auth.SafeError(e)
	}
	_, e = models.Configure(ctx, caller(a.ConfigurationIdempotencyKey), model.NewConfigureRequest(model.ConfigureRequestData{Scope: a.Scope, SeatID: a.SeatID, Selection: a.Selection, CredentialID: a.CredentialID, ExpectedVersion: a.ExpectedVersion, Budget: a.Budget}))
	return auth.SafeError(e)
}

// These private captures lend only original canonical Claim/Dispatch handles
// to the current in-process execution. They never cross the worker boundary.
type m2ExecutionKey struct{}
type m2ClaimCapture struct {
	job    task.Job
	worker task.Worker
}
type m2TaskCaptures struct {
	mu     sync.Mutex
	claims map[string]m2ClaimCapture
}
type m2ObservedTasks struct {
	task.Storage
	captures *m2TaskCaptures
}

func m2JobKey(v task.JobData) string {
	return v.Binding.Workspace + "/" + v.Binding.Session + "/" + v.TaskID
}
func (s *m2ObservedTasks) Claim(ctx context.Context, worker task.Worker) (task.Job, error) {
	j, e := s.Storage.Claim(ctx, worker)
	if e != nil {
		return task.Job{}, e
	}
	v, e := j.StorageValue()
	if e != nil {
		return task.Job{}, task.ErrDenied
	}
	if v.Status != task.Running {
		return j, nil
	}
	copy, e := task.NewJob(v)
	if e != nil {
		return task.Job{}, e
	}
	s.captures.mu.Lock()
	defer s.captures.mu.Unlock()
	key := m2JobKey(v)
	if len(s.captures.claims) >= 64 {
		return task.Job{}, task.ErrBusy
	}
	if _, exists := s.captures.claims[key]; exists {
		return task.Job{}, task.ErrDenied
	}
	s.captures.claims[key] = m2ClaimCapture{copy, worker}
	return j, nil
}
func (c *m2TaskCaptures) remove(j task.Job) {
	v, e := j.StorageValue()
	if e != nil {
		return
	}
	c.mu.Lock()
	defer c.mu.Unlock()
	key := m2JobKey(v)
	old, ok := c.claims[key]
	if !ok {
		return
	}
	x, e := old.job.StorageValue()
	if e == nil && x.Token.Digest() == v.Token.Digest() {
		delete(c.claims, key)
	}
}
func (s *m2ObservedTasks) SaveResult(ctx context.Context, w task.Worker, j task.Job, v task.Value, i task.Inputs) (task.Job, error) {
	defer s.captures.remove(j)
	return s.Storage.SaveResult(ctx, w, j, v, i)
}
func (s *m2ObservedTasks) Retry(ctx context.Context, w task.Worker, j task.Job) error {
	defer s.captures.remove(j)
	return s.Storage.Retry(ctx, w, j)
}
func (s *m2ObservedTasks) Complete(ctx context.Context, w task.Worker, j task.Job, r data.Receipt) error {
	defer s.captures.remove(j)
	return s.Storage.Complete(ctx, w, j, r)
}
func (s *m2ObservedTasks) Cancel(ctx context.Context, w task.Worker, b data.Binding, id string) error {
	c := s.captures
	c.mu.Lock()
	delete(c.claims, b.Workspace+"/"+b.Session+"/"+id)
	c.mu.Unlock()
	return s.Storage.Cancel(ctx, w, b, id)
}

type m2Execution struct {
	mu                                           sync.Mutex
	closed                                       bool
	claim                                        m2ClaimCapture
	reservation                                  *budget.ReservationData
	callerParticipant, callerAccount, callerHash string
	callerRooms                                  room.Storage
}

func (x *m2Execution) close() {
	x.mu.Lock()
	defer x.mu.Unlock()
	x.closed = true
	x.claim = m2ClaimCapture{}
	x.reservation = nil
	x.callerParticipant, x.callerAccount, x.callerHash, x.callerRooms = "", "", "", nil
}
func (c *m2TaskCaptures) take(value task.Value) (*m2Execution, error) {
	raw, e := value.StorageValue()
	if e != nil || raw.Kind != "table" || len(raw.Table) != 2 || raw.Table["server"].Kind != "table" {
		return nil, task.ErrDenied
	}
	m := raw.Table["server"].Table
	if len(m) != 11 {
		return nil, task.ErrDenied
	}
	key := m["workspace"].String + "/" + m["session"].String + "/" + m["task"].String
	c.mu.Lock()
	defer c.mu.Unlock()
	claim, ok := c.claims[key]
	if !ok {
		return nil, task.ErrDenied
	}
	v, e := claim.job.StorageValue()
	if e != nil {
		return nil, task.ErrDenied
	}
	for field, want := range map[string]string{"workspace": v.Scope.WorkspaceID, "room": v.Scope.RoomID, "game": v.Scope.GameID, "session": v.Binding.Session, "graph": v.Binding.GraphHash, "task": v.TaskID, "package": v.PackageID, "configuration": v.ConfigurationID, "configuration_hash": v.ConfigurationHash, "principal": v.OriginPrincipal} {
		if m[field].Kind != "string" || m[field].String != want {
			return nil, task.ErrDenied
		}
	}
	if m["version"].Kind != "integer" || m["version"].Number != strconv.FormatUint(v.OriginVersion, 10) {
		return nil, task.ErrDenied
	}
	if raw.Table["input"].Kind != "table" {
		return nil, task.ErrDenied
	}
	input, e := task.NewValue(raw.Table["input"])
	if e != nil || input.Digest() != v.Payload.Digest() {
		return nil, task.ErrDenied
	}
	delete(c.claims, key)
	return &m2Execution{claim: claim}, nil
}

type m2ObservedBudgets struct{ budget.Storage }
type m2ObservedBudgetTransaction struct{ budget.Transaction }

func (s m2ObservedBudgets) Bind(tx core.Transaction) (budget.Transaction, error) {
	b, e := s.Storage.Bind(tx)
	if e != nil {
		return nil, e
	}
	return m2ObservedBudgetTransaction{b}, nil
}
func (t m2ObservedBudgetTransaction) Dispatch(ctx context.Context, record budget.Reservation) error {
	x, ok := ctx.Value(m2ExecutionKey{}).(*m2Execution)
	if !ok || x == nil {
		return auth.ErrDenied
	}
	x.mu.Lock()
	defer x.mu.Unlock()
	v, e := x.claim.job.StorageValue()
	r := record.StorageValue()
	if x.closed || e != nil || x.reservation != nil || x.callerParticipant == "" || x.callerHash == "" || x.callerRooms == nil || r.Task.Subject.Scope != v.Scope || r.Task.Subject.Binding != v.Binding || r.Task.Subject.ConfigurationID != v.ConfigurationID || r.Task.Subject.ConfigurationHash != v.ConfigurationHash || r.Task.Subject.StateVersion != v.OriginVersion {
		return auth.ErrDenied
	}
	if e = t.Transaction.Dispatch(ctx, record); e != nil {
		return e
	}
	copy := r
	x.reservation = &copy
	return nil
}
func (t m2ObservedBudgetTransaction) Settle(ctx context.Context, record budget.Reservation, spent budget.Units, known bool) (budget.Reservation, error) {
	final, e := t.Transaction.Settle(ctx, record, spent, known)
	if e != nil || final.StorageValue().Status != "settled" {
		return final, e
	}
	if x, ok := ctx.Value(m2ExecutionKey{}).(*m2Execution); ok && x != nil {
		x.mu.Lock()
		if x.reservation != nil && x.reservation.Task.ID == record.StorageValue().Task.ID && x.reservation.RequestHash == record.StorageValue().RequestHash {
			x.reservation = nil
		}
		x.mu.Unlock()
	}
	return final, nil
}
func m2LiveDispatchGuard(ctx context.Context, tx core.Transaction, request gateway.RequestData, caps map[string]budget.Caps) (func(context.Context, aicontext.SubjectData, budget.Units, uint64) error, error) {
	x, ok := ctx.Value(m2ExecutionKey{}).(*m2Execution)
	if !ok || x == nil {
		return nil, auth.ErrDenied
	}
	x.mu.Lock()
	if x.closed || x.reservation == nil {
		x.mu.Unlock()
		return nil, auth.ErrDenied
	}
	claim, observed := x.claim, *x.reservation
	callerID, callerAccount, callerHash, rooms := x.callerParticipant, x.callerAccount, x.callerHash, x.callerRooms
	x.mu.Unlock()
	j, e := claim.job.StorageValue()
	if e != nil || request.Scope != j.Scope || request.Binding != j.Binding || request.TaskID != j.TaskID || request.PackageID != j.PackageID || request.ConfigurationID != j.ConfigurationID || request.ConfigurationHash != j.ConfigurationHash || request.OriginPrincipal != j.OriginPrincipal || request.OriginVersion != j.OriginVersion {
		return nil, auth.ErrDenied
	}
	if callerID == "" || callerAccount == "" || callerHash == "" || rooms == nil {
		return nil, auth.ErrDenied
	}
	id, e := postgres.ReadM2ProviderSourcePrincipal(ctx, tx, claim.worker, claim.job)
	if e != nil || id != callerID {
		return nil, auth.ErrDenied
	}
	session, e := auth.RoomAdmissionSession(tx)
	if e != nil || session.StorageValue().Kind != "account" || session.StorageValue().Hash != callerHash || session.StorageValue().AccountID != callerAccount {
		return nil, auth.ErrDenied
	}
	rt, e := rooms.Bind(tx)
	if e != nil {
		return nil, auth.ErrDenied
	}
	part, e := rt.AccountParticipant(ctx, request.Scope, callerAccount)
	if e != nil || !part.StorageValue().Active || part.StorageValue().ID != callerID || part.StorageValue().Scope != request.Scope {
		return nil, auth.ErrDenied
	}
	cap, ok := caps[request.Scope.WorkspaceID]
	if !ok {
		return nil, auth.ErrDenied
	}
	return func(ctx context.Context, subject aicontext.SubjectData, bound budget.Units, promptBytes uint64) error {
		x.mu.Lock()
		closed := x.closed || x.reservation == nil || x.reservation.Task.ID != observed.Task.ID || x.reservation.RequestHash != observed.RequestHash
		x.mu.Unlock()
		if closed {
			return auth.ErrDenied
		}
		return postgres.ReadM2ProviderReservation(ctx, tx, auth.RoomSecret(observed), subject, bound, promptBytes, cap)
	}, nil
}
