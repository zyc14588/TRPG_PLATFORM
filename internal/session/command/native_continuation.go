// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package command

import (
	"context"
	"encoding/json"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
)

func sameNativeEnvelope(a, b Envelope) bool {
	x, e := json.Marshal(a)
	if e != nil {
		return false
	}
	y, e := json.Marshal(b)
	return e == nil && string(x) == string(y)
}
func sameNativeValue(a, b checkpoint.Value) bool {
	if checkpoint.Validate(a) != nil || checkpoint.Validate(b) != nil {
		return false
	}
	x, e := json.Marshal(a)
	if e != nil {
		return false
	}
	y, e := json.Marshal(b)
	return e == nil && string(x) == string(y)
}
func ownNativeValues(values []checkpoint.Value) ([]checkpoint.Value, error) {
	if len(values) == 0 {
		return nil, nil
	}
	raw, e := json.Marshal(values)
	if e != nil || len(raw) > 128<<10 {
		return nil, ErrDenied
	}
	var out []checkpoint.Value
	if checkpoint.StrictDecode(raw, &out, 128<<10) != nil {
		return nil, ErrDenied
	}
	for _, v := range out {
		if checkpoint.Validate(v) != nil {
			return nil, ErrDenied
		}
	}
	return out, nil
}

// IssueContinuation is reserved for the trusted task composition. Its current
// resolver authenticates the worker and the exact durable lease on every use.
// A browser-issued identity has no continuation purpose, regardless of type.
func (a *Authority) IssueContinuation(ctx context.Context, current NativeResolver, expected Envelope) (Identity, error) {
	raw, e := json.Marshal(expected)
	if e != nil || len(raw) > 128<<10 {
		return Identity{}, ErrDenied
	}
	owned, e := Decode(raw)
	if e != nil || owned.Type != "resume-continuation" || owned.SeatID != "task-system" || owned.Payload.Kind != "table" || checkpoint.Validate(owned.Payload.Table["result"]) != nil {
		return Identity{}, ErrDenied
	}
	i, e := a.IssueNative(ctx, current)
	if e != nil {
		return Identity{}, e
	}
	d := a.state()
	v := i.state()
	d.mu.Lock()
	r, ok := d.records[v.key]
	if !ok || r.seat.Principal != "task-system" || r.seat.Seat != "task-system" || owned.SessionID != r.seat.Binding.Session || len(r.seat.Commands) != 1 || r.seat.Commands[owned.Type] == nil || r.inputs == nil || len(r.seat.Views.ViewFields) != 0 || len(r.seat.Views.ResultFields) != 0 || len(r.seat.Views.EventFields) != 0 || r.seat.Views.ScalarResult || r.recovery {
		delete(d.records, v.key)
		d.mu.Unlock()
		return Identity{}, ErrDenied
	}
	r.continuation = &owned
	d.records[v.key] = r
	d.mu.Unlock()
	if _, e = a.ValidateContext(ctx, i, owned); e != nil {
		_ = a.ReleaseNative(i)
		return Identity{}, ErrDenied
	}
	return i, nil
}
func (a *Authority) CallbackContext(ctx context.Context, i Identity, e Envelope) (string, error) {
	if _, err := a.ValidateContext(ctx, i, e); err != nil {
		return "", err
	}
	r, err := a.resolveCurrent(ctx, i)
	if err != nil {
		return "", err
	}
	if r.continuation != nil {
		return "resume_continuation", nil
	}
	if e.Type == "end" {
		return "on_session_end", nil
	}
	return "command", nil
}
