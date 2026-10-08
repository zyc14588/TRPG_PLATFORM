// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package gateway

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"slices"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/action"
	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/budget"
	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/certification"
	aicontext "github.com/zyc14588/TRPG_PLATFORM/internal/ai/context"
	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/credential"
	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/model"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	store "github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
)

type RequestData struct {
	Scope                                                                  core.Scope
	Binding                                                                data.Binding
	TaskID, PackageID, ConfigurationID, ConfigurationHash, OriginPrincipal string
	OriginVersion                                                          uint64
	SeatID, Selection, Mode                                                string
}
type Request = auth.Secret[RequestData]
type OutputData struct {
	Mode, Status string
	Action       *action.ProposalData
	Narrative    string
	Fallback     bool
	Usage        budget.Units
}
type Output = auth.Secret[OutputData]
type Commands func(context.Context, core.Transaction, aicontext.Subject) (map[string]func(checkpoint.Value) error, error)
type Options struct {
	Authority     *auth.RoomAuthority
	Contexts      *aicontext.Service
	Budgets       *budget.Service
	BudgetStorage budget.Storage
	Models        *model.Service
	ModelStorage  model.Storage
	Vault         *credential.Vault
	Registry      *certification.Registry
	Adapters      []*Adapter
	WorkspaceCaps map[string]budget.Caps
	Commands      Commands
	Amount        budget.Units
	MaxCalls      int
}
type Service struct{ data **serviceData }
type serviceData struct {
	options  Options
	adapters map[string]*Adapter
	caps     map[string]budget.Caps
	active   chan struct{}
}

func (Service) Format(f fmt.State, _ rune) {
	_, _ = io.WriteString(f, "<private server model gateway>")
}
func (Service) MarshalJSON() ([]byte, error) { return nil, auth.ErrDenied }
func (s *Service) state() *serviceData {
	if s == nil || s.data == nil {
		return nil
	}
	return *s.data
}
func New(o Options) (*Service, error) {
	if o.Authority == nil || o.Contexts == nil || o.Budgets == nil || o.BudgetStorage == nil || o.Models == nil || o.ModelStorage == nil || o.Vault == nil || o.Registry == nil || o.Commands == nil || len(o.Adapters) < 1 || len(o.Adapters) > 128 || len(o.WorkspaceCaps) < 1 || len(o.WorkspaceCaps) > 128 || !budget.Valid(o.Amount) || o.Amount.Calls != 1 || o.Amount.Tokens < 1 || o.Amount.LatencyMillis < 1 || o.Amount.LatencyMillis > 2000 || o.Amount.LocalComputeMillis < 1 || o.Amount.LocalComputeMillis > 2000 || o.MaxCalls < 1 || o.MaxCalls > 4 {
		return nil, auth.ErrInvalid
	}
	d := &serviceData{options: o, adapters: map[string]*Adapter{}, caps: map[string]budget.Caps{}, active: make(chan struct{}, 4)}
	for _, a := range o.Adapters {
		if a.state() == nil {
			return nil, auth.ErrInvalid
		}
		key := a.state().endpoint.URL + "\x00" + a.state().endpoint.Adapter
		if _, ok := d.adapters[key]; ok {
			return nil, auth.ErrInvalid
		}
		d.adapters[key] = a
	}
	for w, c := range o.WorkspaceCaps {
		if !store.ValidID(w) || !budget.ValidCaps(c) {
			return nil, auth.ErrInvalid
		}
		d.caps[w] = c
	}
	d.options.Adapters = nil
	d.options.WorkspaceCaps = nil
	return &Service{data: &d}, nil
}
func requestValid(r RequestData) bool {
	return store.ValidID(r.Scope.WorkspaceID) && store.ValidID(r.Scope.RoomID) && store.ValidID(r.Scope.GameID) && r.Binding.Workspace == r.Scope.WorkspaceID && store.ValidID(r.Binding.Session) && checkpoint.IsDigest(r.Binding.GraphHash) && store.ValidID(r.TaskID) && store.ValidID(r.ConfigurationID) && checkpoint.IsDigest(r.ConfigurationHash) && store.ValidID(r.OriginPrincipal) && r.OriginVersion > 0 && r.OriginVersion < 1<<53 && store.ValidID(r.SeatID) && store.ValidID(r.Selection) && (r.Mode == "proposal" || r.Mode == "narrative")
}
func target(r RequestData) aicontext.Target {
	return auth.RoomSecret(aicontext.TargetData{Scope: r.Scope, SeatID: r.SeatID})
}
func sameRequest(v aicontext.SubjectData, r RequestData) bool {
	return v.Scope == r.Scope && v.Binding == r.Binding && v.ConfigurationID == r.ConfigurationID && v.ConfigurationHash == r.ConfigurationHash && v.StateVersion == r.OriginVersion && v.SeatID == r.SeatID
}
func callerKey(c model.Caller, r RequestData, phase string) model.Caller {
	v := c.StorageValue()
	v.IdempotencyKey = "gateway-" + certification.Hash(struct {
		Request RequestData
		Phase   string
	}{r, phase})
	return auth.RoomSecret(v)
}
func payload(p aicontext.Prompt) (aicontext.PayloadData, error) {
	var v aicontext.PayloadData
	e := p.Use(func(b []byte) error {
		if json.Unmarshal(b, &v) != nil {
			return auth.ErrDenied
		}
		return nil
	})
	return v, e
}

type route struct {
	certificate model.CertificationData
	adapter     *Adapter
}

func (s *Service) routes(c model.ConfigurationData, p aicontext.PayloadData, now time.Time) ([]route, error) {
	caps := []string{"structured-actions", "ai-player"}
	level := 3
	if p.Role == "host" {
		caps = []string{"structured-actions", "ai-host"}
		level = 4
	} else if p.Role != "player" {
		return nil, auth.ErrDenied
	}
	bindings := append([]model.CertificateBinding{c.Primary}, slices.Clone(c.Fallbacks)...)
	routes := []route{}
	for _, b := range bindings {
		cert, e := s.state().options.Registry.Bound(c.Scope.WorkspaceID, b, c.GraphHash, caps, level, now)
		if e != nil {
			return nil, e
		}
		v := cert.StorageValue()
		a := s.state().adapters[v.Tuple.Endpoint+"\x00"+v.Tuple.Adapter]
		if a == nil || v.Tuple.PromptTemplate != c.Tuple.PromptTemplate || v.Tuple.ToolMode != c.Tuple.ToolMode || !slices.Contains(a.state().endpoint.Models, v.Tuple.Model) {
			return nil, auth.ErrDenied
		}
		if len(routes) > 0 && a.Price() > routes[0].adapter.Price() {
			continue
		}
		routes = append(routes, route{v, a})
	}
	if len(routes) < 1 || routes[0].certificate.Tuple != c.Tuple {
		return nil, auth.ErrDenied
	}
	return routes, nil
}

type provider struct {
	s             *Service
	caller        model.Caller
	request       RequestData
	configuration model.ConfigurationData
	route         route
	instruction   string
}

func (p provider) Call(ctx context.Context, prompt aicontext.Prompt, cap budget.Units) (budget.Response, error) {
	var key credential.Key
	defer func() { key.Close() }()
	c := p.caller.StorageValue()
	o := p.s.state().options
	e := o.Authority.Inspect(ctx, c.Credential, c.CSRF, false, func(ctx context.Context, tx auth.Transaction, _ auth.SessionData) error {
		current, e := o.Contexts.BuildWithin(ctx, tx.Core(), target(p.request))
		if e != nil {
			return e
		}
		if current.Subject().StorageValue() != prompt.Subject().StorageValue() || !sameRequest(current.Subject().StorageValue(), p.request) {
			return auth.ErrDenied
		}
		var a, b []byte
		defer func() { clear(a); clear(b) }()
		if current.Use(func(v []byte) error { a = append([]byte(nil), v...); return nil }) != nil || prompt.Use(func(v []byte) error { b = append([]byte(nil), v...); return nil }) != nil || !slices.Equal(a, b) {
			return auth.ErrDenied
		}
		mt, e := o.ModelStorage.Bind(tx.Core())
		if e != nil {
			return auth.SafeError(e)
		}
		stored, e := mt.Configuration(ctx, p.request.Scope, p.request.SeatID, p.request.Selection)
		if e != nil {
			return auth.SafeError(e)
		}
		config := stored.StorageValue()
		if certification.Hash(config) != certification.Hash(p.configuration) {
			return auth.ErrDenied
		}
		record, e := mt.Credential(ctx, config.Scope, config.SeatID, config.CredentialID)
		if e != nil {
			return auth.SafeError(e)
		}
		v := record.StorageValue()
		bnd := v.Binding
		if bnd.Scope != config.Scope || bnd.SeatID != config.SeatID || bnd.ID != config.CredentialID || bnd.OwnerKind != config.OwnerKind || bnd.OwnerID != config.OwnerID || bnd.Version != config.CredentialVersion {
			return auth.ErrDenied
		}
		now, e := tx.Core().Now(ctx)
		if e != nil {
			return auth.SafeError(e)
		}
		key, e = o.Vault.Open(ctx, record, bnd, now)
		return e
	})
	if e != nil {
		return budget.Response{}, auth.SafeError(e)
	}
	var answer Answer
	e = prompt.Use(func(raw []byte) error {
		var e error
		answer, e = p.route.adapter.Call(ctx, p.route.certificate.Tuple, key, raw, p.instruction, cap)
		return e
	})
	if e != nil {
		return budget.Response{}, auth.SafeError(e)
	}
	v := answer.StorageValue()
	if p.request.Mode == "proposal" {
		v.Usage.Tools = 1
	}
	return auth.RoomSecret(budget.ResponseData{Advice: checkpoint.Text(v.Text), Usage: v.Usage, Determinate: true}), nil
}

// Pause exhausts a NEW, zero-grant pipeline task without dispatching a
// provider. Shared counter caps are the original configured caps; only the
// new task cap is zero. No usage, hold or shared allowance is fabricated.
func (s *Service) Pause(ctx context.Context, caller model.Caller, request Request) error {
	if s.state() == nil || ctx == nil || !requestValid(request.StorageValue()) {
		return auth.ErrDenied
	}
	r := request.StorageValue()
	c := callerKey(caller, r, "pause").StorageValue()
	canonical, _ := json.Marshal(r)
	_, e := s.state().options.Authority.Do(ctx, c.Credential, c.CSRF, c.IdempotencyKey, c.Network, auth.RoomSecret(auth.RoomCommandData{Action: "ai.gateway-pause", TargetKey: r.Scope.WorkspaceID + "/" + r.Scope.RoomID + "/" + r.Scope.GameID + "/" + r.SeatID, Canonical: canonical, Write: true}), auth.RoomCallbacks{Apply: func(ctx context.Context, tx auth.Transaction, _ auth.SessionData) (auth.Outcome, error) {
		p, e := s.state().options.Contexts.BuildWithin(ctx, tx.Core(), target(r))
		if e != nil {
			return auth.Outcome{}, e
		}
		v := p.Subject().StorageValue()
		if !sameRequest(v, r) {
			return auth.Outcome{}, auth.ErrDenied
		}
		caps, ok := s.state().caps[v.Scope.WorkspaceID]
		if !ok {
			return auth.Outcome{}, auth.ErrDenied
		}
		caps.Task = budget.Units{}
		st, e := s.state().options.BudgetStorage.Bind(tx.Core())
		if e != nil {
			return auth.Outcome{}, auth.SafeError(e)
		}
		id := certification.Hash(struct {
			Request RequestData
			Kind    string
		}{r, "pause"})[:32]
		record := auth.RoomSecret(budget.TaskData{ID: id, Subject: v, State: "open"})
		if e = st.InsertTask(ctx, record); e != nil {
			return auth.Outcome{}, auth.SafeError(e)
		}
		x, e := st.Reserve(ctx, auth.RoomSecret(budget.ReservationData{Task: record.StorageValue(), Amount: budget.Units{Calls: 1}, RequestHash: certification.Hash(r), Status: "reserved"}), caps)
		if e != nil {
			return auth.Outcome{}, auth.SafeError(e)
		}
		if x.StorageValue().Status != "paused" {
			return auth.Outcome{}, auth.ErrDenied
		}
		return auth.RoomOutcome([]byte(`{"status":"paused"}`)), nil
	}, Replay: func(ctx context.Context, tx auth.Transaction, _ auth.SessionData, _ auth.Outcome) error {
		p, e := s.state().options.Contexts.BuildWithin(ctx, tx.Core(), target(r))
		if e != nil {
			return e
		}
		if !sameRequest(p.Subject().StorageValue(), r) {
			return auth.ErrDenied
		}
		st, e := s.state().options.BudgetStorage.Bind(tx.Core())
		if e != nil {
			return auth.SafeError(e)
		}
		paused, e := st.Paused(ctx, p.Subject().StorageValue())
		if e != nil || !paused {
			return auth.ErrDenied
		}
		return nil
	}})
	return auth.SafeError(e)
}

func (s *Service) Execute(ctx context.Context, caller model.Caller, request Request) (output Output, err error) {
	defer func() {
		if recover() != nil {
			output = Output{}
			err = auth.ErrUnavailable
		}
	}()
	if s.state() == nil || ctx == nil || ctx.Err() != nil || !requestValid(request.StorageValue()) {
		return Output{}, auth.ErrDenied
	}
	r := request.StorageValue()
	o := s.state().options
	select {
	case s.state().active <- struct{}{}:
	default:
		return Output{}, auth.ErrUnavailable
	}
	defer func() { <-s.state().active }()
	p, e := o.Contexts.Build(ctx, caller, target(r))
	if e != nil {
		return Output{}, auth.SafeError(e)
	}
	subject := p.Subject().StorageValue()
	if !sameRequest(subject, r) {
		return Output{}, auth.ErrDenied
	}
	view, e := payload(p)
	if e != nil {
		return Output{}, e
	}
	configuration, e := o.Models.Read(ctx, caller, auth.RoomSecret(model.TargetData{Scope: r.Scope, SeatID: r.SeatID, ID: r.Selection}))
	if e != nil {
		return Output{}, auth.SafeError(e)
	}
	config := configuration.StorageValue()
	if config.Version != subject.ModelVersion || config.Tuple != subject.Tuple || !budget.Fits(o.Amount, config.Budget) {
		return Output{}, auth.ErrDenied
	}
	routes, e := s.routes(config, view, time.Now())
	if e != nil {
		return Output{}, e
	}
	commands := map[string]func(checkpoint.Value) error{}
	if r.Mode == "proposal" {
		canPropose := false
		for _, tool := range view.Tools {
			canPropose = canPropose || tool.Mode == "propose"
		}
		if !canPropose {
			return Output{}, auth.ErrDenied
		}
		c := caller.StorageValue()
		e = o.Authority.Inspect(ctx, c.Credential, c.CSRF, false, func(ctx context.Context, tx auth.Transaction, _ auth.SessionData) error {
			var e error
			commands, e = o.Commands(ctx, tx.Core(), p.Subject())
			return auth.SafeError(e)
		})
		if e != nil || len(commands) < 1 || len(commands) > 32 {
			return Output{}, auth.ErrDenied
		}
	}
	result := OutputData{Mode: r.Mode, Status: "paused"}
	routeIndex := 0
	for attempt := 0; attempt < o.MaxCalls; attempt++ {
		// One primary format retry, then only explicitly bound compatible
		// fallbacks at the same or lower server price. Every call reserves anew.
		if attempt >= 2 {
			routeIndex = attempt - 1
		}
		if routeIndex >= len(routes) {
			break
		}
		phase := fmt.Sprintf("%s-%d", r.Mode, attempt)
		task, e := o.Budgets.Start(ctx, callerKey(caller, r, phase+"-start"), target(r), false)
		if e != nil {
			return Output{}, auth.SafeError(e)
		}
		ticket, e := o.Budgets.Reserve(ctx, callerKey(caller, r, phase+"-reserve"), task, o.Amount)
		if e != nil {
			if e == budget.ErrPaused {
				return auth.RoomSecret(result), nil
			}
			return Output{}, auth.SafeError(e)
		}
		if ticket.Status() != "reserved" {
			if e = s.Pause(ctx, caller, request); e != nil {
				return Output{}, e
			}
			return auth.RoomSecret(result), nil
		}
		instruction := "Use only the supplied filtered context. Return a JSON object with type, expected_state_version and a checkpoint-encoded payload. Never claim authority or return narrative before commitment."
		if r.Mode == "narrative" {
			instruction = "Narrate only the committed facts in the supplied filtered context. Never add actions, hidden facts or uncommitted results."
		} else if attempt > 0 {
			instruction += " Return strict JSON without fences or additional fields."
		}
		value, e := o.Budgets.Run(ctx, caller, ticket, provider{s: s, caller: caller, request: r, configuration: config, route: routes[routeIndex], instruction: instruction})
		if e != nil {
			if e == budget.ErrPaused || e == auth.ErrOutcomeUnknown {
				return auth.RoomSecret(result), nil
			}
			return Output{}, auth.SafeError(e)
		}
		v := value.StorageValue()
		result.Usage, e = budget.Add(result.Usage, v.Usage)
		if e != nil || !budget.Fits(result.Usage, config.Budget) {
			return Output{}, auth.ErrDenied
		}
		if v.Advice.Kind != "string" {
			return Output{}, auth.ErrDenied
		}
		result.Fallback = result.Fallback || routeIndex > 0
		if r.Mode == "narrative" {
			if len(v.Advice.String) <= action.MaxNarrativeBytes {
				result.Status = "complete"
				result.Narrative = v.Advice.String
				return auth.RoomSecret(result), nil
			}
		} else {
			proposal, e := action.Decode([]byte(v.Advice.String), subject.StateVersion, commands)
			if e == nil {
				owned := proposal.StorageValue()
				result.Action = &owned
				result.Status = "complete"
				return auth.RoomSecret(result), nil
			}
		}
	}
	if e = s.Pause(ctx, caller, request); e != nil {
		return Output{}, e
	}
	return auth.RoomSecret(result), nil
}
