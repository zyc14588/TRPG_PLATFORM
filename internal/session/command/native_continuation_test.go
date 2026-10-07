// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package command

import (
	"context"
	"testing"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
)

func continuationFixture(t *testing.T) (*Authority, NativeSeat, Envelope) {
	t.Helper()
	a, s, e := nativeFixture(t)
	s.Principal = "task-system"
	s.Seat = "task-system"
	s.Commands = map[string]func(checkpoint.Value) error{"resume-continuation": func(checkpoint.Value) error { return nil }}
	s.Views = ViewPolicy{}
	s.RecoveryPoint = false
	s.Inputs = func(context.Context, Envelope) (NativeInputs, error) {
		return NativeInputs{Time: 1000, Random: []int64{7}, ToolResults: []checkpoint.Value{checkpoint.Int(5)}}, nil
	}
	e.SeatID = s.Seat
	e.Type = "resume-continuation"
	e.Payload = checkpoint.Object(map[string]checkpoint.Value{"result": checkpoint.Int(5)})
	return a, s, e
}
func TestContinuationPurposeBindsEntireEnvelopeAndOwnsInput(t *testing.T) {
	a, s, e := continuationFixture(t)
	ctx := context.Background()
	i, err := a.IssueContinuation(ctx, func(context.Context) (NativeSeat, error) { return s, nil }, e)
	if err != nil {
		t.Fatal("issue")
	}
	defer a.ReleaseNative(i)
	if callback, err := a.CallbackContext(ctx, i, e); err != nil || callback != "resume_continuation" {
		t.Fatal("standard callback")
	}
	if a.CheckRecoveryPoint(ctx, i) != ErrDenied {
		t.Fatal("system recovery grant")
	}
	inputs, err := a.InputsContext(ctx, i, e)
	if err != nil || len(inputs.ToolResults) != 1 {
		t.Fatal("persisted inputs")
	}
	inputs.ToolResults[0] = checkpoint.Int(9)
	again, err := a.InputsContext(ctx, i, e)
	if err != nil || again.ToolResults[0].Number != "5" {
		t.Fatal("input alias")
	}
	for _, name := range []string{"command", "session", "seat", "type", "correlation", "version", "payload"} {
		t.Run(name, func(t *testing.T) {
			q := e
			switch name {
			case "command":
				q.CommandID = "changed"
			case "session":
				q.SessionID = "changed"
			case "seat":
				q.SeatID = "changed"
			case "type":
				q.Type = "changed"
			case "correlation":
				q.CorrelationID = "changed"
			case "version":
				q.ExpectedStateVersion++
			case "payload":
				q.Payload = checkpoint.Object(map[string]checkpoint.Value{"result": checkpoint.Int(9)})
			}
			if _, err := a.ValidateContext(ctx, i, q); err == nil {
				t.Fatal("purpose substituted")
			}
		})
	}
	e.Payload.Table["result"] = checkpoint.Int(9)
	original := e
	original.Payload = checkpoint.Object(map[string]checkpoint.Value{"result": checkpoint.Int(5)})
	if _, err = a.ValidateContext(ctx, i, original); err != nil {
		t.Fatal("expected envelope aliased")
	}
}
func TestHumanEnvelopeTypeCannotAcquireContinuationPurposeOrToolResults(t *testing.T) {
	a, s, e := continuationFixture(t)
	ctx := context.Background()
	s.Principal = "person"
	s.Seat = "player"
	e.SeatID = "player"
	i, err := a.IssueNative(ctx, func(context.Context) (NativeSeat, error) { return s, nil })
	if err != nil {
		t.Fatal("native issue")
	}
	if callback, err := a.CallbackContext(ctx, i, e); err != nil || callback != "command" {
		t.Fatal("human selected standard system callback")
	}
	if _, err = a.InputsContext(ctx, i, e); err != ErrDenied {
		t.Fatal("human supplied tools")
	}
	if _, err = a.IssueContinuation(ctx, func(context.Context) (NativeSeat, error) { return s, nil }, e); err != ErrDenied {
		t.Fatal("human system issue")
	}
}
func TestContinuationRevalidatesLiveResultScopeRevocationAndDeniesViews(t *testing.T) {
	for _, name := range []string{"result", "scope", "revoke", "view"} {
		t.Run(name, func(t *testing.T) {
			a, s, e := continuationFixture(t)
			ctx := context.Background()
			live := true
			resolver := func(context.Context) (NativeSeat, error) {
				if !live {
					return NativeSeat{}, ErrDenied
				}
				return s, nil
			}
			i, err := a.IssueContinuation(ctx, resolver, e)
			if err != nil {
				t.Fatal("issue")
			}
			switch name {
			case "result":
				s.Inputs = func(context.Context, Envelope) (NativeInputs, error) {
					return NativeInputs{Time: 1, ToolResults: []checkpoint.Value{checkpoint.Int(9)}}, nil
				}
				if _, err = a.InputsContext(ctx, i, e); err != ErrDenied {
					t.Fatal("changed result")
				}
			case "scope":
				s.Binding.Workspace = "other"
				if a.VerifyContext(ctx, i) != ErrDenied {
					t.Fatal("changed scope")
				}
			case "revoke":
				live = false
				if a.VerifyContext(ctx, i) != ErrDenied {
					t.Fatal("revoked lease")
				}
			case "view":
				s.Views.ViewFields = []string{"secret"}
				if _, err = a.IssueContinuation(ctx, resolver, e); err != ErrDenied {
					t.Fatal("system view grant")
				}
			}
		})
	}
}
