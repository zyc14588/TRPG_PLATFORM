// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package postgres

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"slices"

	aicontext "github.com/zyc14588/TRPG_PLATFORM/internal/ai/context"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
)

type PlatformAIContextStorage struct{ data **platformBudgetData }
type platformAIContextTransaction struct{ data **platformBudgetTxData }

func (PlatformAIContextStorage) Format(f fmt.State, _ rune) {
	_, _ = io.WriteString(f, "<private AI context storage>")
}
func (PlatformAIContextStorage) MarshalJSON() ([]byte, error) { return nil, auth.ErrDenied }
func (platformAIContextTransaction) Format(f fmt.State, _ rune) {
	_, _ = io.WriteString(f, "<private AI context transaction>")
}
func (platformAIContextTransaction) MarshalJSON() ([]byte, error) { return nil, auth.ErrDenied }
func NewPlatformAIContextStorage(r *PlatformAuthRepository) (*PlatformAIContextStorage, error) {
	if r == nil || r.state() == nil {
		return nil, auth.ErrInvalid
	}
	d := &platformBudgetData{repo: r}
	return &PlatformAIContextStorage{data: &d}, nil
}
func (s *PlatformAIContextStorage) Bind(tx core.Transaction) (aicontext.Transaction, error) {
	if s == nil || s.data == nil || *s.data == nil {
		return nil, auth.ErrUnavailable
	}
	c, e := budgetCore(tx)
	if e != nil {
		return nil, e
	}
	d := &platformBudgetTxData{core: c}
	return &platformAIContextTransaction{data: &d}, nil
}
func (t *platformAIContextTransaction) state() *platformBudgetTxData {
	if t == nil || t.data == nil {
		return nil
	}
	return *t.data
}
func (t *platformAIContextTransaction) Snapshot(ctx context.Context, scope core.Scope, b data.Binding) (aicontext.Snapshot, error) {
	if t.state() == nil || !sessionTarget(scope, b) {
		return aicontext.Snapshot{}, auth.ErrDenied
	}
	v := aicontext.SnapshotData{Binding: b}
	var raw []byte
	e := t.state().core.tx.QueryRowContext(ctx, `SELECT h.version,h.event_sequence,h.state FROM platform_launch.sessions l JOIN platform_room.rooms r USING(workspace_id,room_id,game_id) JOIN host_command.sessions h ON h.workspace=l.workspace_id AND h.session=l.session_id AND h.graph_hash=l.graph_hash WHERE l.workspace_id=$1 AND l.room_id=$2 AND l.game_id=$3 AND l.session_id=$4 AND l.graph_hash=$5 AND r.state='launched' AND NOT EXISTS(SELECT 1 FROM host_command.endings e WHERE e.workspace=h.workspace AND e.session=h.session) FOR SHARE OF h`, scope.WorkspaceID, scope.RoomID, scope.GameID, b.Session, b.GraphHash).Scan(&v.Version, &v.Cursor, &raw)
	if e != nil {
		return aicontext.Snapshot{}, authStorageError(e)
	}
	if checkpoint.StrictDecode(raw, &v.State, checkpoint.MaxBytes*2) != nil || checkpoint.Validate(v.State) != nil || v.Version < 1 || v.Version >= 1<<53 || v.Cursor >= 1<<53 {
		return aicontext.Snapshot{}, auth.ErrDenied
	}
	// Reading only the current bounded window is conservative: a memory whose
	// source falls outside this window is omitted rather than exposed unchecked.
	rows, e := t.state().core.tx.QueryContext(ctx, `SELECT sequence,version,command_id,event_id,event_type,payload,schema_version,schema_hash FROM host_command.events WHERE workspace=$1 AND session=$2 AND sequence<=$3 ORDER BY sequence DESC LIMIT 129`, b.Workspace, b.Session, v.Cursor)
	if e != nil {
		return aicontext.Snapshot{}, authStorageError(e)
	}
	defer rows.Close()
	size := 0
	for rows.Next() {
		var x data.JournalEvent
		var payload []byte
		if e = rows.Scan(&x.Sequence, &x.Version, &x.CommandID, &x.Event.ID, &x.Event.Type, &payload, &x.Event.SchemaVersion, &x.Event.SchemaHash); e != nil {
			return aicontext.Snapshot{}, authStorageError(e)
		}
		if len(v.Events) == 128 {
			break
		}
		size += len(payload)
		if size > 256<<10 || x.Sequence < 1 || x.Sequence > v.Cursor || x.Version > v.Version || checkpoint.StrictDecode(payload, &x.Event.Payload, checkpoint.MaxBytes*2) != nil || checkpoint.Validate(x.Event.Payload) != nil {
			return aicontext.Snapshot{}, auth.ErrDenied
		}
		v.Events = append(v.Events, x)
	}
	if e = rows.Err(); e != nil {
		return aicontext.Snapshot{}, authStorageError(e)
	}
	slices.Reverse(v.Events)
	return auth.RoomSecret(v), nil
}
func (t *platformAIContextTransaction) Memories(ctx context.Context, subject aicontext.Subject) ([]aicontext.Memory, error) {
	v := subject.StorageValue()
	if t.state() == nil || !budgetSubject(v) {
		return nil, auth.ErrDenied
	}
	rows, e := t.state().core.tx.QueryContext(ctx, `SELECT id,generator_version,fact_level,body FROM platform_budget.memories WHERE workspace_id=$1 AND room_id=$2 AND game_id=$3 AND session_id=$4 AND seat_id=$5 AND graph_hash=$6 ORDER BY id LIMIT 65`, v.Scope.WorkspaceID, v.Scope.RoomID, v.Scope.GameID, v.Binding.Session, v.SeatID, v.Binding.GraphHash)
	if e != nil {
		return nil, authStorageError(e)
	}
	defer rows.Close()
	out := []aicontext.Memory{}
	for rows.Next() {
		var id, generator, level string
		var raw []byte
		var m aicontext.MemoryData
		if e = rows.Scan(&id, &generator, &level, &raw); e != nil {
			return nil, authStorageError(e)
		}
		if len(out) == 64 || checkpoint.StrictDecode(raw, &m, 8192) != nil || m.ID != id || m.GeneratorVersion != generator || m.FactLevel != level {
			return nil, auth.ErrDenied
		}
		m.Sources = slices.Clone(m.Sources)
		out = append(out, auth.RoomSecret(m))
	}
	return out, authStorageError(rows.Err())
}
func (t *platformAIContextTransaction) PutMemory(ctx context.Context, subject aicontext.Subject, memory aicontext.Memory) error {
	v := subject.StorageValue()
	m := memory.StorageValue()
	if t.state() == nil || !budgetSubject(v) {
		return auth.ErrDenied
	}
	raw, e := json.Marshal(m)
	if e != nil || len(raw) > 8192 {
		return auth.ErrInvalid
	}
	_, e = t.state().core.tx.ExecContext(ctx, `INSERT INTO platform_budget.memories(workspace_id,room_id,game_id,session_id,seat_id,graph_hash,id,generator_version,fact_level,body) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10) ON CONFLICT DO NOTHING`, v.Scope.WorkspaceID, v.Scope.RoomID, v.Scope.GameID, v.Binding.Session, v.SeatID, v.Binding.GraphHash, m.ID, m.GeneratorVersion, m.FactLevel, raw)
	if e != nil {
		return authStorageError(e)
	}
	var old []byte
	e = t.state().core.tx.QueryRowContext(ctx, `SELECT body FROM platform_budget.memories WHERE workspace_id=$1 AND room_id=$2 AND game_id=$3 AND session_id=$4 AND seat_id=$5 AND graph_hash=$6 AND id=$7`, v.Scope.WorkspaceID, v.Scope.RoomID, v.Scope.GameID, v.Binding.Session, v.SeatID, v.Binding.GraphHash, m.ID).Scan(&old)
	if e != nil {
		return authStorageError(e)
	}
	if !slices.Equal(raw, old) {
		return auth.ErrConflict
	}
	return auth.SafeError(t.state().core.inject(ctx, "budget-after-memory"))
}

var _ aicontext.Storage = (*PlatformAIContextStorage)(nil)
