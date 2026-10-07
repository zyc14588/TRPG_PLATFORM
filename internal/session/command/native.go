// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package command

import (
	"context"
	"crypto/rand"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
	"time"
)

// NativeResolver is a trusted server-composition seam, never client policy or a
// Lua capability. Every use retrieves the current cookie, participant, human
// slot, installed graph and approved package policy. No grant is cached here.
type NativeResolver func(context.Context) (NativeSeat, error)
type NativeInputs struct {
	Time        int64
	Random      []int64
	ToolResults []checkpoint.Value
}
type NativeInputSource func(context.Context, Envelope) (NativeInputs, error)
type NativeSeat struct {
	Binding         data.Binding
	Principal, Seat string
	Commands        map[string]func(checkpoint.Value) error
	Views           ViewPolicy
	RecoveryPoint   bool
	Inputs          NativeInputSource
}

func ownNative(v NativeSeat) (record, error) {
	if !store.ValidID(v.Binding.Workspace) || !store.ValidID(v.Binding.Session) || !checkpoint.IsDigest(v.Binding.GraphHash) || !store.ValidID(v.Principal) || !store.ValidID(v.Seat) || len(v.Commands) > 32 {
		return record{}, ErrDenied
	}
	p, e := copyPolicy(v.Views)
	if e != nil {
		return record{}, e
	}
	commands := map[string]func(checkpoint.Value) error{}
	for name, f := range v.Commands {
		if !store.ValidID(name) || f == nil {
			return record{}, ErrDenied
		}
		commands[name] = f
	}
	return record{seat: FixtureSeat{Binding: v.Binding, Principal: v.Principal, Seat: v.Seat, Commands: commands, Views: p}, recovery: v.RecoveryPoint, inputs: v.Inputs}, nil
}
func currentNative(ctx context.Context, f NativeResolver) (r record, err error) {
	defer func() {
		if recover() != nil {
			r = record{}
			err = ErrDenied
		}
	}()
	if ctx == nil || ctx.Err() != nil || f == nil {
		return r, ErrDenied
	}
	bounded, cancel := context.WithTimeout(ctx, 2*time.Second)
	defer cancel()
	v, e := f(bounded)
	if e != nil || bounded.Err() != nil {
		return r, ErrDenied
	}
	return ownNative(v)
}
func (a *Authority) IssueNative(ctx context.Context, current NativeResolver) (Identity, error) {
	if a.state() == nil {
		return Identity{}, ErrDenied
	}
	r, e := currentNative(ctx, current)
	if e != nil {
		return Identity{}, e
	}
	var key [32]byte
	if _, e = rand.Read(key[:]); e != nil {
		return Identity{}, ErrDenied
	}
	r.current = current
	r.epoch = 1
	d := a.state()
	d.mu.Lock()
	defer d.mu.Unlock()
	if len(d.records) >= 256 {
		return Identity{}, ErrDenied
	}
	if _, exists := d.records[key]; exists {
		return Identity{}, ErrDenied
	}
	d.records[key] = r
	return issued(a, key, r), nil
}
func (a *Authority) resolveCurrent(ctx context.Context, i Identity) (record, error) {
	d := a.state()
	v := i.state()
	if d == nil || v == nil || v.authority != a || ctx == nil || ctx.Err() != nil {
		return record{}, ErrDenied
	}
	d.mu.RLock()
	r, ok := d.records[v.key]
	d.mu.RUnlock()
	if !ok || r.disabled || r.epoch != v.epoch || r.seat.Binding != v.binding || r.seat.Principal != v.principal || r.seat.Seat != v.seat {
		return record{}, ErrDenied
	}
	if r.current != nil {
		live, e := currentNative(ctx, r.current)
		if e != nil || live.seat.Binding != v.binding || live.seat.Principal != v.principal || live.seat.Seat != v.seat {
			return record{}, ErrDenied
		}
		if r.continuation != nil && (len(live.seat.Commands) != 1 || live.seat.Commands["resume-continuation"] == nil || live.inputs == nil || live.recovery || len(live.seat.Views.ViewFields) != 0 || len(live.seat.Views.ResultFields) != 0 || len(live.seat.Views.EventFields) != 0 || live.seat.Views.ScalarResult) {
			return record{}, ErrDenied
		}
		// The resolver executes outside our lock. Closing/revoking the handle while
		// its database read is in flight must still invalidate that read's result.
		d.mu.RLock()
		latest, exists := d.records[v.key]
		d.mu.RUnlock()
		if !exists || latest.disabled || latest.epoch != v.epoch {
			return record{}, ErrDenied
		}
		live.continuation = r.continuation
		return live, nil
	}
	return r, nil
}
func (a *Authority) ReleaseNative(i Identity) error {
	d := a.state()
	v := i.state()
	if d == nil || v == nil || v.authority != a {
		return ErrDenied
	}
	d.mu.Lock()
	defer d.mu.Unlock()
	r, ok := d.records[v.key]
	if !ok || r.current == nil || r.epoch != v.epoch {
		return ErrDenied
	}
	delete(d.records, v.key)
	return nil
}
func (a *Authority) CheckRecoveryPoint(ctx context.Context, i Identity) error {
	r, e := a.resolveCurrent(ctx, i)
	if e != nil || !r.recovery {
		return ErrDenied
	}
	return nil
}

// OwnPolicy copies a trusted package policy before another layer keeps it.
func OwnPolicy(p ViewPolicy) (ViewPolicy, error) { return copyPolicy(p) }

// InputsContext runs only after duplicate lookup, on the existing actor mailbox.
// Values come from an approved server package policy, never envelope fields.
func (a *Authority) InputsContext(ctx context.Context, i Identity, e Envelope) (NativeInputs, error) {
	r, err := a.resolveCurrent(ctx, i)
	if err != nil || r.inputs == nil {
		return NativeInputs{}, ErrDenied
	}
	owned, err := a.ValidateContext(ctx, i, e)
	if err != nil {
		return NativeInputs{}, err
	}
	v, err := r.inputs(ctx, owned)
	if err != nil || ctx.Err() != nil || v.Time < 0 || len(v.Random) > 256 {
		return NativeInputs{}, ErrDenied
	}
	for _, n := range v.Random {
		if n < 0 {
			return NativeInputs{}, ErrDenied
		}
	}
	v.Random = append([]int64(nil), v.Random...)
	if len(v.ToolResults) > 1 || r.continuation == nil && len(v.ToolResults) != 0 {
		return NativeInputs{}, ErrDenied
	}
	if r.continuation != nil && (len(v.ToolResults) != 1 || !sameNativeValue(v.ToolResults[0], owned.Payload.Table["result"])) {
		return NativeInputs{}, ErrDenied
	}
	v.ToolResults, err = ownNativeValues(v.ToolResults)
	if err != nil {
		return NativeInputs{}, ErrDenied
	}
	return v, nil
}
