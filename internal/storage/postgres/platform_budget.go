// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package postgres

import (
	"context"
	"encoding/json"
	"fmt"
	"io"

	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/budget"
	aicontext "github.com/zyc14588/TRPG_PLATFORM/internal/ai/context"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	store "github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
)

type PlatformBudgetStorage struct{ data **platformBudgetData }
type platformBudgetData struct{ repo *PlatformAuthRepository }
type platformBudgetTransaction struct{ data **platformBudgetTxData }
type platformBudgetTxData struct{ core *platformCoreTransaction }

func (PlatformBudgetStorage) Format(f fmt.State, _ rune) {
	_, _ = io.WriteString(f, "<private budget storage>")
}
func (PlatformBudgetStorage) MarshalJSON() ([]byte, error) { return nil, auth.ErrDenied }
func (platformBudgetTransaction) Format(f fmt.State, _ rune) {
	_, _ = io.WriteString(f, "<private budget transaction>")
}
func (platformBudgetTransaction) MarshalJSON() ([]byte, error) { return nil, auth.ErrDenied }
func (t *platformBudgetTransaction) state() *platformBudgetTxData {
	if t == nil || t.data == nil {
		return nil
	}
	return *t.data
}
func NewPlatformBudgetStorage(r *PlatformAuthRepository) (*PlatformBudgetStorage, error) {
	if r == nil || r.state() == nil {
		return nil, auth.ErrInvalid
	}
	d := &platformBudgetData{repo: r}
	return &PlatformBudgetStorage{data: &d}, nil
}
func budgetCore(tx core.Transaction) (*platformCoreTransaction, error) {
	if _, e := auth.RoomAdmissionSession(tx); e != nil {
		return nil, auth.ErrDenied
	}
	c, ok := auth.RoomStorageCore(tx).(*platformCoreTransaction)
	if !ok || c == nil || c.tx == nil {
		return nil, auth.ErrDenied
	}
	return c, nil
}
func (s *PlatformBudgetStorage) Bind(tx core.Transaction) (budget.Transaction, error) {
	if s == nil || s.data == nil || *s.data == nil {
		return nil, auth.ErrUnavailable
	}
	c, e := budgetCore(tx)
	if e != nil {
		return nil, e
	}
	d := &platformBudgetTxData{core: c}
	return &platformBudgetTransaction{data: &d}, nil
}
func budgetSubject(v aicontext.SubjectData) bool {
	return sessionTarget(v.Scope, v.Binding) && store.ValidID(v.SeatID) && store.ValidID(v.Controller) && store.ValidID(v.ConfigurationID) && checkpoint.IsDigest(v.ConfigurationHash) && v.PreparationRevision > 0 && v.ModelVersion > 0 && v.StateVersion > 0 && v.StateVersion < 1<<53 && v.EventCursor < 1<<53
}
func budgetJSON(v any) []byte { b, _ := json.Marshal(v); return b }
func (t *platformBudgetTransaction) Task(ctx context.Context, v aicontext.SubjectData, id string) (budget.TaskRecord, error) {
	if t.state() == nil || !budgetSubject(v) || !store.ValidID(id) {
		return budget.TaskRecord{}, auth.ErrDenied
	}
	var raw []byte
	var state string
	var value budget.TaskData
	e := t.state().core.tx.QueryRowContext(ctx, `SELECT body,status FROM platform_budget.tasks WHERE workspace_id=$1 AND room_id=$2 AND game_id=$3 AND session_id=$4 AND seat_id=$5 AND id=$6 FOR UPDATE`, v.Scope.WorkspaceID, v.Scope.RoomID, v.Scope.GameID, v.Binding.Session, v.SeatID, id).Scan(&raw, &state)
	if e != nil {
		return budget.TaskRecord{}, authStorageError(e)
	}
	if checkpoint.StrictDecode(raw, &value, 8192) != nil || value.ID != id || value.Subject != v || value.State != "open" {
		return budget.TaskRecord{}, auth.ErrDenied
	}
	value.State = state
	return auth.RoomSecret(value), nil
}
func (t *platformBudgetTransaction) Paused(ctx context.Context, v aicontext.SubjectData) (bool, error) {
	if t.state() == nil || !budgetSubject(v) {
		return false, auth.ErrDenied
	}
	var paused bool
	e := t.state().core.tx.QueryRowContext(ctx, `SELECT EXISTS(SELECT 1 FROM platform_budget.pauses WHERE workspace_id=$1 AND session_id=$2 AND seat_id=$3) OR EXISTS(SELECT 1 FROM platform_budget.reservations WHERE workspace_id=$1 AND session_id=$2 AND seat_id=$3 AND status IN('dispatched','uncertain'))`, v.Scope.WorkspaceID, v.Binding.Session, v.SeatID).Scan(&paused)
	return paused, authStorageError(e)
}
func (t *platformBudgetTransaction) InsertTask(ctx context.Context, record budget.TaskRecord) error {
	v := record.StorageValue()
	if t.state() == nil || !budgetSubject(v.Subject) || !store.ValidID(v.ID) || v.State != "open" {
		return auth.ErrInvalid
	}
	var count int
	if e := t.state().core.tx.QueryRowContext(ctx, `SELECT count(*) FROM platform_budget.tasks WHERE workspace_id=$1 AND session_id=$2 AND status IN('open','reserved','dispatched')`, v.Subject.Scope.WorkspaceID, v.Subject.Binding.Session).Scan(&count); e != nil {
		return authStorageError(e)
	}
	if count >= 64 {
		return auth.ErrDenied
	}
	s := v.Subject
	_, e := t.state().core.tx.ExecContext(ctx, `INSERT INTO platform_budget.tasks(workspace_id,room_id,game_id,session_id,seat_id,id,controller,graph_hash,body,status) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,'open')`, s.Scope.WorkspaceID, s.Scope.RoomID, s.Scope.GameID, s.Binding.Session, s.SeatID, v.ID, s.Controller, s.Binding.GraphHash, budgetJSON(v))
	return authStorageError(e)
}
func (t *platformBudgetTransaction) Reservation(ctx context.Context, task budget.TaskRecord) (budget.Reservation, error) {
	v := task.StorageValue()
	s := v.Subject
	if t.state() == nil || !budgetSubject(s) || !store.ValidID(v.ID) {
		return budget.Reservation{}, auth.ErrDenied
	}
	var raw, spent []byte
	var status string
	var r budget.ReservationData
	e := t.state().core.tx.QueryRowContext(ctx, `SELECT body,spent,status FROM platform_budget.reservations WHERE workspace_id=$1 AND room_id=$2 AND game_id=$3 AND session_id=$4 AND seat_id=$5 AND task_id=$6 FOR UPDATE`, s.Scope.WorkspaceID, s.Scope.RoomID, s.Scope.GameID, s.Binding.Session, s.SeatID, v.ID).Scan(&raw, &spent, &status)
	if e != nil {
		return budget.Reservation{}, authStorageError(e)
	}
	if checkpoint.StrictDecode(raw, &r, 8192) != nil || r.Task.ID != v.ID || r.Task.Subject != s || r.Task.Advice != v.Advice || !budget.Valid(r.Amount) || checkpoint.StrictDecode(spent, &r.Spent, 512) != nil || !budget.Valid(r.Spent) {
		return budget.Reservation{}, auth.ErrDenied
	}
	r.Status = status
	return auth.RoomSecret(r), nil
}

type budgetCell struct {
	level, node     string
	cap, used, held budget.Units
}

func cells(v budget.TaskData, c budget.Caps) []budgetCell {
	s := v.Subject
	return []budgetCell{{level: "workspace", node: s.Scope.WorkspaceID, cap: c.Workspace}, {level: "room", node: s.Scope.RoomID + "/" + s.Scope.GameID, cap: c.Room}, {level: "session", node: s.Binding.Session, cap: c.Session}, {level: "seat", node: s.Binding.Session + "/" + s.SeatID, cap: c.Seat}, {level: "task", node: s.Binding.Session + "/" + s.SeatID + "/" + v.ID, cap: c.Task}}
}
func (t *platformBudgetTransaction) lockCells(ctx context.Context, v budget.TaskData, c budget.Caps, create bool) ([]budgetCell, error) {
	out := cells(v, c)
	for i := range out {
		x := &out[i]
		if create {
			_, e := t.state().core.tx.ExecContext(ctx, `INSERT INTO platform_budget.counters(workspace_id,level,node_id,cap,used,held) VALUES($1,$2,$3,$4,$5,$5) ON CONFLICT DO NOTHING`, v.Subject.Scope.WorkspaceID, x.level, x.node, budgetJSON(x.cap), budgetJSON(budget.Units{}))
			if e != nil {
				return nil, authStorageError(e)
			}
		}
		var capRaw, used, held []byte
		e := t.state().core.tx.QueryRowContext(ctx, `SELECT cap,used,held FROM platform_budget.counters WHERE workspace_id=$1 AND level=$2 AND node_id=$3 FOR UPDATE`, v.Subject.Scope.WorkspaceID, x.level, x.node).Scan(&capRaw, &used, &held)
		if e != nil {
			return nil, authStorageError(e)
		}
		var cap budget.Units
		if checkpoint.StrictDecode(capRaw, &cap, 512) != nil || checkpoint.StrictDecode(used, &x.used, 512) != nil || checkpoint.StrictDecode(held, &x.held, 512) != nil || !budget.Valid(cap) || !budget.Valid(x.used) || !budget.Valid(x.held) {
			return nil, auth.ErrDenied
		}
		if create && cap != x.cap {
			return nil, auth.ErrConflict
		}
		x.cap = cap
	}
	return out, nil
}
func (t *platformBudgetTransaction) saveCells(ctx context.Context, w string, cs []budgetCell) error {
	for _, c := range cs {
		if !budget.CanReserve(c.used, c.held, budget.Units{}, c.cap) {
			return auth.ErrDenied
		}
		if e := platformAffected(t.state().core.tx.ExecContext(ctx, `UPDATE platform_budget.counters SET used=$4,held=$5 WHERE workspace_id=$1 AND level=$2 AND node_id=$3`, w, c.level, c.node, budgetJSON(c.used), budgetJSON(c.held))); e != nil {
			return auth.SafeError(e)
		}
	}
	return nil
}
func (t *platformBudgetTransaction) pause(ctx context.Context, v budget.TaskData, reason string) error {
	_, e := t.state().core.tx.ExecContext(ctx, `INSERT INTO platform_budget.pauses(workspace_id,session_id,seat_id,reason) VALUES($1,$2,$3,$4) ON CONFLICT DO NOTHING`, v.Subject.Scope.WorkspaceID, v.Subject.Binding.Session, v.Subject.SeatID, reason)
	return authStorageError(e)
}
func (t *platformBudgetTransaction) Reserve(ctx context.Context, record budget.Reservation, caps budget.Caps) (budget.Reservation, error) {
	r := record.StorageValue()
	if t.state() == nil || !budgetSubject(r.Task.Subject) || r.Status != "reserved" || !budget.Valid(r.Amount) || !budget.ValidCaps(caps) || r.Amount.Calls < 1 || r.PromptBytes > r.Amount.ContextBytes {
		return budget.Reservation{}, auth.ErrInvalid
	}
	current, e := t.Task(ctx, r.Task.Subject, r.Task.ID)
	if e != nil {
		return budget.Reservation{}, e
	}
	v := current.StorageValue()
	old, e := t.Reservation(ctx, current)
	if e == nil {
		if old.StorageValue().RequestHash != r.RequestHash || old.StorageValue().Amount != r.Amount {
			return budget.Reservation{}, auth.ErrConflict
		}
		return old, nil
	}
	if e != auth.ErrDenied {
		return budget.Reservation{}, e
	}
	if v.State != "open" {
		return budget.Reservation{}, auth.ErrDenied
	}
	paused, e := t.Paused(ctx, r.Task.Subject)
	if e != nil {
		return budget.Reservation{}, e
	}
	cs, e := t.lockCells(ctx, r.Task, caps, true)
	if e != nil {
		return budget.Reservation{}, e
	}
	for _, c := range cs {
		if !budget.CanReserve(c.used, c.held, r.Amount, c.cap) {
			paused = true
		}
	}
	if paused {
		r.Status = "paused"
		if e = t.pause(ctx, r.Task, "exhausted"); e != nil {
			return budget.Reservation{}, e
		}
	} else {
		for i := range cs {
			cs[i].held, e = budget.Add(cs[i].held, r.Amount)
			if e != nil {
				return budget.Reservation{}, auth.ErrDenied
			}
		}
		if e = t.saveCells(ctx, r.Task.Subject.Scope.WorkspaceID, cs); e != nil {
			return budget.Reservation{}, e
		}
	}
	s := r.Task.Subject
	_, e = t.state().core.tx.ExecContext(ctx, `INSERT INTO platform_budget.reservations(workspace_id,room_id,game_id,session_id,seat_id,task_id,body,spent,status) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9)`, s.Scope.WorkspaceID, s.Scope.RoomID, s.Scope.GameID, s.Binding.Session, s.SeatID, r.Task.ID, budgetJSON(r), budgetJSON(budget.Units{}), r.Status)
	if e != nil {
		return budget.Reservation{}, authStorageError(e)
	}
	if e = platformAffected(t.state().core.tx.ExecContext(ctx, `UPDATE platform_budget.tasks SET status=$7 WHERE workspace_id=$1 AND room_id=$2 AND game_id=$3 AND session_id=$4 AND seat_id=$5 AND id=$6 AND status='open'`, s.Scope.WorkspaceID, s.Scope.RoomID, s.Scope.GameID, s.Binding.Session, s.SeatID, r.Task.ID, r.Status)); e != nil {
		return budget.Reservation{}, auth.SafeError(e)
	}
	if e = t.state().core.inject(ctx, "budget-after-reservation"); e != nil {
		return budget.Reservation{}, auth.SafeError(e)
	}
	return auth.RoomSecret(r), nil
}
func (t *platformBudgetTransaction) Dispatch(ctx context.Context, record budget.Reservation) error {
	r := record.StorageValue()
	current, e := t.Reservation(ctx, auth.RoomSecret(r.Task))
	if e != nil {
		return e
	}
	c := current.StorageValue()
	if c.Status != "reserved" || c.RequestHash != r.RequestHash || c.Amount != r.Amount {
		return auth.ErrConflict
	}
	s := r.Task.Subject
	if e = platformAffected(t.state().core.tx.ExecContext(ctx, `UPDATE platform_budget.reservations SET status='dispatched' WHERE workspace_id=$1 AND room_id=$2 AND game_id=$3 AND session_id=$4 AND seat_id=$5 AND task_id=$6 AND status='reserved'`, s.Scope.WorkspaceID, s.Scope.RoomID, s.Scope.GameID, s.Binding.Session, s.SeatID, r.Task.ID)); e != nil {
		return auth.SafeError(e)
	}
	return auth.SafeError(platformAffected(t.state().core.tx.ExecContext(ctx, `UPDATE platform_budget.tasks SET status='dispatched' WHERE workspace_id=$1 AND session_id=$2 AND seat_id=$3 AND id=$4 AND status='reserved'`, s.Scope.WorkspaceID, s.Binding.Session, s.SeatID, r.Task.ID)))
}
func (t *platformBudgetTransaction) Settle(ctx context.Context, record budget.Reservation, spent budget.Units, known bool) (budget.Reservation, error) {
	r := record.StorageValue()
	current, e := t.Reservation(ctx, auth.RoomSecret(r.Task))
	if e != nil {
		return budget.Reservation{}, e
	}
	c := current.StorageValue()
	if c.RequestHash != r.RequestHash || c.Amount != r.Amount {
		return budget.Reservation{}, auth.ErrConflict
	}
	if c.Status == "settled" || c.Status == "uncertain" {
		if known && c.Status == "settled" && c.Spent == spent || !known && c.Status == "uncertain" {
			return current, nil
		}
		return budget.Reservation{}, auth.ErrConflict
	}
	if c.Status != "dispatched" {
		return budget.Reservation{}, auth.ErrDenied
	}
	if !known || !budget.Fits(spent, r.Amount) {
		c.Status = "uncertain"
		c.Spent = r.Amount
		if e = t.pause(ctx, r.Task, "uncertain"); e != nil {
			return budget.Reservation{}, e
		}
	} else {
		cs, e := t.lockCells(ctx, r.Task, budget.Caps{}, false)
		if e != nil {
			return budget.Reservation{}, e
		}
		for i := range cs {
			cs[i].held, e = budget.Sub(cs[i].held, r.Amount)
			if e != nil {
				return budget.Reservation{}, e
			}
			cs[i].used, e = budget.Add(cs[i].used, spent)
			if e != nil {
				return budget.Reservation{}, auth.ErrDenied
			}
		}
		if e = t.saveCells(ctx, r.Task.Subject.Scope.WorkspaceID, cs); e != nil {
			return budget.Reservation{}, e
		}
		c.Status = "settled"
		c.Spent = spent
	}
	s := r.Task.Subject
	if e = platformAffected(t.state().core.tx.ExecContext(ctx, `UPDATE platform_budget.reservations SET status=$7,spent=$8 WHERE workspace_id=$1 AND room_id=$2 AND game_id=$3 AND session_id=$4 AND seat_id=$5 AND task_id=$6 AND status='dispatched'`, s.Scope.WorkspaceID, s.Scope.RoomID, s.Scope.GameID, s.Binding.Session, s.SeatID, r.Task.ID, c.Status, budgetJSON(c.Spent))); e != nil {
		return budget.Reservation{}, auth.SafeError(e)
	}
	if e = platformAffected(t.state().core.tx.ExecContext(ctx, `UPDATE platform_budget.tasks SET status=$5 WHERE workspace_id=$1 AND session_id=$2 AND seat_id=$3 AND id=$4 AND status='dispatched'`, s.Scope.WorkspaceID, s.Binding.Session, s.SeatID, r.Task.ID, c.Status)); e != nil {
		return budget.Reservation{}, auth.SafeError(e)
	}
	if e = t.state().core.inject(ctx, "budget-after-settlement"); e != nil {
		return budget.Reservation{}, auth.SafeError(e)
	}
	return auth.RoomSecret(c), nil
}

var _ budget.Storage = (*PlatformBudgetStorage)(nil)
