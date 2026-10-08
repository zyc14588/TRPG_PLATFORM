// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package player

import (
	"context"
	"sync/atomic"
	"testing"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/task"
)

type taskBase struct{ task.Storage }

func TestPlayerTaskGateCoversPaidNarrativeAndProposalCallbacks(t *testing.T) {
	for _, mode := range []string{"proposal", "narrative", "task"} {
		t.Run(mode, func(t *testing.T) {
			c, m, one, two := controlFixture(t)
			ctx := context.Background()
			_, _ = c.ResumeWithin(ctx, m, one, 1)
			_, _ = c.ResumeWithin(ctx, m, two, 2)
			a := one.StorageValue().Access
			var calls atomic.Int32
			policy := task.Policy{GraphHash: a.Binding.GraphHash, ConfigurationHash: a.ConfigurationHash, PackageID: "example.test/host", ValidateInput: func(v checkpoint.Value) error {
				if v.Kind != "string" || v.String != mode {
					return task.ErrInvalid
				}
				return nil
			}, ValidateResult: func(checkpoint.Value) error { return nil }, Execute: func(context.Context, task.Value) (task.Value, error) {
				calls.Add(1)
				return task.NewValue(checkpoint.Text("bounded result"))
			}, Inputs: func(context.Context, task.Job) (task.Inputs, error) {
				return task.NewInputs(time.Now().UnixMilli(), nil)
			}}
			_, policies, e := BindTasks(&taskBase{}, c, []task.Policy{policy})
			if e != nil {
				t.Fatal("task policy composition rejected")
			}
			meta := taskMetadata(task.JobData{Scope: a.Scope, Binding: a.Binding, ConfigurationID: "minimal", ConfigurationHash: a.ConfigurationHash, PackageID: policy.PackageID, TaskID: "task", OriginPrincipal: a.Principal, OriginVersion: 1})
			payload := checkpoint.Object(map[string]checkpoint.Value{"player_server": meta, "input": checkpoint.Text(mode)})
			value, e := task.NewValue(payload)
			if e != nil {
				t.Fatal("bounded task payload invalid")
			}
			if policies[0].ValidateInput(payload) != nil {
				t.Fatal("canonical task metadata rejected")
			}
			if _, e = policies[0].Execute(ctx, value); e != nil || calls.Load() != 1 {
				t.Fatal("current task did not execute once")
			}
			if _, e = c.PauseWithin(ctx, m, two, 3); e != nil {
				t.Fatal("pause failed")
			}
			if e = c.Quiesce(ctx, a.Binding); e != nil {
				t.Fatal("pause fence failed")
			}
			if _, e = policies[0].Execute(ctx, value); e != task.ErrDenied || calls.Load() != 1 {
				t.Fatal("paused callback dispatched provider or narrative")
			}
			payload.Table["player_server"].Table["graph"] = checkpoint.Text("bad")
			if policies[0].ValidateInput(payload) == nil {
				t.Fatal("substituted task metadata accepted")
			}
		})
	}
}
func TestPlayerPauseCancelsAndWaitsForInFlightExternalCallback(t *testing.T) {
	c, m, one, two := controlFixture(t)
	ctx := context.Background()
	_, _ = c.ResumeWithin(ctx, m, one, 1)
	_, _ = c.ResumeWithin(ctx, m, two, 2)
	a := one.StorageValue().Access
	work, release, e := c.BeginMutation(ctx, a.Scope, a.Binding)
	if e != nil {
		t.Fatal("current work rejected")
	}
	done := make(chan error, 1)
	go func() { done <- c.Quiesce(ctx, a.Binding) }()
	select {
	case <-work.Done():
	case <-time.After(time.Second):
		t.Fatal("pause did not cancel admitted external work")
	}
	select {
	case <-done:
		t.Fatal("pause acknowledgment overtook admitted external work")
	default:
	}
	release()
	select {
	case e := <-done:
		if e != nil {
			t.Fatal("pause fence failed")
		}
	case <-time.After(time.Second):
		t.Fatal("pause fence did not finish")
	}
}
