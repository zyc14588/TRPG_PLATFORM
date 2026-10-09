// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package postgres

import (
	"context"
	"encoding/json"
	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/budget"
	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/model"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
)

var _ model.PresentationRegistry = (*platformModelTransaction)(nil)
var _ model.RoomPresentationRegistry = (*platformModelTransaction)(nil)

// Counter rows are complete native eight-dimensional records. A missing
// dimension must not acquire Go's implicit zero value and restore readiness.
func presentationBudgetUnits(raw []byte) (budget.Units, error) {
	var keys map[string]json.RawMessage
	if checkpoint.StrictDecode(raw, &keys, 512) != nil || len(keys) != 8 {
		return budget.Units{}, auth.ErrDenied
	}
	for _, name := range []string{"Calls", "Tokens", "CostMicros", "LatencyMillis", "Tools", "Subagents", "ContextBytes", "LocalComputeMillis"} {
		if _, ok := keys[name]; !ok {
			return budget.Units{}, auth.ErrDenied
		}
	}
	var units budget.Units
	if checkpoint.StrictDecode(raw, &units, 512) != nil || !budget.Valid(units) {
		return budget.Units{}, auth.ErrDenied
	}
	return units, nil
}

// Enumerate physical identities, then use the original strict point decoder so
// neither an unbound body nor a forged duplicate column can become authority.
func (t *platformModelTransaction) PresentationConfigurations(ctx context.Context, w, id string) ([]model.Configuration, error) {
	return t.presentationConfigurations(ctx, w, id, nil)
}

func (t *platformModelTransaction) RoomPresentationConfigurations(ctx context.Context, scope core.Scope, id string) ([]model.Configuration, error) {
	if !store.ValidID(scope.RoomID) || !store.ValidID(scope.GameID) {
		return nil, auth.ErrDenied
	}
	return t.presentationConfigurations(ctx, scope.WorkspaceID, id, &scope)
}

func (t *platformModelTransaction) presentationConfigurations(ctx context.Context, w, id string, scope *core.Scope) ([]model.Configuration, error) {
	if t.state() == nil || ctx == nil || ctx.Err() != nil || !store.ValidID(w) || !store.ValidID(id) {
		return nil, auth.ErrDenied
	}
	query := `SELECT room_id,game_id,seat_id,selection FROM platform_model.configurations WHERE workspace_id=$1 AND configuration_id=$2 ORDER BY room_id,game_id,seat_id,selection LIMIT 65`
	args := []any{w, id}
	if scope != nil {
		query = `SELECT room_id,game_id,seat_id,selection FROM platform_model.configurations WHERE workspace_id=$1 AND room_id=$2 AND game_id=$3 AND configuration_id=$4 ORDER BY seat_id,selection LIMIT 65`
		args = []any{w, scope.RoomID, scope.GameID, id}
	}
	rows, e := t.state().core.tx.QueryContext(ctx, query, args...)
	if e != nil {
		return nil, authStorageError(e)
	}
	type key struct{ room, game, seat, selection string }
	keys := []key{}
	for rows.Next() {
		var k key
		if e = rows.Scan(&k.room, &k.game, &k.seat, &k.selection); e != nil {
			_ = rows.Close()
			return nil, authStorageError(e)
		}
		keys = append(keys, k)
	}
	e = rows.Err()
	closeErr := rows.Close()
	if e != nil || closeErr != nil {
		return nil, auth.ErrUnavailable
	}
	if len(keys) > 64 {
		return nil, auth.ErrUnavailable
	}
	out := make([]model.Configuration, 0, len(keys))
	for _, k := range keys {
		c, e := t.Configuration(ctx, core.Scope{WorkspaceID: w, RoomID: k.room, GameID: k.game}, k.seat, k.selection)
		if e != nil {
			return nil, auth.SafeError(e)
		}
		out = append(out, c)
	}
	return out, nil
}

// Budget readiness reads the same durable pause/reservation/counter tables as
// the original gateway. Dispatched/uncertain usage is never optimistic. This
// conservative preview does not reserve, clear a pause, settle or dispatch.
// Actual launch and dispatch still recheck and reserve their current limits.
func (t *platformModelTransaction) PresentationBudgetReady(ctx context.Context, row model.Configuration) (bool, error) {
	c := row.StorageValue()
	if t.state() == nil || ctx == nil || ctx.Err() != nil || c.Revoked || !store.ValidID(c.Scope.WorkspaceID) || !store.ValidID(c.Scope.RoomID) || !store.ValidID(c.Scope.GameID) || !store.ValidID(c.SeatID) || c.Version < 1 {
		return false, auth.ErrDenied
	}
	var paused bool
	e := t.state().core.tx.QueryRowContext(ctx, `SELECT EXISTS(SELECT 1 FROM platform_budget.reservations WHERE workspace_id=$1 AND room_id=$2 AND game_id=$3 AND seat_id=$4 AND status IN('dispatched','uncertain','paused')) OR EXISTS(SELECT 1 FROM platform_budget.pauses p JOIN platform_launch.sessions s ON (s.workspace_id,s.session_id)=(p.workspace_id,p.session_id) WHERE p.workspace_id=$1 AND s.room_id=$2 AND s.game_id=$3 AND p.seat_id=$4)`, c.Scope.WorkspaceID, c.Scope.RoomID, c.Scope.GameID, c.SeatID).Scan(&paused)
	if e != nil {
		return false, authStorageError(e)
	}
	if paused {
		return false, nil
	}
	rows, e := t.state().core.tx.QueryContext(ctx, `SELECT cap,used,held FROM platform_budget.counters WHERE workspace_id=$1 AND ((level='workspace' AND node_id=$1) OR (level='room' AND node_id=$2) OR (level='session' AND node_id IN(SELECT session_id FROM platform_launch.sessions WHERE workspace_id=$1 AND room_id=$3 AND game_id=$4)) OR (level='seat' AND node_id IN(SELECT session_id||'/'||$5 FROM platform_launch.sessions WHERE workspace_id=$1 AND room_id=$3 AND game_id=$4))) ORDER BY level,node_id LIMIT 257`, c.Scope.WorkspaceID, c.Scope.RoomID+"/"+c.Scope.GameID, c.Scope.RoomID, c.Scope.GameID, c.SeatID)
	if e != nil {
		return false, authStorageError(e)
	}
	defer rows.Close()
	amount := budget.Units{Calls: c.Budget.Calls, Tokens: c.Budget.Tokens, CostMicros: c.Budget.CostMicros, LatencyMillis: c.Budget.LatencyMillis, Tools: c.Budget.Tools, Subagents: c.Budget.Subagents, ContextBytes: c.Budget.ContextBytes, LocalComputeMillis: c.Budget.LocalComputeMillis}
	ready, count := true, 0
	for rows.Next() {
		count++
		if count > 256 {
			return false, auth.ErrUnavailable
		}
		var a, b, h []byte
		var cap, used, held budget.Units
		if e = rows.Scan(&a, &b, &h); e != nil {
			return false, authStorageError(e)
		}
		var capError, usedError, heldError error
		cap, capError = presentationBudgetUnits(a)
		used, usedError = presentationBudgetUnits(b)
		held, heldError = presentationBudgetUnits(h)
		if capError != nil || usedError != nil || heldError != nil {
			return false, auth.ErrDenied
		}
		ready = ready && budget.CanReserve(used, held, amount, cap)
	}
	if e = rows.Err(); e != nil {
		return false, authStorageError(e)
	}
	return ready, nil
}
