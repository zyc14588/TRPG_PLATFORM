//go:build linux && integration

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package model_gateway_test

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"net/http"
	"strings"
	"sync/atomic"
	"testing"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/budget"
	aicontext "github.com/zyc14588/TRPG_PLATFORM/internal/ai/context"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
)

func TestGatewaySendsOnlyFilteredSeatContextAndAccountsActualLocalResponse(t *testing.T) {
	var calls atomic.Int64
	f, g := configuredGateway(t, func(w http.ResponseWriter, r *http.Request) {
		calls.Add(1)
		v := readLocal(t, r)
		for _, marker := range []string{"synthetic-human-private-state", "synthetic-other-human-event", "synthetic-nested-private-state", "owned-fixture-private-provider-key"} {
			if strings.Contains(v.Messages[1].Content, marker) {
				t.Error("private source reached local provider")
			}
		}
		if r.Header.Get("Authorization") != "Bearer owned-fixture-private-provider-key-329874" {
			t.Error("credential boundary missing")
		}
		var p aicontext.PayloadData
		if json.Unmarshal([]byte(v.Messages[1].Content), &p) != nil || p.SeatID != "ai" || len(p.Tools) != 2 {
			t.Error("filtered identity or tool boundary lost")
		}
		writeAnswer(w, v.Model, actionText(p.Version))
	}, false, time.Second, 3)
	out, e := g.Execute(f.ctx, f.caller(f.owner, ""), gatewayRequest(t, f, "owned-action", "proposal"))
	need(t, e)
	v := out.StorageValue()
	if v.Status != "complete" || v.Action == nil || v.Action.ExpectedVersion != 1 || v.Fallback || v.Usage.Calls != 1 || calls.Load() != 1 {
		t.Fatal("actual local proposal or billing incorrect")
	}
	used, held := f.counter(t, "workspace")
	if used.Calls != 1 || used.Tools != 1 || held != (budget.Units{}) {
		t.Fatal("actual reservation not settled")
	}
}
func TestMalformedPrimaryIsBoundedAndOnlyPreauthorizedFallbackRuns(t *testing.T) {
	var primary, fallback atomic.Int64
	f, g := configuredGateway(t, func(w http.ResponseWriter, r *http.Request) {
		v := readLocal(t, r)
		var p aicontext.PayloadData
		_ = json.Unmarshal([]byte(v.Messages[1].Content), &p)
		if v.Model == "fixture:small" {
			primary.Add(1)
			writeAnswer(w, v.Model, "malformed structured result")
			return
		}
		fallback.Add(1)
		writeAnswer(w, v.Model, actionText(p.Version))
	}, true, time.Second, 3)
	out, e := g.Execute(f.ctx, f.caller(f.owner, ""), gatewayRequest(t, f, "fallback-action", "proposal"))
	need(t, e)
	v := out.StorageValue()
	if v.Status != "complete" || !v.Fallback || primary.Load() != 2 || fallback.Load() != 1 || v.Usage.Calls != 3 {
		t.Fatal("bounded preauthorized fallback path incorrect")
	}
	used, held := f.counter(t, "seat")
	if used.Calls != 3 || held != (budget.Units{}) {
		t.Fatal("retry/fallback bypassed seat accounting")
	}
}
func TestNoFallbackDurablyPausesOnlySelectedSeatWithoutFictitiousBilling(t *testing.T) {
	var calls atomic.Int64
	f, g := configuredGateway(t, func(w http.ResponseWriter, r *http.Request) {
		calls.Add(1)
		v := readLocal(t, r)
		writeAnswer(w, v.Model, "malformed")
	}, false, time.Second, 2)
	request := gatewayRequest(t, f, "no-fallback", "proposal")
	out, e := g.Execute(f.ctx, f.caller(f.owner, ""), request)
	need(t, e)
	if out.StorageValue().Status != "paused" || calls.Load() != 2 {
		t.Fatal("missing fallback did not pause finitely")
	}
	used, held := f.counter(t, "workspace")
	if used.Calls != 2 || held != (budget.Units{}) {
		t.Fatal("pause fabricated spend or hold")
	}
	if string(sqlCapture(t, fmt.Sprintf("SELECT count(*) FROM platform_budget.pauses WHERE workspace_id='%s' AND seat_id='ai'", f.scope.WorkspaceID))) != "1\n" {
		t.Fatal("selected seat pause not durable")
	}
	if string(sqlCapture(t, fmt.Sprintf("SELECT count(*) FROM platform_budget.pauses WHERE workspace_id='%s' AND seat_id='ai-other'", f.scope.WorkspaceID))) != "0\n" {
		t.Fatal("unrelated seat paused")
	}
	need(t, g.Pause(f.ctx, f.caller(f.owner, ""), request))
	if _, e := f.contexts.Build(f.ctx, f.caller(f.owner, ""), f.target("ai-other")); e != nil {
		t.Fatal("unrelated context disabled")
	}
}
func TestTimeoutRetainsEveryHoldAndDoesNotAttemptFallback(t *testing.T) {
	var calls atomic.Int64
	f, g := configuredGateway(t, func(w http.ResponseWriter, r *http.Request) { calls.Add(1); <-r.Context().Done() }, true, 20*time.Millisecond, 3)
	out, e := g.Execute(f.ctx, f.caller(f.owner, ""), gatewayRequest(t, f, "timeout-action", "proposal"))
	need(t, e)
	if out.StorageValue().Status != "paused" || calls.Load() != 1 {
		t.Fatal("unknown billing retried or lost pause")
	}
	for _, level := range []string{"workspace", "room", "session", "seat", "task"} {
		used, held := f.counter(t, level)
		want := amount()
		want.Subagents = 0
		want.Tools = 1
		if used != (budget.Units{}) || held != want {
			t.Fatal("unknown billing released reservation dimension")
		}
	}
}
func TestRevokedIdentityStaleVersionAndNarrowedToolsPreventProviderDispatch(t *testing.T) {
	for _, name := range []string{"disabled", "version", "tools", "tenant"} {
		t.Run(name, func(t *testing.T) {
			var calls atomic.Int64
			f, g := configuredGateway(t, func(w http.ResponseWriter, r *http.Request) {
				calls.Add(1)
				writeAnswer(w, "fixture:small", actionText(1))
			}, false, time.Second, 3)
			request := gatewayRequest(t, f, "denied-action", "proposal")
			v := request.StorageValue()
			switch name {
			case "disabled":
				sqlCapture(t, fmt.Sprintf("UPDATE platform_core.accounts SET disabled=true WHERE id='%s'", f.owner.StorageValue().ID))
			case "version":
				v.OriginVersion++
				request = auth.RoomSecret(v)
			case "tools":
				f.policy.mu.Lock()
				f.policy.grants = false
				f.policy.mu.Unlock()
			case "tenant":
				v.Scope.WorkspaceID = "foreign"
				v.Binding.Workspace = "foreign"
				request = auth.RoomSecret(v)
			}
			if _, e := g.Execute(f.ctx, f.caller(f.owner, ""), request); e == nil || calls.Load() != 0 {
				t.Fatal("revoked or foreign action reached provider")
			}
		})
	}
}
func TestLostUnsavedProviderResultPausesInsteadOfRepeatingPaidOperation(t *testing.T) {
	var calls atomic.Int64
	f, g := configuredGateway(t, func(w http.ResponseWriter, r *http.Request) {
		calls.Add(1)
		v := readLocal(t, r)
		writeAnswer(w, v.Model, actionText(1))
	}, false, time.Second, 3)
	request := gatewayRequest(t, f, "same-durable-task", "proposal")
	out, e := g.Execute(f.ctx, f.caller(f.owner, ""), request)
	need(t, e)
	if out.StorageValue().Status != "complete" {
		t.Fatal("first operation unavailable")
	}
	out, e = g.Execute(f.ctx, f.caller(f.owner, ""), request)
	need(t, e)
	if out.StorageValue().Status != "paused" || calls.Load() != 1 {
		t.Fatal("unsaved result recovery repeated provider")
	}
}
func TestGatewayProtectedHandlesKeepProviderAndSeatStateOutOfDiagnostics(t *testing.T) {
	f, g := configuredGateway(t, func(w http.ResponseWriter, r *http.Request) { writeAnswer(w, "fixture:small", actionText(1)) }, false, time.Second, 3)
	request := gatewayRequest(t, f, "private-task-marker", "proposal")
	for _, value := range []any{*g, request} {
		for _, format := range []string{"%v", "%+v", "%#v", "%q", "%.*s"} {
			if bytes.Contains([]byte(fmt.Sprintf(format, value)), []byte("private-task-marker")) {
				t.Fatal("private task in diagnostics")
			}
		}
		if _, e := json.Marshal(value); e == nil {
			t.Fatal("private gateway value exported")
		}
	}
	ctx, cancel := context.WithCancel(f.ctx)
	cancel()
	if _, e := g.Execute(ctx, f.caller(f.owner, ""), request); e != auth.ErrDenied {
		t.Fatal("cancelled task dispatched")
	}
}
