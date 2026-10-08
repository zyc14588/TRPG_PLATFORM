//go:build linux && integration

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package model_gateway_test

import (
	"context"
	"encoding/hex"
	"encoding/json"
	"net/http"
	"strings"
	"sync/atomic"
	"testing"

	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/action"
	aicontext "github.com/zyc14588/TRPG_PLATFORM/internal/ai/context"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
	"github.com/zyc14588/TRPG_PLATFORM/internal/task"
)

func TestActualAIIntentReturnsThroughActorBeforeDurableNarrative(t *testing.T) {
	var calls atomic.Int64
	var f *actorGateway
	f = newActorGateway(t, func(w http.ResponseWriter, r *http.Request) {
		calls.Add(1)
		if f == nil || f.unlocked(r.Context()) != nil {
			t.Error("provider called while authority row locked")
		}
		v := readLocal(t, r)
		if strings.Contains(v.Messages[1].Content, loopPrivateValue) {
			t.Error("native private state reached model")
		}
		var p aicontext.PayloadData
		_ = json.Unmarshal([]byte(v.Messages[1].Content), &p)
		if p.Version == 2 {
			writeAnswer(w, v.Model, actionText(2))
		} else if p.Version == 3 {
			if f.counterValue(t) != "3" {
				t.Error("narrative began before action commitment")
			}
			writeAnswer(w, v.Model, "行动已执行。")
		} else {
			t.Error("unexpected durable phase")
			w.WriteHeader(500)
		}
	})
	f.source(t)
	if calls.Load() != 0 || f.version(t) != "2" {
		t.Fatal("source command synchronously waited for provider")
	}
	r := f.runtime(t)
	first, e := r.RunOnce(f.ctx)
	need(t, e)
	if first.Executed != 1 || first.Applied != 1 || f.counterValue(t) != "3" || f.version(t) != "3" || calls.Load() != 1 {
		t.Fatal("AI proposal did not use native authority")
	}
	second, e := r.RunOnce(f.ctx)
	need(t, e)
	if second.Executed != 1 || second.Applied != 1 || f.version(t) != "4" || calls.Load() != 2 {
		t.Fatal("narrative did not use distinct durable task")
	}
	if _, e := r.RunOnce(f.ctx); e != task.ErrNotFound {
		t.Fatal("completed provider task redispatched")
	}
}
func TestNarrativeFailureKeepsNativeEventAndFilteredDeterministicTemplate(t *testing.T) {
	var f *actorGateway
	f = newActorGateway(t, func(w http.ResponseWriter, r *http.Request) {
		v := readLocal(t, r)
		var p aicontext.PayloadData
		_ = json.Unmarshal([]byte(v.Messages[1].Content), &p)
		if p.Version == 2 {
			writeAnswer(w, v.Model, actionText(2))
			return
		}
		w.WriteHeader(503)
	})
	f.source(t)
	r := f.runtime(t)
	_, e := r.RunOnce(f.ctx)
	need(t, e)
	raw := f.receipt(t, "3")
	defer clear(raw)
	var receipt data.Receipt
	need(t, json.Unmarshal(raw, &receipt))
	committed, e := action.Committed(receipt, checkpoint.Object(map[string]checkpoint.Value{"counter": checkpoint.Int(3)}))
	need(t, e)
	_, e = r.RunOnce(f.ctx)
	need(t, e)
	if f.counterValue(t) != "3" || f.version(t) != "4" {
		t.Fatal("narrative failure rolled back accepted event")
	}
	// The actual durable narrative result must carry the filtered template;
	// a separate helper call cannot stand in for worker/Actor integration.
	resultRaw := sqlCapture(t, "SELECT encode(body,'hex') FROM platform_task.jobs WHERE workspace='"+f.scope.WorkspaceID+"' AND convert_from(body,'UTF8')::jsonb->'payload'->'table'->'mode'->>'string'='narrative'")
	resultBody, e := hex.DecodeString(strings.TrimSpace(string(resultRaw)))
	need(t, e)
	defer clear(resultBody)
	job, e := task.DecodeStored(resultBody)
	need(t, e)
	jobData, e := job.StorageValue()
	need(t, e)
	durableResult, e := jobData.Result.StorageValue()
	need(t, e)
	if durableResult.Table["status"].String != "paused" || !strings.Contains(durableResult.Table["narrative"].String, `"number":"3"`) || strings.Contains(durableResult.Table["narrative"].String, loopPrivateValue) {
		t.Fatal("durable narrative pause lost filtered committed template")
	}
	text, used, e := action.AfterCommit(context.Background(), committed, func(context.Context, action.Commit) (string, error) { return "", task.ErrFailed })
	need(t, e)
	if !used || strings.Contains(text, loopPrivateValue) || !strings.Contains(text, `"number":"3"`) {
		t.Fatal("failure template lost filtered committed result")
	}
}
func TestMutableAIAndOutboxKindsCannotReplaceCommittedSource(t *testing.T) {
	for _, name := range []string{"outbox-mismatch", "paired-relabel"} {
		t.Run(name, func(t *testing.T) {
			f := newActorGateway(t, func(w http.ResponseWriter, r *http.Request) {
				t.Error("relabelled source reached provider")
				w.WriteHeader(500)
			})
			f.source(t)
			scope := f.scope.WorkspaceID
			sqlCapture(t, "UPDATE host_command.outbox SET kind='dispatch-task' WHERE workspace='"+scope+"' AND kind='dispatch-ai'")
			if name == "paired-relabel" {
				sqlCapture(t, "UPDATE host_command.tasks SET kind='task' WHERE workspace='"+scope+"' AND kind='ai'")
			}
			_, e := f.tasks.Claim(f.ctx, f.worker)
			if name == "outbox-mismatch" {
				if e != task.ErrNotFound {
					t.Fatal("mismatched SQL pair collected")
				}
			} else if e != task.ErrDenied {
				t.Fatal("paired SQL relabel bypassed immutable source")
			}
			if f.version(t) != "2" {
				t.Fatal("rejected source changed native state")
			}
		})
	}
}
