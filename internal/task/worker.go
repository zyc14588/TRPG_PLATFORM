// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package task

import (
	"context"
	"fmt"
	"io"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/model"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
)

type Storage interface {
	Claim(context.Context, Worker) (Job, error)
	SaveResult(context.Context, Worker, Job, Value, Inputs) (Job, error)
	Current(context.Context, Worker, Job) (Job, error)
	Complete(context.Context, Worker, Job, data.Receipt) error
	Retry(context.Context, Worker, Job) error
	Cancel(context.Context, Worker, data.Binding, string) error
	Status(context.Context, Worker, data.Binding, string) (string, error)
}
type Policy struct {
	GraphHash, ConfigurationHash, PackageID string
	ValidateInput                           func(checkpoint.Value) error
	ValidateResult                          func(checkpoint.Value) error
	Execute                                 func(context.Context, Value) (Value, error)
	Inputs                                  func(context.Context, Job) (Inputs, error)
}
type WorkerOptions struct {
	Identity                           Worker
	Storage                            Storage
	Policies                           []Policy
	Post                               func(context.Context, Worker, Job) (data.Receipt, error)
	MaxActive                          int
	ExternalTimeout, PostTimeout, Poll time.Duration
}
type Runtime struct{ data **runtimeData }
type runtimeData struct {
	options  WorkerOptions
	policies map[string]Policy
	slots    chan struct{}
}

func (Runtime) Format(f fmt.State, _ rune)   { _, _ = io.WriteString(f, "<bounded task worker>") }
func (Runtime) MarshalJSON() ([]byte, error) { return nil, ErrDenied }
func (r *Runtime) state() *runtimeData {
	if r == nil || r.data == nil {
		return nil
	}
	return *r.data
}

type Report struct {
	Claimed, Executed, Applied int
	Code                       string
}

func policyKey(graph, configuration, pkg string) string {
	return graph + "\x00" + configuration + "\x00" + pkg
}
func NewWorker(o WorkerOptions) (*Runtime, error) {
	if o.Identity.state() == nil || o.Storage == nil || o.Post == nil || len(o.Policies) < 1 || len(o.Policies) > 128 || o.MaxActive < 1 || o.MaxActive > 4 || o.ExternalTimeout < time.Millisecond || o.ExternalTimeout > 5*time.Second || o.PostTimeout < time.Millisecond || o.PostTimeout > 3*time.Second || o.Poll < 50*time.Millisecond || o.Poll > 30*time.Second {
		return nil, ErrInvalid
	}
	policies := map[string]Policy{}
	for _, p := range o.Policies {
		if !checkpoint.IsDigest(p.GraphHash) || !checkpoint.IsDigest(p.ConfigurationHash) || p.ValidateInput == nil || p.ValidateResult == nil || p.Execute == nil || p.Inputs == nil {
			return nil, ErrInvalid
		}
		if _, e := model.ParsePackageID(p.PackageID); e != nil {
			return nil, ErrInvalid
		}
		key := policyKey(p.GraphHash, p.ConfigurationHash, p.PackageID)
		if _, ok := policies[key]; ok {
			return nil, ErrInvalid
		}
		policies[key] = p
	}
	o.Policies = nil
	d := &runtimeData{options: o, policies: policies, slots: make(chan struct{}, o.MaxActive)}
	return &Runtime{data: &d}, nil
}
func validateValue(v Value, f func(checkpoint.Value) error) (err error) {
	defer func() {
		if recover() != nil {
			err = ErrInvalid
		}
	}()
	raw, e := v.StorageValue()
	if e != nil || f == nil {
		return ErrInvalid
	}
	if f(raw) != nil {
		return ErrInvalid
	}
	return nil
}
func boundDeadline(ctx context.Context, limit time.Duration, expires time.Time) (context.Context, context.CancelFunc) {
	deadline := time.Now().Add(limit)
	if expires.Before(deadline) {
		deadline = expires
	}
	return context.WithDeadline(ctx, deadline)
}
func (r *Runtime) retry(ctx context.Context, j Job) {
	bounded, cancel := context.WithTimeout(context.WithoutCancel(ctx), 2*time.Second)
	defer cancel()
	_ = r.state().options.Storage.Retry(bounded, r.state().options.Identity, j)
}
func (r *Runtime) RunOnce(ctx context.Context) (report Report, err error) {
	if r.state() == nil || ctx == nil || ctx.Err() != nil {
		return report, ErrDenied
	}
	d := r.state()
	o := d.options
	if _, err = o.Identity.Workspaces(ctx); err != nil {
		return report, ErrDenied
	}
	select {
	case d.slots <- struct{}{}:
	default:
		return report, ErrBusy
	}
	slotOwned := true
	defer func() {
		if slotOwned {
			<-d.slots
		}
		if recover() != nil {
			err = ErrUnavailable
		}
		if err != nil {
			err = SafeError(err)
			report.Code = err.Error()
		}
	}()
	j, e := o.Storage.Claim(ctx, o.Identity)
	if e != nil {
		return report, SafeError(e)
	}
	report.Claimed = 1
	v, e := j.StorageValue()
	if e != nil {
		return report, ErrInvalid
	}
	p, ok := d.policies[policyKey(v.Binding.GraphHash, v.ConfigurationHash, v.PackageID)]
	if !ok || o.Identity.Check(ctx, v.Binding.Workspace) != nil || validateValue(v.Payload, p.ValidateInput) != nil {
		r.retry(ctx, j)
		return report, ErrDenied
	}
	if v.Status == Running {
		// A timed-out callback retains this bounded slot until it really exits.
		// Repeated polls cannot create unbounded abandoned goroutines.
		expires := v.Expires
		if v.LeaseUntil.Before(expires) {
			expires = v.LeaseUntil
		}
		workCtx, cancel := boundDeadline(ctx, o.ExternalTimeout, expires)
		defer cancel()
		type answer struct {
			value  Value
			inputs Inputs
			err    error
		}
		reply := make(chan answer, 1)
		slotOwned = false
		go func() {
			a := answer{}
			defer func() {
				if recover() != nil {
					a = answer{err: ErrUnavailable}
				}
				<-d.slots
				reply <- a
			}()
			if o.Identity.Check(workCtx, v.Binding.Workspace) != nil {
				a.err = ErrDenied
				return
			}
			a.value, a.err = p.Execute(workCtx, v.Payload)
			if a.err != nil || workCtx.Err() != nil {
				a.err = ErrFailed
				return
			}
			if validateValue(a.value, p.ValidateResult) != nil {
				a.err = ErrInvalid
				return
			}
			a.inputs, a.err = p.Inputs(workCtx, j)
			if a.err != nil || a.inputs.state() == nil || workCtx.Err() != nil {
				a.err = ErrInvalid
			}
		}()
		var a answer
		select {
		case <-workCtx.Done():
			r.retry(ctx, j)
			return report, workCtx.Err()
		case a = <-reply:
		}
		if a.err != nil {
			r.retry(ctx, j)
			return report, SafeError(a.err)
		}
		// Reacquire a slot for the bounded delivery phase. The callback has
		// completed; the operational result is committed before any Actor entry.
		select {
		case d.slots <- struct{}{}:
			slotOwned = true
		default:
			r.retry(ctx, j)
			return report, ErrBusy
		}
		if o.Identity.Check(ctx, v.Binding.Workspace) != nil {
			r.retry(ctx, j)
			return report, ErrDenied
		}
		j, e = o.Storage.SaveResult(ctx, o.Identity, j, a.value, a.inputs)
		if e != nil {
			return report, SafeError(e)
		}
		report.Executed = 1
		v, e = j.StorageValue()
		if e != nil {
			return report, ErrInvalid
		}
	}
	if v.Status == Done {
		return report, nil
	}
	if v.Status != Delivering || validateValue(v.Result, p.ValidateResult) != nil {
		return report, ErrInvalid
	}
	expires := v.LeaseUntil
	if v.Expires.Before(expires) {
		expires = v.Expires
	}
	postCtx, cancel := boundDeadline(ctx, o.PostTimeout, expires)
	defer cancel()
	current, e := o.Storage.Current(postCtx, o.Identity, j)
	if e != nil {
		r.retry(ctx, j)
		return report, SafeError(e)
	}
	receipt, e := o.Post(postCtx, o.Identity, current)
	if e != nil {
		r.retry(ctx, j)
		return report, SafeError(e)
	}
	if e = o.Storage.Complete(ctx, o.Identity, current, receipt); e != nil {
		return report, SafeError(e)
	}
	report.Applied = 1
	return report, nil
}
func (r *Runtime) Run(ctx context.Context) error {
	if r.state() == nil || ctx == nil {
		return ErrDenied
	}
	ticker := time.NewTicker(r.state().options.Poll)
	defer ticker.Stop()
	for {
		select {
		case <-ctx.Done():
			return ctx.Err()
		case <-ticker.C:
			_, _ = r.RunOnce(ctx)
		}
	}
}
