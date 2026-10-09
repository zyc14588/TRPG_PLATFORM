//go:build linux && integration

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package fixture

import (
	"context"
	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/model"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
	"testing"
)

// This trusted test fault preserves the physical transaction and original point
// decoder. It delays only its new enumeration until the actual read deadline.
type delayedModelStorage struct{ model.Storage }

func (s delayedModelStorage) Bind(tx core.Transaction) (model.Transaction, error) {
	mt, e := s.Storage.Bind(tx)
	if e != nil {
		return nil, e
	}
	return delayedModelTx{mt}, nil
}

type delayedModelTx struct{ model.Transaction }

func (t delayedModelTx) PresentationConfigurations(ctx context.Context, w, id string) ([]model.Configuration, error) {
	<-ctx.Done()
	return nil, ctx.Err()
}
func (t delayedModelTx) PresentationBudgetReady(ctx context.Context, c model.Configuration) (bool, error) {
	return t.Transaction.(model.PresentationRegistry).PresentationBudgetReady(ctx, c)
}
func (h *Harness) DelayReadUntilDeadline(t *testing.T) {
	opts := h.modelOptions
	opts.Storage = delayedModelStorage{opts.Storage}
	var e error
	h.Models, e = model.New(opts)
	need(t, e)
	h.models = h.Models
	h.recomposeLaunch(t, h.configs)
	h.composePlayer(t)
	h.Compose(t)
}
func (h *Harness) CorruptBudget(t *testing.T) {
	h.ExhaustBudget(t)
	_ = h.sql(t, "UPDATE platform_budget.counters SET used=decode('7b7d','hex') WHERE workspace_id='"+h.w+"'")
}
func (h *Harness) OpenTransactionCount(t *testing.T) string {
	return h.sql(t, "SELECT count(*) FROM pg_stat_activity WHERE datname=current_database() AND backend_type='client backend' AND state='idle in transaction'")
}
