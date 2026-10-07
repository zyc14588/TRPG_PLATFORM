// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package postgres

import (
	"context"
	"database/sql"
	"encoding/json"
	"fmt"
	"io"
	"math"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
	platformsession "github.com/zyc14588/TRPG_PLATFORM/internal/platform/session"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
)

type PlatformSessionStorage struct{ data **platformSessionData }
type platformSessionData struct{ repo *PlatformAuthRepository }

func NewPlatformSessionStorage(repo *PlatformAuthRepository) (*PlatformSessionStorage, error) {
	if repo == nil || repo.state() == nil {
		return nil, auth.ErrInvalid
	}
	d := &platformSessionData{repo: repo}
	return &PlatformSessionStorage{data: &d}, nil
}
func (PlatformSessionStorage) Format(f fmt.State, _ rune) {
	_, _ = io.WriteString(f, "<private session storage>")
}
func (PlatformSessionStorage) MarshalJSON() ([]byte, error) { return nil, auth.ErrDenied }
func (s *PlatformSessionStorage) state() *platformSessionData {
	if s == nil || s.data == nil {
		return nil
	}
	return *s.data
}
func sessionTarget(scope core.Scope, b data.Binding) bool {
	return store.ValidID(scope.WorkspaceID) && store.ValidID(scope.RoomID) && store.ValidID(scope.GameID) && b.Workspace == scope.WorkspaceID && validBinding(b)
}
func (s *PlatformSessionStorage) begin(ctx context.Context, scope core.Scope, b data.Binding) (*sql.Tx, platformsession.PageData, string, error) {
	v := platformsession.PageData{Binding: b}
	if s.state() == nil || ctx == nil || ctx.Err() != nil || !sessionTarget(scope, b) {
		return nil, v, "", auth.ErrDenied
	}
	tx, e := s.state().repo.state().core.state().db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelRepeatableRead, ReadOnly: true})
	if e != nil {
		return nil, v, "", authStorageError(e)
	}
	if _, e = tx.ExecContext(ctx, `SET LOCAL statement_timeout='2s'`); e != nil {
		_ = tx.Rollback()
		return nil, v, "", authStorageError(e)
	}
	var schema string
	e = tx.QueryRowContext(ctx, `SELECT h.version,h.event_sequence,h.schema_hash,EXISTS(SELECT 1 FROM host_command.endings e WHERE e.workspace=h.workspace AND e.session=h.session) FROM platform_launch.sessions l JOIN platform_room.rooms r ON r.workspace_id=l.workspace_id AND r.room_id=l.room_id AND r.game_id=l.game_id JOIN host_command.sessions h ON h.workspace=l.workspace_id AND h.session=l.session_id AND h.graph_hash=l.graph_hash WHERE l.workspace_id=$1 AND l.room_id=$2 AND l.game_id=$3 AND l.session_id=$4 AND l.graph_hash=$5 AND r.state='launched'`, scope.WorkspaceID, scope.RoomID, scope.GameID, b.Session, b.GraphHash).Scan(&v.Version, &v.Cursor, &schema, &v.Ended)
	if e != nil {
		_ = tx.Rollback()
		return nil, v, "", authStorageError(e)
	}
	return tx, v, schema, nil
}

// Raw events exist only in this protected trusted storage handle. The service
// filters them by the current requester before returning any export handle.
func (s *PlatformSessionStorage) ReadPage(ctx context.Context, scope core.Scope, b data.Binding, after uint64, limit int) (platformsession.Page, error) {
	if after >= math.MaxInt64 || limit < 1 || limit > 128 {
		return platformsession.Page{}, auth.ErrInvalid
	}
	tx, v, _, e := s.begin(ctx, scope, b)
	if e != nil {
		return platformsession.Page{}, e
	}
	defer tx.Rollback()
	if after > v.Cursor {
		return platformsession.Page{}, auth.ErrConflict
	}
	v.NextCursor = after
	v.Events = []data.JournalEvent{}
	rows, e := tx.QueryContext(ctx, `SELECT sequence,version,command_id,event_id,event_type,payload,schema_version,schema_hash FROM host_command.events WHERE workspace=$1 AND session=$2 AND sequence>$3 AND sequence<=$4 ORDER BY sequence LIMIT $5`, b.Workspace, b.Session, after, v.Cursor, limit+1)
	if e != nil {
		return platformsession.Page{}, authStorageError(e)
	}
	defer rows.Close()
	size := 0
	for rows.Next() {
		var event data.JournalEvent
		var raw []byte
		if e = rows.Scan(&event.Sequence, &event.Version, &event.CommandID, &event.Event.ID, &event.Event.Type, &raw, &event.Event.SchemaVersion, &event.Event.SchemaHash); e != nil {
			return platformsession.Page{}, authStorageError(e)
		}
		if len(v.Events) == limit || size+len(raw) > 256<<10 {
			if len(v.Events) == 0 {
				return platformsession.Page{}, platformsession.ErrBackpressure
			}
			v.More = true
			break
		}
		if event.Sequence <= v.NextCursor || event.Sequence > v.Cursor || event.Version > v.Version || !store.ValidID(event.CommandID) || !store.ValidID(event.Event.ID) || checkpoint.StrictDecode(raw, &event.Event.Payload, 256<<10) != nil || checkpoint.Validate(event.Event.Payload) != nil {
			return platformsession.Page{}, auth.ErrDenied
		}
		size += len(raw)
		v.Events = append(v.Events, event)
		v.NextCursor = event.Sequence
	}
	if e = rows.Err(); e != nil {
		return platformsession.Page{}, authStorageError(e)
	}
	if e = rows.Close(); e != nil {
		return platformsession.Page{}, authStorageError(e)
	}
	if e = tx.Commit(); e != nil {
		return platformsession.Page{}, authStorageError(e)
	}
	return auth.RoomSecret(v), nil
}
func (s *PlatformSessionStorage) ReadPoint(ctx context.Context, scope core.Scope, b data.Binding) (platformsession.Point, error) {
	tx, v, schema, e := s.begin(ctx, scope, b)
	if e != nil {
		return platformsession.Point{}, e
	}
	defer tx.Rollback()
	var raw []byte
	if e = tx.QueryRowContext(ctx, `SELECT cache FROM host_command.checkpoints WHERE workspace=$1 AND session=$2`, b.Workspace, b.Session).Scan(&raw); e != nil {
		return platformsession.Point{}, authStorageError(e)
	}
	var point data.CheckpointCache
	if checkpoint.StrictDecode(raw, &point, checkpoint.MaxBytes*2) != nil || point.Binding != b || point.Version == 0 || point.Version > v.Version || point.Cursor > v.Cursor || point.StateSchema != schema || !checkpoint.IsDigest(point.CheckpointSchema) || checkpoint.Validate(point.Value) != nil {
		return platformsession.Point{}, auth.ErrDenied
	}
	encoded, e := json.Marshal(point.Value)
	if e != nil || point.Hash != checkpoint.Hash(encoded) {
		return platformsession.Point{}, auth.ErrDenied
	}
	if e = tx.Commit(); e != nil {
		return platformsession.Point{}, authStorageError(e)
	}
	return auth.RoomSecret(platformsession.PointData{Scope: scope, Binding: b, Version: point.Version, Cursor: point.Cursor}), nil
}
