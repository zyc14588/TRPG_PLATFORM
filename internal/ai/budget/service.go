// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package budget

import (
	"context"
	"crypto/rand"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"io"
	"slices"
	"time"

	aicontext "github.com/zyc14588/TRPG_PLATFORM/internal/ai/context"
	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/model"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	store "github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
)

type Options struct {
	Authority     *auth.RoomAuthority
	Contexts      *aicontext.Service
	Storage       Storage
	WorkspaceCaps map[string]Caps
}
type Service struct{ data **serviceData }
type serviceData struct {
	options Options
	caps    map[string]Caps
	active  chan struct{}
}

func (Service) Format(f fmt.State, _ rune)   { _, _ = io.WriteString(f, "<private AI budget service>") }
func (Service) MarshalJSON() ([]byte, error) { return nil, auth.ErrDenied }
func (s *Service) state() *serviceData {
	if s == nil || s.data == nil {
		return nil
	}
	return *s.data
}
func New(o Options) (*Service, error) {
	if o.Authority == nil || o.Contexts == nil || o.Storage == nil || len(o.WorkspaceCaps) > 128 {
		return nil, auth.ErrInvalid
	}
	d := &serviceData{options: o, caps: map[string]Caps{}, active: make(chan struct{}, 32)}
	for w, c := range o.WorkspaceCaps {
		if !store.ValidID(w) || !ValidCaps(c) {
			return nil, auth.ErrInvalid
		}
		d.caps[w] = c
	}
	d.options.WorkspaceCaps = nil
	return &Service{data: &d}, nil
}
func target(v aicontext.SubjectData) aicontext.Target {
	return auth.RoomSecret(aicontext.TargetData{Scope: v.Scope, SeatID: v.SeatID})
}
func sameSubject(a, b aicontext.SubjectData) bool { return a == b }
func canonical(v any) ([]byte, error) {
	b, e := json.Marshal(v)
	if e != nil || len(b) > 16<<10 {
		return nil, auth.ErrInvalid
	}
	return b, nil
}
func receipt(id, status string) auth.Outcome {
	b, _ := json.Marshal(struct{ ID, Status string }{id, status})
	return auth.RoomOutcome(b)
}
func savedID(o auth.Outcome) (string, error) {
	var v struct{ ID, Status string }
	if checkpoint.StrictDecode(o.StorageValue().Body, &v, 512) != nil || !store.ValidID(v.ID) {
		return "", auth.ErrDenied
	}
	return v.ID, nil
}
func (s *Service) do(ctx context.Context, caller model.Caller, action string, t aicontext.TargetData, raw []byte, calls auth.RoomCallbacks) (auth.Outcome, error) {
	if s.state() == nil {
		return auth.Outcome{}, auth.ErrUnavailable
	}
	c := caller.StorageValue()
	return s.state().options.Authority.Do(ctx, c.Credential, c.CSRF, c.IdempotencyKey, c.Network, auth.RoomSecret(auth.RoomCommandData{Action: action, TargetKey: t.Scope.WorkspaceID + "/" + t.Scope.RoomID + "/" + t.Scope.GameID + "/" + t.SeatID, Canonical: raw, Write: true}), calls)
}
func (s *Service) Start(ctx context.Context, caller model.Caller, t aicontext.Target, advice bool) (Task, error) {
	raw, e := canonical(struct {
		Target aicontext.TargetData
		Advice bool
	}{t.StorageValue(), advice})
	if e != nil {
		return Task{}, e
	}
	var task Task
	work := func(ctx context.Context, tx auth.Transaction, saved *auth.Outcome) (auth.Outcome, error) {
		prompt, e := s.state().options.Contexts.BuildWithin(ctx, tx.Core(), t)
		if e != nil {
			return auth.Outcome{}, e
		}
		v := prompt.Subject().StorageValue()
		if _, ok := s.state().caps[v.Scope.WorkspaceID]; !ok {
			return auth.Outcome{}, auth.ErrDenied
		}
		if advice {
			var role string
			e = prompt.Use(func(b []byte) error {
				var p aicontext.PayloadData
				if json.Unmarshal(b, &p) != nil {
					return auth.ErrDenied
				}
				role = p.Role
				return nil
			})
			if e != nil || role != "host" {
				return auth.Outcome{}, auth.ErrDenied
			}
		}
		st, e := s.state().options.Storage.Bind(tx.Core())
		if e != nil {
			return auth.Outcome{}, auth.SafeError(e)
		}
		paused, e := st.Paused(ctx, v)
		if e != nil || paused {
			return auth.Outcome{}, auth.ErrDenied
		}
		var value TaskData
		if saved != nil {
			id, e := savedID(*saved)
			if e != nil {
				return auth.Outcome{}, e
			}
			r, e := st.Task(ctx, v, id)
			if e != nil {
				return auth.Outcome{}, auth.SafeError(e)
			}
			value = r.StorageValue()
			if value.Advice != advice {
				return auth.Outcome{}, auth.ErrConflict
			}
		} else {
			var id [16]byte
			if _, e = rand.Read(id[:]); e != nil {
				return auth.Outcome{}, auth.ErrUnavailable
			}
			value = TaskData{ID: hex.EncodeToString(id[:]), Subject: v, Advice: advice, State: "open"}
			if e = st.InsertTask(ctx, auth.RoomSecret(value)); e != nil {
				return auth.Outcome{}, auth.SafeError(e)
			}
		}
		if !sameSubject(value.Subject, v) {
			return auth.Outcome{}, auth.ErrDenied
		}
		d := &taskData{owner: s, value: value}
		task = Task{data: &d}
		return receipt(value.ID, "task"), nil
	}
	_, e = s.do(ctx, caller, "ai.task", t.StorageValue(), raw, auth.RoomCallbacks{Apply: func(c context.Context, t auth.Transaction, _ auth.SessionData) (auth.Outcome, error) {
		return work(c, t, nil)
	}, Replay: func(c context.Context, t auth.Transaction, _ auth.SessionData, o auth.Outcome) error {
		_, e := work(c, t, &o)
		return e
	}})
	if e != nil {
		return Task{}, safe(e)
	}
	return task, nil
}
func (s *Service) Reserve(ctx context.Context, caller model.Caller, task Task, amount Units) (Ticket, error) {
	td := task.state()
	if s.state() == nil || td == nil || td.owner != s || !Valid(amount) || amount.Calls != 1 || amount.Tokens < 1 || amount.LatencyMillis < 1 || amount.LatencyMillis > 60000 || amount.LocalComputeMillis < 1 || amount.LocalComputeMillis > 60000 {
		return Ticket{}, auth.ErrDenied
	}
	raw, e := canonical(struct {
		ID     string
		Amount Units
	}{td.value.ID, amount})
	if e != nil {
		return Ticket{}, e
	}
	hash := sha256.Sum256(raw)
	var ticket Ticket
	work := func(ctx context.Context, tx auth.Transaction, saved *auth.Outcome) (auth.Outcome, error) {
		prompt, e := s.state().options.Contexts.BuildWithin(ctx, tx.Core(), target(td.value.Subject))
		if e != nil {
			return auth.Outcome{}, e
		}
		v := prompt.Subject().StorageValue()
		if !sameSubject(v, td.value.Subject) || !Fits(amount, v.Budget) {
			return auth.Outcome{}, auth.ErrDenied
		}
		if td.value.Advice {
			prompt, e = prompt.Advice()
			if e != nil || amount.Subagents < 1 {
				return auth.Outcome{}, auth.ErrDenied
			}
		}
		if amount.ContextBytes < uint64(prompt.Bytes()) {
			return auth.Outcome{}, auth.ErrDenied
		}
		st, e := s.state().options.Storage.Bind(tx.Core())
		if e != nil {
			return auth.Outcome{}, auth.SafeError(e)
		}
		current, e := st.Task(ctx, v, td.value.ID)
		if e != nil {
			return auth.Outcome{}, auth.SafeError(e)
		}
		taskValue := current.StorageValue()
		if !sameSubject(taskValue.Subject, v) || taskValue.Advice != td.value.Advice {
			return auth.Outcome{}, auth.ErrDenied
		}
		r := ReservationData{Task: taskValue, Amount: amount, PromptBytes: uint64(prompt.Bytes()), RequestHash: hex.EncodeToString(hash[:]), Status: "reserved"}
		var stored Reservation
		if saved != nil {
			stored, e = st.Reservation(ctx, current)
			if e != nil {
				return auth.Outcome{}, auth.SafeError(e)
			}
			sr := stored.StorageValue()
			if sr.RequestHash != r.RequestHash || sr.Amount != amount {
				return auth.Outcome{}, auth.ErrConflict
			}
			id, e := savedID(*saved)
			if e != nil || id != td.value.ID {
				return auth.Outcome{}, auth.ErrConflict
			}
		} else {
			stored, e = st.Reserve(ctx, auth.RoomSecret(r), s.state().caps[v.Scope.WorkspaceID])
			if e != nil {
				return auth.Outcome{}, auth.SafeError(e)
			}
		}
		d := &ticketData{owner: s, value: stored.StorageValue(), prompt: prompt}
		ticket = Ticket{data: &d}
		if saved != nil {
			return *saved, nil
		}
		return receipt(td.value.ID, d.value.Status), nil
	}
	_, e = s.do(ctx, caller, "ai.reserve", target(td.value.Subject).StorageValue(), raw, auth.RoomCallbacks{Apply: func(c context.Context, t auth.Transaction, _ auth.SessionData) (auth.Outcome, error) {
		return work(c, t, nil)
	}, Replay: func(c context.Context, t auth.Transaction, _ auth.SessionData, o auth.Outcome) error {
		_, e := work(c, t, &o)
		return e
	}})
	if e != nil {
		return Ticket{}, safe(e)
	}
	if ticket.Status() == "paused" {
		return ticket, ErrPaused
	}
	return ticket, nil
}

// Run performs one dispatch transition before invoking an external provider.
// A duplicate/unknown dispatch cannot cause a second network call. Timeout,
// ambiguous billing or invalid usage retains every reservation and pauses.
func (s *Service) Run(ctx context.Context, caller model.Caller, ticket Ticket, provider Provider) (Result, error) {
	t := ticket.state()
	if s.state() == nil || t == nil || t.owner != s || t.value.Status != "reserved" || provider == nil || ctx == nil || ctx.Err() != nil {
		return Result{}, auth.ErrDenied
	}
	select {
	case s.state().active <- struct{}{}:
	default:
		return Result{}, auth.ErrUnavailable
	}
	providerStarted := false
	defer func() {
		if !providerStarted {
			<-s.state().active
		}
	}()
	c := caller.StorageValue()
	dispatched := false
	e := s.state().options.Authority.Inspect(ctx, c.Credential, c.CSRF, true, func(ctx context.Context, tx auth.Transaction, _ auth.SessionData) error {
		p, e := s.state().options.Contexts.BuildWithin(ctx, tx.Core(), target(t.value.Task.Subject))
		if e != nil {
			return e
		}
		if !sameSubject(p.Subject().StorageValue(), t.value.Task.Subject) {
			return auth.ErrDenied
		}
		if t.value.Task.Advice {
			p, e = p.Advice()
			if e != nil {
				return e
			}
		}
		var a, b []byte
		_ = p.Use(func(v []byte) error { a = append([]byte(nil), v...); return nil })
		_ = t.prompt.Use(func(v []byte) error { b = append([]byte(nil), v...); return nil })
		defer clear(a)
		defer clear(b)
		if !slices.Equal(a, b) {
			return auth.ErrConflict
		}
		st, e := s.state().options.Storage.Bind(tx.Core())
		if e != nil {
			return auth.SafeError(e)
		}
		paused, e := st.Paused(ctx, t.value.Task.Subject)
		if e != nil || paused {
			return auth.ErrDenied
		}
		if e = st.Dispatch(ctx, auth.RoomSecret(t.value)); e != nil {
			return auth.SafeError(e)
		}
		dispatched = true
		return nil
	})
	if e != nil || !dispatched {
		return Result{}, safe(e)
	}
	duration := min(t.value.Amount.LatencyMillis, t.value.Amount.LocalComputeMillis)
	deadline, cancel := context.WithTimeout(ctx, time.Duration(duration)*time.Millisecond)
	defer cancel()
	started := time.Now()
	type answer struct {
		value Response
		err   error
	}
	ch := make(chan answer, 1)
	providerStarted = true
	go func() {
		var a answer
		defer func() {
			<-s.state().active
			if recover() != nil {
				a.err = auth.ErrUnavailable
			}
			ch <- a
		}()
		a.value, a.err = provider.Call(deadline, t.prompt, t.value.Amount)
	}()
	var a answer
	select {
	case a = <-ch:
	case <-deadline.Done():
		a.err = auth.ErrUnavailable
	}
	response := a.value.StorageValue()
	spent := response.Usage
	elapsed := uint64((time.Since(started) + time.Millisecond - 1) / time.Millisecond)
	if elapsed > spent.LatencyMillis {
		spent.LatencyMillis = elapsed
	}
	if elapsed > spent.LocalComputeMillis {
		spent.LocalComputeMillis = elapsed
	}
	if spent.ContextBytes < uint64(t.prompt.Bytes()) {
		spent.ContextBytes = uint64(t.prompt.Bytes())
	}
	if t.value.Task.Advice && spent.Subagents < 1 {
		spent.Subagents = 1
	}
	resultRaw, resultErr := json.Marshal(ResultData{Advice: response.Advice, Usage: spent})
	known := a.err == nil && deadline.Err() == nil && response.Determinate && spent.Calls == 1 && Fits(spent, t.value.Amount) && checkpoint.Validate(response.Advice) == nil && resultErr == nil && len(resultRaw) <= aicontext.MaxPromptBytes
	var final Reservation
	// Billing reconciliation deliberately uses a bounded cancellation-independent
	// transaction. If it cannot commit, the dispatched durable hold still stands.
	settleCtx, finish := context.WithTimeout(context.WithoutCancel(ctx), 5*time.Second)
	defer finish()
	e = s.state().options.Authority.Inspect(settleCtx, c.Credential, c.CSRF, true, func(ctx context.Context, tx auth.Transaction, _ auth.SessionData) error {
		p, e := s.state().options.Contexts.BuildWithin(ctx, tx.Core(), target(t.value.Task.Subject))
		if e != nil {
			return e
		}
		if !sameSubject(p.Subject().StorageValue(), t.value.Task.Subject) {
			return auth.ErrDenied
		}
		st, e := s.state().options.Storage.Bind(tx.Core())
		if e != nil {
			return auth.SafeError(e)
		}
		final, e = st.Settle(ctx, auth.RoomSecret(t.value), spent, known)
		return auth.SafeError(e)
	})
	if e != nil {
		return Result{}, auth.ErrOutcomeUnknown
	}
	if !known || final.StorageValue().Status != "settled" {
		return Result{}, ErrPaused
	}
	var result ResultData
	if json.Unmarshal(resultRaw, &result) != nil {
		return Result{}, auth.ErrUnavailable
	}
	return auth.RoomSecret(result), nil
}
