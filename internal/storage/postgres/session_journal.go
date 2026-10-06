// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package postgres

import (
	"context"
	"database/sql"
	"errors"
	"math"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
)

// ReadJournal uses one bounded repeatable-read snapshot. Cursor gaps caused by
// seat filtering do not grant access to the omitted event payloads.
func (r *HostRepository) ReadJournal(ctx context.Context, b data.Binding, after uint64, limit int) (data.JournalPage, error) {
	page := data.JournalPage{Binding: b}
	if !validBinding(b) || after >= math.MaxInt64 || limit < 1 || limit > 128 {
		return page, data.ErrDenied
	}
	tx, err := r.options.DB.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelRepeatableRead, ReadOnly: true})
	if err != nil {
		return page, err
	}
	defer tx.Rollback()
	if _, err = tx.ExecContext(ctx, `SET LOCAL statement_timeout='2s'`); err != nil {
		return page, err
	}
	var graph string
	err = tx.QueryRowContext(ctx, `SELECT graph_hash,version,event_sequence,EXISTS(SELECT 1 FROM host_command.endings e WHERE e.workspace=s.workspace AND e.session=s.session) FROM host_command.sessions s WHERE workspace=$1 AND session=$2`, b.Workspace, b.Session).Scan(&graph, &page.Version, &page.Cursor, &page.Ended)
	if errors.Is(err, sql.ErrNoRows) {
		return page, data.ErrNotFound
	}
	if err != nil {
		return page, err
	}
	if graph != b.GraphHash {
		return page, data.ErrDenied
	}
	if after > page.Cursor {
		return page, data.ErrConflict
	}
	rows, err := tx.QueryContext(ctx, `SELECT sequence,version,command_id,event_id,event_type,payload FROM host_command.events WHERE workspace=$1 AND session=$2 AND sequence>$3 ORDER BY sequence LIMIT $4`, b.Workspace, b.Session, int64(after), limit+1)
	if err != nil {
		return page, err
	}
	defer rows.Close()
	size := 0
	for rows.Next() {
		var e data.JournalEvent
		var raw []byte
		if err = rows.Scan(&e.Sequence, &e.Version, &e.CommandID, &e.Event.ID, &e.Event.Type, &raw); err != nil {
			return page, err
		}
		size += len(raw)
		if len(page.Events) >= limit || size > 256<<10 {
			page.Truncated = true
			page.Events = nil
			break
		}
		if e.Sequence <= after || e.Sequence > page.Cursor || !store.ValidID(e.CommandID) || !store.ValidID(e.Event.ID) || checkpoint.StrictDecode(raw, &e.Event.Payload, checkpoint.MaxBytes*2) != nil || checkpoint.Validate(e.Event.Payload) != nil {
			return page, data.ErrDenied
		}
		page.Events = append(page.Events, e)
	}
	if err = rows.Err(); err != nil {
		return page, err
	}
	rows.Close()
	if err = tx.Commit(); err != nil {
		return page, err
	}
	return page, nil
}
func (r *HostRepository) ReadCreation(ctx context.Context, b data.Binding) (data.Creation, error) {
	var c data.Creation
	if !validBinding(b) {
		return c, data.ErrDenied
	}
	var raw []byte
	var graph string
	err := r.options.DB.QueryRowContext(ctx, `SELECT s.graph_hash,c.evidence FROM host_command.sessions s JOIN host_command.creation c ON c.workspace=s.workspace AND c.session=s.session WHERE s.workspace=$1 AND s.session=$2`, b.Workspace, b.Session).Scan(&graph, &raw)
	if errors.Is(err, sql.ErrNoRows) {
		return c, data.ErrNotFound
	}
	if err != nil {
		return c, err
	}
	if graph != b.GraphHash || checkpoint.StrictDecode(raw, &c, checkpoint.MaxBytes*2) != nil || c.Binding != b || c.Version == 0 || !checkpoint.IsDigest(c.SchemaHash) || !checkpoint.IsDigest(c.ArtifactsHash) || checkpoint.Validate(c.Seed) != nil {
		return data.Creation{}, data.ErrDenied
	}
	seed, _ := hostJSON(c.Seed)
	if checkpoint.Hash(seed) != c.SeedHash {
		return data.Creation{}, data.ErrDenied
	}
	return c, nil
}

// LookupCommand is an authenticated application read; the caller must compare
// its complete immutable envelope with the saved inputs before exposing it.
func (r *HostRepository) LookupCommand(ctx context.Context, b data.Binding, principal, id string) (data.Receipt, error) {
	var saved data.Receipt
	if !validBinding(b) || !store.ValidID(principal) || !store.ValidID(id) {
		return saved, data.ErrDenied
	}
	var graph, owner string
	var raw []byte
	err := r.options.DB.QueryRowContext(ctx, `SELECT s.graph_hash,r.principal,r.receipt FROM host_command.sessions s JOIN host_command.requests r ON r.workspace=s.workspace AND r.session=s.session WHERE s.workspace=$1 AND s.session=$2 AND r.command_id=$3`, b.Workspace, b.Session, id).Scan(&graph, &owner, &raw)
	if errors.Is(err, sql.ErrNoRows) {
		return saved, data.ErrNotFound
	}
	if err != nil {
		return saved, err
	}
	if graph != b.GraphHash || owner != principal || checkpoint.StrictDecode(raw, &saved, 2<<20) != nil || saved.Header.Binding != b || saved.Header.Principal != principal || saved.Header.CommandID != id || !checkpoint.IsDigest(saved.Header.Fingerprint) {
		return data.Receipt{}, data.ErrDenied
	}
	saved.Replayed = true
	return saved, nil
}
func (r *HostRepository) SaveCheckpoint(ctx context.Context, c data.CheckpointCache) error {
	if !validBinding(c.Binding) || c.Version == 0 || c.Version >= math.MaxInt64 || c.Cursor >= math.MaxInt64 || !checkpoint.IsDigest(c.StateSchema) || !checkpoint.IsDigest(c.CheckpointSchema) || checkpoint.Validate(c.Value) != nil {
		return data.ErrDenied
	}
	value, err := hostJSON(c.Value)
	if err != nil || c.Hash != checkpoint.Hash(value) {
		return data.ErrDenied
	}
	raw, err := hostJSON(c)
	if err != nil {
		return err
	}
	tx, err := r.options.DB.BeginTx(ctx, nil)
	if err != nil {
		return err
	}
	defer tx.Rollback()
	if _, err = tx.ExecContext(ctx, `SET LOCAL statement_timeout='2s'`); err != nil {
		return err
	}
	var graph, schema string
	var version, cursor uint64
	if err = tx.QueryRowContext(ctx, `SELECT graph_hash,schema_hash,version,event_sequence FROM host_command.sessions WHERE workspace=$1 AND session=$2 FOR UPDATE`, c.Binding.Workspace, c.Binding.Session).Scan(&graph, &schema, &version, &cursor); err != nil {
		return err
	}
	if graph != c.Binding.GraphHash || schema != c.StateSchema || version != c.Version || cursor != c.Cursor {
		return data.ErrConflict
	}
	if _, err = tx.ExecContext(ctx, `INSERT INTO host_command.checkpoints(workspace,session,cache) VALUES($1,$2,$3) ON CONFLICT(workspace,session) DO UPDATE SET cache=EXCLUDED.cache`, c.Binding.Workspace, c.Binding.Session, raw); err != nil {
		return err
	}
	return tx.Commit()
}
func (r *HostRepository) ReadCheckpoint(ctx context.Context, b data.Binding) (data.CheckpointCache, error) {
	var c data.CheckpointCache
	if !validBinding(b) {
		return c, data.ErrDenied
	}
	var raw []byte
	var graph string
	err := r.options.DB.QueryRowContext(ctx, `SELECT s.graph_hash,c.cache FROM host_command.sessions s JOIN host_command.checkpoints c ON c.workspace=s.workspace AND c.session=s.session WHERE s.workspace=$1 AND s.session=$2`, b.Workspace, b.Session).Scan(&graph, &raw)
	if errors.Is(err, sql.ErrNoRows) {
		return c, data.ErrNotFound
	}
	if err != nil {
		return c, err
	}
	if graph != b.GraphHash || checkpoint.StrictDecode(raw, &c, checkpoint.MaxBytes*2) != nil || c.Binding != b || c.Version == 0 || c.Version >= math.MaxInt64 || c.Cursor >= math.MaxInt64 || !checkpoint.IsDigest(c.StateSchema) || !checkpoint.IsDigest(c.CheckpointSchema) || checkpoint.Validate(c.Value) != nil {
		return data.CheckpointCache{}, data.ErrDenied
	}
	value, _ := hostJSON(c.Value)
	if c.Hash != checkpoint.Hash(value) {
		return data.CheckpointCache{}, data.ErrDenied
	}
	return c, nil
}
