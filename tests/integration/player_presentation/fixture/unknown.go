//go:build linux && integration

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package fixture

import (
	"context"
	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/budget"
	aicontext "github.com/zyc14588/TRPG_PLATFORM/internal/ai/context"
	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/model"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/postgres"
	"strings"
	"testing"
)

// Use the original durable budget transitions against an actually launched
// production-Lua session. No provider is called to manufacture unknown usage.
func (h *Harness) MarkUnknownBilling(t *testing.T) {
	h.launchConfigured(t)
	owner := h.owner.StorageValue()
	stored, e := h.Models.Read(h.ctx, auth.RoomSecret(model.CallerData{Credential: owner.Cookie}), auth.RoomSecret(model.TargetData{Scope: h.Scope(), SeatID: "ai", ID: "selected"}))
	need(t, e)
	c := stored.StorageValue()
	storage, e := postgres.NewPlatformBudgetStorage(h.r)
	need(t, e)
	need(t, h.authority.Inspect(h.ctx, owner.Cookie, "", false, func(ctx context.Context, tx auth.Transaction, _ auth.SessionData) error {
		bt, e := storage.Bind(tx.Core())
		if e != nil {
			return e
		}
		subject := aicontext.SubjectData{Scope: h.Scope(), Binding: data.Binding{Workspace: h.w, Session: h.sessionID, GraphHash: c.GraphHash}, SeatID: "ai", Controller: owner.ID, ConfigurationID: c.ConfigurationID, ConfigurationHash: c.ConfigurationHash, PreparationRevision: c.PreparationRevision, ModelVersion: c.Version, StateVersion: 1, Tuple: c.Tuple, Budget: c.Budget}
		task := budget.TaskData{ID: strings.Repeat("a", 32), Subject: subject, State: "open", Advice: false}
		if e = bt.InsertTask(ctx, auth.RoomSecret(task)); e != nil {
			return e
		}
		v := Limits()
		cap := budget.Units{Calls: v.Calls, Tokens: v.Tokens, CostMicros: v.CostMicros, LatencyMillis: v.LatencyMillis, Tools: v.Tools, Subagents: v.Subagents, ContextBytes: v.ContextBytes, LocalComputeMillis: v.LocalComputeMillis}
		reservation, e := bt.Reserve(ctx, auth.RoomSecret(budget.ReservationData{Task: task, Amount: budget.Units{Calls: 1, Tokens: 1, CostMicros: 1, LatencyMillis: 1, ContextBytes: 1}, PromptBytes: 1, RequestHash: checkpoint.Hash([]byte("owned unknown result")), Status: "reserved"}), budget.Caps{Workspace: cap, Room: cap, Session: cap, Seat: cap, Task: cap})
		if e != nil {
			return e
		}
		if e = bt.Dispatch(ctx, reservation); e != nil {
			return e
		}
		_, e = bt.Settle(ctx, reservation, budget.Units{}, false)
		return e
	}))
}
