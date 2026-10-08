//go:build linux && integration

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package player_test

import (
	"context"
	"encoding/json"
	"net/http"
	"strings"
	"sync/atomic"
	"testing"
	"time"

	aicontext "github.com/zyc14588/TRPG_PLATFORM/internal/ai/context"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	playerapi "github.com/zyc14588/TRPG_PLATFORM/internal/platform/player"
	platformsession "github.com/zyc14588/TRPG_PLATFORM/internal/platform/session"
	"github.com/zyc14588/TRPG_PLATFORM/internal/task"
)

func TestPlayerPauseRetainsActualAIProposalAndNarrativeWithoutDispatchOrBilling(t *testing.T) {
	var calls atomic.Int64
	var f *actualAI
	f = newActualAI(t, func(w http.ResponseWriter, r *http.Request) {
		calls.Add(1)
		if f == nil || f.unlocked(r.Context()) != nil {
			t.Error("provider called while native authority locked")
		}
		v := readAI(t, r)
		if len(v.Messages) != 2 {
			return
		}
		if strings.Contains(v.Messages[1].Content, PrivateValue) {
			t.Error("private human state reached model")
		}
		var p aicontext.PayloadData
		if json.Unmarshal([]byte(v.Messages[1].Content), &p) != nil {
			t.Error("current native context missing")
			return
		}
		switch p.Version {
		case 2:
			writeAI(w, v.Model, proposalText(2))
		case 3:
			if f.sql(t, "SELECT version FROM host_command.sessions WHERE workspace='"+f.w+"'") != "3" {
				t.Error("narrative preceded durable action")
			}
			writeAI(w, v.Model, "行动已执行。")
		default:
			t.Error("unexpected durable AI phase")
			w.WriteHeader(500)
		}
	})
	f.source(t)
	r := f.runtime(t)
	f.pauseNow(t)
	for i := 0; i < 2; i++ {
		if _, e := r.RunOnce(f.ctx); e != task.ErrNotFound {
			t.Fatal("paused AI task was claimed")
		}
	}
	if calls.Load() != 0 || f.version(t) != "2" || f.sql(t, "SELECT coalesce(sum(attempt),0) FROM platform_task.jobs WHERE workspace='"+f.w+"'") != "0" || f.sql(t, "SELECT count(*) FROM platform_budget.reservations WHERE workspace_id='"+f.w+"'") != "0" {
		t.Fatal("paused proposal dispatched, billed, attempted or mutated")
	}
	f.resumeAll(t)
	first, e := r.RunOnce(f.ctx)
	need(t, e)
	if first.Executed != 1 || first.Applied != 1 || calls.Load() != 1 || f.version(t) != "3" {
		t.Fatal("resumed proposal failed native Actor round trip")
	}
	f.pauseNow(t)
	before := f.sql(t, "SELECT md5(string_agg(encode(used,'hex')||encode(held,'hex'),',' ORDER BY level,node_id)) FROM platform_budget.counters WHERE workspace_id='"+f.w+"'")
	f.restart(t)
	if !f.snapshot(t, f.owner, f.hostConnection, "0", 8)["control"].(map[string]any)["paused"].(bool) {
		t.Fatal("restart lost durable AI pause")
	}
	if _, e := r.RunOnce(f.ctx); e != task.ErrNotFound {
		t.Fatal("paused narrative was claimed")
	}
	after := f.sql(t, "SELECT md5(string_agg(encode(used,'hex')||encode(held,'hex'),',' ORDER BY level,node_id)) FROM platform_budget.counters WHERE workspace_id='"+f.w+"'")
	if before != after || calls.Load() != 1 || f.version(t) != "3" {
		t.Fatal("paused narrative changed billing or native state")
	}
	// Recompose the internal delivery transport after server restart. The actual
	// queue and source remain in PostgreSQL, with their original immutable IDs.
	completion, e := platformsession.NewContinuations(platformsession.ContinuationOptions{Launch: f.service, Storage: f.tasks, Policies: f.policies})
	need(t, e)
	f.completion = completion
	f.resumeAll(t)
	r = f.runtime(t)
	second, e := r.RunOnce(f.ctx)
	need(t, e)
	if second.Executed != 1 || second.Applied != 1 || calls.Load() != 2 || f.version(t) != "4" {
		t.Fatal("resumed narrative failed distinct native Actor round trip")
	}
	if _, e := r.RunOnce(f.ctx); e != task.ErrNotFound {
		t.Fatal("completed AI task redispatched")
	}
	private(t, f.snapshot(t, f.participant, f.playerConnection, "0", 8), false)
}

func TestPlayerPauseAcknowledgmentCancelsInFlightProviderAndFencesFallbackRetry(t *testing.T) {
	var calls atomic.Int64
	started := make(chan struct{}, 1)
	f := newActualAI(t, func(w http.ResponseWriter, r *http.Request) {
		calls.Add(1)
		_ = readAI(t, r)
		select {
		case started <- struct{}{}:
		default:
		}
		select {
		case <-r.Context().Done():
		case <-time.After(3 * time.Second):
			w.WriteHeader(503)
		}
	})
	f.source(t)
	runtime := f.runtime(t)
	finished := make(chan error, 1)
	go func() { _, e := runtime.RunOnce(f.ctx); finished <- e }()
	select {
	case <-started:
	case <-time.After(3 * time.Second):
		t.Fatal("owned provider did not start")
	}
	if f.unlocked(f.ctx) != nil {
		t.Fatal("external provider held native SQL transaction")
	}
	f.pauseNow(t)
	select {
	case e := <-finished:
		if e == nil {
			t.Fatal("cancelled provider completed continuation")
		}
	case <-time.After(3 * time.Second):
		t.Fatal("pause did not quiesce admitted provider")
	}
	if f.version(t) != "2" || calls.Load() != 1 {
		t.Fatal("pause acknowledgment allowed provider fallback or native mutation")
	}
	before := f.sql(t, "SELECT md5(string_agg(encode(body,'hex'),',' ORDER BY task_id)) FROM platform_task.jobs WHERE workspace='"+f.w+"'")
	billing := f.sql(t, "SELECT md5(string_agg(encode(used,'hex')||encode(held,'hex'),',' ORDER BY level,node_id)) FROM platform_budget.counters WHERE workspace_id='"+f.w+"'")
	for i := 0; i < 3; i++ {
		if _, e := runtime.RunOnce(f.ctx); e != task.ErrNotFound {
			t.Fatal("paused in-flight task was retried")
		}
	}
	if before != f.sql(t, "SELECT md5(string_agg(encode(body,'hex'),',' ORDER BY task_id)) FROM platform_task.jobs WHERE workspace='"+f.w+"'") || billing != f.sql(t, "SELECT md5(string_agg(encode(used,'hex')||encode(held,'hex'),',' ORDER BY level,node_id)) FROM platform_budget.counters WHERE workspace_id='"+f.w+"'") || calls.Load() != 1 || f.version(t) != "2" {
		t.Fatal("post-ack retry changed durable task, billing, provider or Actor")
	}
}

func TestPlayerPauseGuardsActualGenericTaskRetryAndContinuation(t *testing.T) {
	n := newPlayerFixture(t, false)
	n.configureAndLaunch(t)
	pre, e := n.service.PlayerLobby(n.ctx, n.caller(n.owner, ""), n.w, n.room)
	need(t, e)
	prep := pre.StorageValue().Preparation
	storage, worker := playerWorker(t, n.launchFixture)
	var calls atomic.Int64
	policy := task.Policy{GraphHash: prep.GraphHash, ConfigurationHash: prep.ConfigurationHash, PackageID: PackageID,
		ValidateInput: func(v checkpoint.Value) error {
			if v.Kind != "table" || len(v.Table) != 1 || v.Table["value"].Kind != "integer" {
				return task.ErrInvalid
			}
			return nil
		},
		ValidateResult: func(v checkpoint.Value) error {
			if v.Kind != "integer" || v.Number != "2" {
				return task.ErrInvalid
			}
			return nil
		},
		Execute: func(ctx context.Context, _ task.Value) (task.Value, error) {
			if ctx.Err() != nil {
				return task.Value{}, task.ErrDenied
			}
			if calls.Add(1) == 1 {
				return task.Value{}, task.ErrFailed
			}
			return task.NewValue(checkpoint.Int(2))
		},
		Inputs: func(ctx context.Context, _ task.Job) (task.Inputs, error) {
			if ctx.Err() != nil {
				return task.Inputs{}, task.ErrDenied
			}
			return task.NewInputs(time.Now().UnixMilli(), nil)
		}}
	facade, policies, e := playerapi.BindTasks(storage, n.control, []task.Policy{policy})
	need(t, e)
	completion, e := platformsession.NewContinuations(platformsession.ContinuationOptions{Launch: n.service, Storage: storage, Policies: []task.Policy{policy}})
	need(t, e)
	runtime, e := task.NewWorker(task.WorkerOptions{Identity: worker, Storage: facade, Policies: policies, Post: completion.Post, MaxActive: 1, ExternalTimeout: time.Second, PostTimeout: 3 * time.Second, Poll: 50 * time.Millisecond})
	need(t, e)
	n.api(t, n.owner, "command", n.commandFields(n.hostConnection, "player-generic-source", "1"))
	if _, e = runtime.RunOnce(n.ctx); e != task.ErrFailed {
		t.Fatal("generic task did not exercise real retry path")
	}
	n.pauseNow(t)
	before := n.sql(t, "SELECT md5(string_agg(encode(body,'hex'),',' ORDER BY task_id)) FROM platform_task.jobs WHERE workspace='"+n.w+"'")
	if _, e = runtime.RunOnce(n.ctx); e != task.ErrNotFound {
		t.Fatal("paused generic retry was claimed")
	}
	if calls.Load() != 1 || before != n.sql(t, "SELECT md5(string_agg(encode(body,'hex'),',' ORDER BY task_id)) FROM platform_task.jobs WHERE workspace='"+n.w+"'") || n.sql(t, "SELECT version FROM host_command.sessions WHERE workspace='"+n.w+"'") != "2" {
		t.Fatal("paused generic retry changed callback or Actor")
	}
	n.resumeAll(t)
	report, e := runtime.RunOnce(n.ctx)
	need(t, e)
	if report.Executed != 1 || report.Applied != 1 || calls.Load() != 2 || n.sql(t, "SELECT version FROM host_command.sessions WHERE workspace='"+n.w+"'") != "3" {
		t.Fatal("resumed generic task failed native continuation")
	}
}
