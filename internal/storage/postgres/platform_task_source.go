// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package postgres

import (
	"context"
	"database/sql"
	"encoding/json"
	"errors"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/eventstore"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/persistence"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
	"github.com/zyc14588/TRPG_PLATFORM/internal/task"
)

func equalTaskValue(a, b any) bool {
	x, e := json.Marshal(a)
	if e != nil {
		return false
	}
	y, e := json.Marshal(b)
	return e == nil && string(x) == string(y)
}

func equalStoredTaskValue(raw []byte, expected checkpoint.Value) bool {
	// JSON decoding into an existing table retains absent map keys. Each
	// independent intent must have a fresh value, never the previous row's map.
	var saved checkpoint.Value
	return checkpoint.StrictDecode(raw, &saved, 256<<10) == nil && checkpoint.Validate(saved) == nil && equalTaskValue(saved, expected)
}
func projectTask(raw []byte, v task.JobData) (task.JobData, error) {
	r, e := eventstore.Decode(raw)
	if e != nil || r.Header.Binding != v.Binding || r.Header.CommandID != v.SourceCommand || r.Migration != nil || r.Ended {
		return task.JobData{}, task.ErrDenied
	}
	v.SourceHash = checkpoint.Hash(raw)
	v.OriginVersion = r.Version
	v.OriginPrincipal = r.Header.Principal
	found := false
	for _, t := range r.Tasks {
		if t.ID == v.TaskID {
			if found || t.PackageID != v.PackageID || t.Kind != "task" {
				return task.JobData{}, task.ErrDenied
			}
			v.Payload, e = task.NewValue(t.Payload)
			if e != nil {
				return task.JobData{}, e
			}
			found = true
		}
	}
	if !found {
		return task.JobData{}, task.ErrDenied
	}
	found = false
	for _, o := range r.Outbox {
		if o.ID == v.OutboxID {
			if found || o.PackageID != v.PackageID || o.Kind != "dispatch-task" || o.Payload.Kind != "table" || len(o.Payload.Table) != 1 || o.Payload.Table["task"].Kind != "string" || o.Payload.Table["task"].String != v.TaskID {
				return task.JobData{}, task.ErrDenied
			}
			found = true
		}
	}
	if !found {
		return task.JobData{}, task.ErrDenied
	}
	v.Continuations = []task.Continuation{}
	for _, c := range r.Continuations {
		if c.Payload.Kind != "table" || c.Payload.Table["task"].Kind != "string" {
			return task.JobData{}, task.ErrDenied
		}
		if c.Payload.Table["task"].String != v.TaskID {
			continue
		}
		if c.PackageID != v.PackageID || c.Kind != "continuation" || len(c.Payload.Table) != 2 {
			return task.JobData{}, task.ErrDenied
		}
		value, e := task.NewValue(c.Payload.Table["value"])
		if e != nil {
			return task.JobData{}, e
		}
		v.Continuations = append(v.Continuations, task.Continuation{ID: c.ID, Value: value})
	}
	return v, nil
}

type taskSeed struct {
	v                                   task.JobData
	taskRaw, outboxRaw, source, receipt []byte
}

func (s *PlatformTaskStorage) seed(ctx context.Context, tx *sql.Tx, space string, now time.Time, limit int) (int, error) {
	var active int
	if e := tx.QueryRowContext(ctx, `SELECT count(*) FROM platform_task.jobs WHERE workspace=$1 AND status IN ('queued','running','ready','delivering')`, space).Scan(&active); e != nil {
		return 0, taskSQL(e)
	}
	if active >= 64 {
		return 0, nil
	}
	if limit > 64-active {
		limit = 64 - active
	}
	// Match the typed checkpoint payload to avoid a cross product between
	// multiple tasks in one committed command. Both inputs are server data.
	rows, e := tx.QueryContext(ctx, `SELECT l.workspace_id,l.room_id,l.game_id,l.session_id,l.graph_hash,l.configuration_id,l.configuration_hash,t.id,o.id,t.command_id,t.package_id,t.payload,o.payload,r.evidence,q.receipt FROM platform_launch.sessions l JOIN platform_room.rooms room ON room.workspace_id=l.workspace_id AND room.room_id=l.room_id AND room.game_id=l.game_id JOIN host_command.sessions h ON h.workspace=l.workspace_id AND h.session=l.session_id AND h.graph_hash=l.graph_hash JOIN host_command.tasks t ON t.workspace=h.workspace AND t.session=h.session AND t.kind='task' JOIN host_command.outbox o ON o.workspace=t.workspace AND o.session=t.session AND o.command_id=t.command_id AND o.package_id=t.package_id AND o.kind='dispatch-task' AND convert_from(o.payload,'UTF8')::jsonb->'table'->'task'->>'string'=t.id JOIN host_command.replay_effects r ON r.workspace=t.workspace AND r.session=t.session AND r.command_id=t.command_id JOIN host_command.requests q ON q.workspace=t.workspace AND q.session=t.session AND q.command_id=t.command_id WHERE l.workspace_id=$1 AND room.state='launched' AND NOT EXISTS(SELECT 1 FROM host_command.endings e WHERE e.workspace=h.workspace AND e.session=h.session) AND NOT EXISTS(SELECT 1 FROM platform_task.jobs j WHERE j.workspace=t.workspace AND j.session=t.session AND (j.task_id=t.id OR j.outbox_id=o.id)) ORDER BY r.version,t.id LIMIT $2`, space, limit)
	if e != nil {
		return 0, taskSQL(e)
	}
	defer rows.Close()
	pending := []taskSeed{}
	for rows.Next() {
		p := taskSeed{v: task.JobData{Status: task.Queued, Expires: now.Add(s.state().lifetime)}}
		v := &p.v
		if e = rows.Scan(&v.Scope.WorkspaceID, &v.Scope.RoomID, &v.Scope.GameID, &v.Binding.Session, &v.Binding.GraphHash, &v.ConfigurationID, &v.ConfigurationHash, &v.TaskID, &v.OutboxID, &v.SourceCommand, &v.PackageID, &p.taskRaw, &p.outboxRaw, &p.source, &p.receipt); e != nil {
			return 0, taskSQL(e)
		}
		v.Binding.Workspace = space
		pending = append(pending, p)
	}
	if e = rows.Err(); e != nil {
		return 0, taskSQL(e)
	}
	if e = rows.Close(); e != nil {
		return 0, taskSQL(e)
	}
	for _, p := range pending {
		v, e := projectTask(p.source, p.v)
		if e != nil {
			return 0, e
		}
		var original data.Receipt
		if checkpoint.StrictDecode(p.receipt, &original, 2<<20) != nil {
			return 0, task.ErrDenied
		}
		record, e := eventstore.Decode(p.source)
		if e != nil || original.Header != record.Header || original.Version != record.Version || !equalTaskValue(original.Inputs, record.Inputs) || !equalTaskValue(original.Events, record.Events) || !equalTaskValue(original.Result, record.Result) {
			return 0, task.ErrDenied
		}
		payload, e := v.Payload.StorageValue()
		if e != nil {
			return 0, e
		}
		if !equalStoredTaskValue(p.taskRaw, payload) {
			return 0, task.ErrDenied
		}
		wanted := checkpoint.Object(map[string]checkpoint.Value{"task": checkpoint.Text(v.TaskID)})
		if !equalStoredTaskValue(p.outboxRaw, wanted) {
			return 0, task.ErrDenied
		}
		for _, c := range v.Continuations {
			var raw []byte
			var pkg, kind, source string
			if e = tx.QueryRowContext(ctx, `SELECT command_id,package_id,kind,payload FROM host_command.continuations WHERE workspace=$1 AND session=$2 AND id=$3`, space, v.Binding.Session, c.ID).Scan(&source, &pkg, &kind, &raw); e != nil {
				return 0, taskSQL(e)
			}
			value, e := c.Value.StorageValue()
			if e != nil {
				return 0, e
			}
			wanted := checkpoint.Object(map[string]checkpoint.Value{"task": checkpoint.Text(v.TaskID), "value": value})
			if source != v.SourceCommand || pkg != v.PackageID || kind != "continuation" || !equalStoredTaskValue(raw, wanted) {
				return 0, task.ErrDenied
			}
		}
		if e = putTask(ctx, tx, v, "", true); e != nil {
			return 0, e
		}
	}
	return len(pending), nil
}

func taskLive(ctx context.Context, tx *sql.Tx, v task.JobData) error {
	var scope core.Scope
	var graph, config, configHash string
	var version uint64
	var ended bool
	var raw []byte
	e := tx.QueryRowContext(ctx, `SELECT l.workspace_id,l.room_id,l.game_id,l.graph_hash,l.configuration_id,l.configuration_hash,h.version,EXISTS(SELECT 1 FROM host_command.endings e WHERE e.workspace=h.workspace AND e.session=h.session),r.evidence FROM platform_launch.sessions l JOIN platform_room.rooms room ON room.workspace_id=l.workspace_id AND room.room_id=l.room_id AND room.game_id=l.game_id JOIN host_command.sessions h ON h.workspace=l.workspace_id AND h.session=l.session_id JOIN host_command.replay_effects r ON r.workspace=h.workspace AND r.session=h.session AND r.command_id=$3 WHERE l.workspace_id=$1 AND l.session_id=$2 AND room.state='launched' AND h.graph_hash=l.graph_hash`, v.Binding.Workspace, v.Binding.Session, v.SourceCommand).Scan(&scope.WorkspaceID, &scope.RoomID, &scope.GameID, &graph, &config, &configHash, &version, &ended, &raw)
	if errors.Is(e, sql.ErrNoRows) {
		return task.ErrDenied
	}
	if e != nil {
		return taskSQL(e)
	}
	if scope != v.Scope || graph != v.Binding.GraphHash || config != v.ConfigurationID || configHash != v.ConfigurationHash || ended {
		return task.ErrDenied
	}
	projected, e := projectTask(raw, v)
	if e != nil || projected.SourceHash != v.SourceHash || projected.OriginVersion != v.OriginVersion || projected.OriginPrincipal != v.OriginPrincipal || projected.Payload.Digest() != v.Payload.Digest() || len(projected.Continuations) != len(v.Continuations) {
		return task.ErrDenied
	}
	for i, c := range projected.Continuations {
		if c.ID != v.Continuations[i].ID || c.Value.Digest() != v.Continuations[i].Value.Digest() {
			return task.ErrDenied
		}
	}
	if version == v.OriginVersion+uint64(v.Next) {
		return nil
	}
	// A completion committed before an acknowledgment crash is replayable only
	// with its entire immutable system command and recorded input identity.
	if v.ResultWorker != "" {
		if _, e = taskReceipt(ctx, tx, v); e == nil {
			return nil
		} else if !errors.Is(e, task.ErrNotFound) && !errors.Is(e, task.ErrDenied) {
			return e
		}
	}
	return task.ErrStale
}
func taskReceipt(ctx context.Context, tx *sql.Tx, v task.JobData) (data.Receipt, error) {
	j, e := task.NewJob(v)
	if e != nil {
		return data.Receipt{}, task.ErrDenied
	}
	envelope, e := j.Envelope()
	if e != nil {
		return data.Receipt{}, task.ErrDenied
	}
	var raw []byte
	e = tx.QueryRowContext(ctx, `SELECT receipt FROM host_command.requests WHERE workspace=$1 AND session=$2 AND command_id=$3 AND principal='task-system'`, v.Binding.Workspace, v.Binding.Session, envelope.CommandID).Scan(&raw)
	if e != nil {
		return data.Receipt{}, taskSQL(e)
	}
	var r data.Receipt
	if checkpoint.StrictDecode(raw, &r, 2<<20) != nil || r.Header.Binding != v.Binding || r.Header.Principal != "task-system" || r.Header.CommandID != envelope.CommandID || r.Header.ReadOnly || r.Header.ExpectedVersion != envelope.ExpectedStateVersion || r.Version != envelope.ExpectedStateVersion+1 || !checkpoint.IsDigest(r.Header.Fingerprint) || r.Inputs.Callback != "resume_continuation" || r.Inputs.Envelope == nil || *r.Inputs.Envelope != (data.EnvelopeMetadata{Seat: envelope.SeatID, Type: envelope.Type, Correlation: envelope.CorrelationID}) || !equalTaskValue(r.Inputs.Command, persistence.Input(envelope)) {
		return data.Receipt{}, task.ErrDenied
	}
	result, e := v.Result.StorageValue()
	if e != nil {
		return data.Receipt{}, task.ErrDenied
	}
	at, random, e := v.Inputs.StorageValue()
	if e != nil || r.Inputs.Time != at || !equalTaskValue(random, r.Inputs.Random) || len(r.Inputs.ToolResults) != 1 || !equalTaskValue(result, r.Inputs.ToolResults[0]) {
		return data.Receipt{}, task.ErrDenied
	}
	return r, nil
}
func sameTaskReceipt(a, b data.Receipt) bool {
	return a.Header == b.Header && a.Version == b.Version && a.Cursor == b.Cursor && equalTaskValue(a.Inputs, b.Inputs) && equalTaskValue(a.Result, b.Result) && equalTaskValue(a.Events, b.Events)
}
