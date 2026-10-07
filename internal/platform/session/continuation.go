// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package session

import (
	"context"
	"encoding/json"
	"fmt"
	"io"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/model"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/launch"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/command"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
	"github.com/zyc14588/TRPG_PLATFORM/internal/task"
)

type ContinuationOptions struct {
	Launch   *launch.Service
	Storage  task.Storage
	Policies []task.Policy
}
type Continuations struct{ data **continuationData }
type continuationData struct {
	launch   *launch.Service
	storage  task.Storage
	policies map[string]func(checkpoint.Value) error
}

func (Continuations) Format(f fmt.State, _ rune) {
	_, _ = io.WriteString(f, "<private continuation transport>")
}
func (Continuations) MarshalJSON() ([]byte, error) { return nil, task.ErrDenied }
func (s *Continuations) state() *continuationData {
	if s == nil || s.data == nil {
		return nil
	}
	return *s.data
}
func continuationPolicyKey(v task.JobData) string {
	return v.Binding.GraphHash + "\x00" + v.ConfigurationHash + "\x00" + v.PackageID
}
func NewContinuations(o ContinuationOptions) (*Continuations, error) {
	if o.Launch == nil || o.Storage == nil || len(o.Policies) < 1 || len(o.Policies) > 128 {
		return nil, task.ErrInvalid
	}
	d := &continuationData{launch: o.Launch, storage: o.Storage, policies: map[string]func(checkpoint.Value) error{}}
	for _, p := range o.Policies {
		if !checkpoint.IsDigest(p.GraphHash) || !checkpoint.IsDigest(p.ConfigurationHash) || p.ValidateResult == nil {
			return nil, task.ErrInvalid
		}
		if _, e := model.ParsePackageID(p.PackageID); e != nil {
			return nil, task.ErrInvalid
		}
		key := p.GraphHash + "\x00" + p.ConfigurationHash + "\x00" + p.PackageID
		if _, ok := d.policies[key]; ok {
			return nil, task.ErrInvalid
		}
		d.policies[key] = p.ValidateResult
	}
	return &Continuations{data: &d}, nil
}
func (s *Continuations) current(ctx context.Context, w task.Worker, j task.Job) (current task.Job, err error) {
	defer func() {
		if recover() != nil {
			current = task.Job{}
			err = task.ErrInvalid
		}
	}()
	if s.state() == nil || ctx == nil || ctx.Err() != nil {
		return task.Job{}, task.ErrDenied
	}
	current, e := s.state().storage.Current(ctx, w, j)
	if e != nil {
		return task.Job{}, task.SafeError(e)
	}
	v, e := current.StorageValue()
	if e != nil || w.Check(ctx, v.Binding.Workspace) != nil {
		return task.Job{}, task.ErrDenied
	}
	validate := s.state().policies[continuationPolicyKey(v)]
	result, e := v.Result.StorageValue()
	if e != nil || validate == nil || validate(result) != nil {
		return task.Job{}, task.ErrInvalid
	}
	return current, nil
}

// Post is a trusted internal worker seam. Browser cookies and supplied seat IDs
// have no path to this authority; the exact task/token/version is re-read even
// during duplicate resolution and immediately before the Host transaction.
func (s *Continuations) Post(ctx context.Context, w task.Worker, j task.Job) (data.Receipt, error) {
	current, e := s.current(ctx, w, j)
	if e != nil {
		return data.Receipt{}, e
	}
	expected, e := current.Envelope()
	if e != nil {
		return data.Receipt{}, task.ErrDenied
	}
	resolver := func(ctx context.Context) (command.NativeSeat, error) {
		live, e := s.current(ctx, w, j)
		if e != nil {
			return command.NativeSeat{}, e
		}
		v, e := live.StorageValue()
		if e != nil {
			return command.NativeSeat{}, task.ErrDenied
		}
		envelope, e := live.Envelope()
		if e != nil {
			return command.NativeSeat{}, task.ErrDenied
		}
		a, _ := json.Marshal(expected)
		b, _ := json.Marshal(envelope)
		if string(a) != string(b) {
			return command.NativeSeat{}, task.ErrDenied
		}
		validate := func(value checkpoint.Value) error {
			a, e := json.Marshal(value)
			if e != nil {
				return task.ErrDenied
			}
			b, _ := json.Marshal(envelope.Payload)
			if string(a) != string(b) {
				return task.ErrDenied
			}
			return nil
		}
		inputs := func(ctx context.Context, _ command.Envelope) (command.NativeInputs, error) {
			live, e := s.current(ctx, w, j)
			if e != nil {
				return command.NativeInputs{}, e
			}
			v, e := live.StorageValue()
			if e != nil {
				return command.NativeInputs{}, task.ErrDenied
			}
			at, random, e := v.Inputs.StorageValue()
			if e != nil {
				return command.NativeInputs{}, task.ErrDenied
			}
			result, e := v.Result.StorageValue()
			if e != nil {
				return command.NativeInputs{}, task.ErrDenied
			}
			return command.NativeInputs{Time: at, Random: random, ToolResults: []checkpoint.Value{result}}, nil
		}
		return command.NativeSeat{Binding: v.Binding, Principal: "task-system", Seat: "task-system", Commands: map[string]func(checkpoint.Value) error{"resume-continuation": validate}, Views: command.ViewPolicy{}, Inputs: inputs}, nil
	}
	return s.state().launch.SubmitContinuation(ctx, current, resolver)
}
