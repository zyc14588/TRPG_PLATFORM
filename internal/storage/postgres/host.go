// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package postgres

import (
	"context"
	"database/sql"
	"encoding/json"
	"errors"
	"math"
	"time"

	_ "github.com/jackc/pgx/v5/stdlib"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/model"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
)

type HostOptions struct {
	DB      *sql.DB
	Fault   func(context.Context, string) error
	AbortAt string
}
type HostRepository struct {
	options HostOptions
	owned   bool
}

func NewHostRepository(o HostOptions) (*HostRepository, error) {
	if o.DB == nil {
		return nil, data.ErrDenied
	}
	return &HostRepository{options: o}, nil
}

// OpenHostRepository is an operator/composition seam, keeping raw drivers out of
// callbacks and integration tests. It never bootstraps or provisions implicitly.
func OpenHostRepository(ctx context.Context, dsn string, fault func(context.Context, string) error) (*HostRepository, error) {
	db, err := sql.Open("pgx", dsn)
	if err != nil {
		return nil, err
	}
	if err = db.PingContext(ctx); err != nil {
		db.Close()
		return nil, err
	}
	r, _ := NewHostRepository(HostOptions{DB: db, Fault: fault})
	r.owned = true
	return r, nil
}
func (r *HostRepository) Close() error {
	if r.owned {
		return r.options.DB.Close()
	}
	return nil
}
func (r *HostRepository) fault(ctx context.Context, point string) error {
	if ctx.Err() != nil {
		return ctx.Err()
	}
	if r.options.Fault != nil {
		return r.options.Fault(ctx, point)
	}
	return nil
}
func validBinding(b data.Binding) bool {
	return store.ValidID(b.Workspace) && store.ValidID(b.Session) && checkpoint.IsDigest(b.GraphHash)
}
func validHeader(h data.Header) bool {
	return validBinding(h.Binding) && store.ValidID(h.Principal) && store.ValidID(h.CommandID) && checkpoint.IsDigest(h.Fingerprint) && h.ExpectedVersion > 0 && h.ExpectedVersion < math.MaxInt64
}
func hostJSON(v any) ([]byte, error) { return json.Marshal(v) }
func hostClone[T any](v T) T {
	b, _ := json.Marshal(v)
	var out T
	_ = json.Unmarshal(b, &out)
	return out
}

// Bootstrap is a separate trusted operator action. Runtime command operations
// are static parameterized statements and never execute package-provided DDL.
func (r *HostRepository) Bootstrap(ctx context.Context) error {
	tx, err := r.options.DB.BeginTx(ctx, nil)
	if err != nil {
		return err
	}
	defer tx.Rollback()
	for _, statement := range []string{
		`CREATE SCHEMA IF NOT EXISTS host_command`,
		`CREATE TABLE IF NOT EXISTS host_command.sessions(workspace text NOT NULL,session text NOT NULL,graph_hash text NOT NULL,version bigint NOT NULL CHECK(version>0),state jsonb NOT NULL,schema_hash text NOT NULL,event_sequence bigint NOT NULL DEFAULT 0,PRIMARY KEY(workspace,session))`,
		`CREATE TABLE IF NOT EXISTS host_command.installed_graphs(workspace text NOT NULL,session text NOT NULL,graph_bytes bytea NOT NULL,PRIMARY KEY(workspace,session),FOREIGN KEY(workspace,session) REFERENCES host_command.sessions)`,
		`CREATE TABLE IF NOT EXISTS host_command.requests(workspace text NOT NULL,session text NOT NULL,command_id text NOT NULL,principal text NOT NULL,fingerprint text NOT NULL,receipt bytea NOT NULL,PRIMARY KEY(workspace,session,command_id),FOREIGN KEY(workspace,session) REFERENCES host_command.sessions)`,
		`CREATE TABLE IF NOT EXISTS host_command.patches(workspace text NOT NULL,session text NOT NULL,command_id text NOT NULL,ordinal integer NOT NULL,event_id text NOT NULL,patch bytea NOT NULL,PRIMARY KEY(workspace,session,command_id,ordinal),FOREIGN KEY(workspace,session,command_id) REFERENCES host_command.requests DEFERRABLE INITIALLY DEFERRED)`,
		`CREATE TABLE IF NOT EXISTS host_command.documents(workspace text NOT NULL,session text NOT NULL,package_id text NOT NULL,namespace text NOT NULL,key text NOT NULL,schema_hash text NOT NULL,value bytea NOT NULL,command_id text NOT NULL,event_id text NOT NULL,PRIMARY KEY(workspace,session,package_id,namespace,key),FOREIGN KEY(workspace,session) REFERENCES host_command.sessions)`,
		`CREATE INDEX IF NOT EXISTS host_command_document_scope ON host_command.documents(workspace,session,package_id,namespace,key)`,
		`CREATE TABLE IF NOT EXISTS host_command.quantity(workspace text NOT NULL,session text NOT NULL,package_id text NOT NULL,key text NOT NULL,quantity bigint NOT NULL,command_id text NOT NULL,event_id text NOT NULL,PRIMARY KEY(workspace,session,package_id,key),FOREIGN KEY(workspace,session) REFERENCES host_command.sessions)`,
		`CREATE TABLE IF NOT EXISTS host_command.events(workspace text NOT NULL,session text NOT NULL,sequence bigint NOT NULL,version bigint NOT NULL,command_id text NOT NULL,event_id text NOT NULL,event_type text NOT NULL,payload bytea NOT NULL,PRIMARY KEY(workspace,session,sequence),UNIQUE(workspace,session,event_id),FOREIGN KEY(workspace,session,command_id) REFERENCES host_command.requests DEFERRABLE INITIALLY DEFERRED)`,
		`CREATE TABLE IF NOT EXISTS host_command.tasks(workspace text NOT NULL,session text NOT NULL,command_id text NOT NULL,id text NOT NULL,package_id text NOT NULL,kind text NOT NULL,payload bytea NOT NULL,PRIMARY KEY(workspace,session,id),FOREIGN KEY(workspace,session,command_id) REFERENCES host_command.requests DEFERRABLE INITIALLY DEFERRED)`,
		`CREATE TABLE IF NOT EXISTS host_command.continuations(workspace text NOT NULL,session text NOT NULL,command_id text NOT NULL,id text NOT NULL,package_id text NOT NULL,kind text NOT NULL,payload bytea NOT NULL,PRIMARY KEY(workspace,session,id),FOREIGN KEY(workspace,session,command_id) REFERENCES host_command.requests DEFERRABLE INITIALLY DEFERRED)`,
		`CREATE TABLE IF NOT EXISTS host_command.outbox(workspace text NOT NULL,session text NOT NULL,command_id text NOT NULL,id text NOT NULL,package_id text NOT NULL,kind text NOT NULL,payload bytea NOT NULL,PRIMARY KEY(workspace,session,id),FOREIGN KEY(workspace,session,command_id) REFERENCES host_command.requests DEFERRABLE INITIALLY DEFERRED)`,
		`CREATE TABLE IF NOT EXISTS host_command.audit(workspace text NOT NULL,session text NOT NULL,command_id text NOT NULL,ordinal integer NOT NULL,record bytea NOT NULL,PRIMARY KEY(workspace,session,command_id,ordinal),FOREIGN KEY(workspace,session,command_id) REFERENCES host_command.requests DEFERRABLE INITIALLY DEFERRED)`,
	} {
		if _, err = tx.ExecContext(ctx, statement); err != nil {
			return err
		}
	}
	return tx.Commit()
}
func (r *HostRepository) ProvisionSession(ctx context.Context, b data.Binding, version uint64, schemaHash string, state checkpoint.Value) error {
	if !validBinding(b) || version == 0 || version >= math.MaxInt64 || !checkpoint.IsDigest(schemaHash) || checkpoint.Validate(state) != nil {
		return data.ErrDenied
	}
	raw, err := hostJSON(state)
	if err != nil {
		return err
	}
	result, err := r.options.DB.ExecContext(ctx, `INSERT INTO host_command.sessions(workspace,session,graph_hash,version,state,schema_hash) VALUES($1,$2,$3,$4,$5::jsonb,$6) ON CONFLICT DO NOTHING`, b.Workspace, b.Session, b.GraphHash, int64(version), string(raw), schemaHash)
	if err != nil {
		return err
	}
	n, err := result.RowsAffected()
	if err != nil {
		return err
	}
	if n == 1 {
		return nil
	}
	var graph, schema string
	var current uint64
	var actual []byte
	if err = r.options.DB.QueryRowContext(ctx, `SELECT graph_hash,version,state,schema_hash FROM host_command.sessions WHERE workspace=$1 AND session=$2`, b.Workspace, b.Session).Scan(&graph, &current, &actual, &schema); err != nil {
		return err
	}
	var value checkpoint.Value
	if checkpoint.StrictDecode(actual, &value, checkpoint.MaxBytes*2) != nil || graph != b.GraphHash || current != version || schema != schemaHash {
		return data.ErrConflict
	}
	one, _ := json.Marshal(value)
	if string(one) != string(raw) {
		return data.ErrConflict
	}
	return nil
}

type hostTransaction struct {
	repository    *HostRepository
	tx            *sql.Tx
	header        data.Header
	snapshot      data.Snapshot
	eventSequence uint64
	closed        bool
}

func (r *HostRepository) Begin(ctx context.Context, h data.Header) (data.Transaction, error) {
	if !validHeader(h) {
		return nil, data.ErrDenied
	}
	if err := r.fault(ctx, "before-transaction"); err != nil {
		return nil, err
	}
	tx, err := r.options.DB.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelReadCommitted})
	if err != nil {
		return nil, err
	}
	fail := func(e error) (data.Transaction, error) { tx.Rollback(); return nil, e }
	if _, err = tx.ExecContext(ctx, `SET LOCAL statement_timeout='2s'`); err != nil {
		return fail(err)
	}
	t := &hostTransaction{repository: r, tx: tx, header: h}
	t.snapshot.Binding = h.Binding
	var graph string
	var state []byte
	err = tx.QueryRowContext(ctx, `SELECT graph_hash,version,state,schema_hash,event_sequence FROM host_command.sessions WHERE workspace=$1 AND session=$2 FOR UPDATE`, h.Binding.Workspace, h.Binding.Session).Scan(&graph, &t.snapshot.Version, &state, &t.snapshot.SchemaHash, &t.eventSequence)
	if errors.Is(err, sql.ErrNoRows) {
		return fail(data.ErrNotFound)
	}
	if err != nil {
		return fail(err)
	}
	if graph != h.Binding.GraphHash {
		return fail(data.ErrDenied)
	}
	var receipt []byte
	var principal, fingerprint string
	err = tx.QueryRowContext(ctx, `SELECT principal,fingerprint,receipt FROM host_command.requests WHERE workspace=$1 AND session=$2 AND command_id=$3`, h.Binding.Workspace, h.Binding.Session, h.CommandID).Scan(&principal, &fingerprint, &receipt)
	if err == nil {
		if principal != h.Principal || fingerprint != h.Fingerprint {
			return fail(data.ErrConflict)
		}
		var saved data.Receipt
		if checkpoint.StrictDecode(receipt, &saved, 2<<20) != nil || saved.Header != h {
			return fail(data.ErrConflict)
		}
		t.snapshot.Existing = &saved
		return t, nil
	}
	if !errors.Is(err, sql.ErrNoRows) {
		return fail(err)
	}
	if t.snapshot.Version != h.ExpectedVersion || checkpoint.StrictDecode(state, &t.snapshot.State, checkpoint.MaxBytes*2) != nil || checkpoint.Validate(t.snapshot.State) != nil {
		return fail(data.ErrConflict)
	}
	rows, err := tx.QueryContext(ctx, `SELECT package_id,namespace,key,schema_hash,value FROM host_command.documents WHERE workspace=$1 AND session=$2 ORDER BY package_id,namespace,key LIMIT 129`, h.Binding.Workspace, h.Binding.Session)
	if err != nil {
		return fail(err)
	}
	size := 0
	for rows.Next() {
		var row data.Row
		var raw []byte
		if err = rows.Scan(&row.PackageID, &row.Namespace, &row.Key, &row.SchemaHash, &raw); err != nil {
			rows.Close()
			return fail(err)
		}
		size += len(raw)
		if size > 256<<10 || len(t.snapshot.Rows) >= 128 || checkpoint.StrictDecode(raw, &row.Value, checkpoint.MaxBytes*2) != nil || checkpoint.Validate(row.Value) != nil {
			rows.Close()
			return fail(data.ErrDenied)
		}
		t.snapshot.Rows = append(t.snapshot.Rows, row)
	}
	err = rows.Err()
	rows.Close()
	if err != nil {
		return fail(err)
	}
	rows, err = tx.QueryContext(ctx, `SELECT package_id,key,quantity FROM host_command.quantity WHERE workspace=$1 AND session=$2 ORDER BY package_id,key LIMIT 129`, h.Binding.Workspace, h.Binding.Session)
	if err != nil {
		return fail(err)
	}
	for rows.Next() {
		var q data.Quantity
		q.Table = "quantity"
		if err = rows.Scan(&q.PackageID, &q.Key, &q.Value); err != nil {
			rows.Close()
			return fail(err)
		}
		if len(t.snapshot.Quantities) >= 128 {
			rows.Close()
			return fail(data.ErrDenied)
		}
		t.snapshot.Quantities = append(t.snapshot.Quantities, q)
	}
	err = rows.Err()
	rows.Close()
	if err != nil {
		return fail(err)
	}
	return t, nil
}
func (t *hostTransaction) Snapshot() data.Snapshot { return hostClone(t.snapshot) }
func (t *hostTransaction) Rollback() error {
	if t.closed {
		return nil
	}
	t.closed = true
	return t.tx.Rollback()
}
func (t *hostTransaction) Commit(ctx context.Context, c data.Commit) (data.Receipt, error) {
	var zero data.Receipt
	if t.closed || c.Header != t.header || t.snapshot.Existing != nil || c.SchemaHash != t.snapshot.SchemaHash || checkpoint.Validate(c.State) != nil || checkpoint.Validate(c.Result) != nil || len(c.Events) > 64 || len(c.Rows)+len(c.Quantities) > 128 || len(c.Patches) > 128 || len(c.Tasks) > 32 || len(c.Continuations) > 32 || len(c.Outbox) > 64 || len(c.Audit) > 257 {
		return zero, data.ErrDenied
	}
	h := c.Header
	w, s, id := h.Binding.Workspace, h.Binding.Session, h.CommandID
	version := h.ExpectedVersion + 1
	fault := func(point string) error {
		if t.repository.options.AbortAt == point {
			_, err := t.tx.ExecContext(ctx, `SELECT 1/0`)
			return err
		}
		return t.repository.fault(ctx, point)
	}
	state, err := hostJSON(c.State)
	if err != nil {
		return zero, err
	}
	r, err := t.tx.ExecContext(ctx, `UPDATE host_command.sessions SET version=$3,state=$4::jsonb,event_sequence=$5 WHERE workspace=$1 AND session=$2 AND version=$6 AND graph_hash=$7 AND schema_hash=$8`, w, s, int64(version), string(state), int64(t.eventSequence+uint64(len(c.Events))), int64(h.ExpectedVersion), h.Binding.GraphHash, c.SchemaHash)
	if err != nil {
		return zero, err
	}
	n, err := r.RowsAffected()
	if err != nil || n != 1 {
		return zero, data.ErrConflict
	}
	eventID := ""
	if len(c.Events) > 0 {
		eventID = c.Events[0].ID
	}
	for i, p := range c.Patches {
		if p.EventID != eventID || p.CommandID != id || eventID == "" {
			return zero, data.ErrDenied
		}
		raw, err := hostJSON(p)
		if err != nil {
			return zero, err
		}
		if _, err = t.tx.ExecContext(ctx, `INSERT INTO host_command.patches(workspace,session,command_id,ordinal,event_id,patch) VALUES($1,$2,$3,$4,$5,$6)`, w, s, id, i, eventID, raw); err != nil {
			return zero, err
		}
	}
	if err = fault("after-state"); err != nil {
		return zero, err
	}
	for _, row := range c.Rows {
		if _, e := model.ParsePackageID(row.PackageID); e != nil {
			return zero, data.ErrDenied
		}
		if !store.ValidID(row.Namespace) || !store.ValidID(row.Key) || !checkpoint.IsDigest(row.SchemaHash) || checkpoint.Validate(row.Value) != nil || eventID == "" {
			return zero, data.ErrDenied
		}
		if row.Deleted {
			_, err = t.tx.ExecContext(ctx, `DELETE FROM host_command.documents WHERE workspace=$1 AND session=$2 AND package_id=$3 AND namespace=$4 AND key=$5`, w, s, row.PackageID, row.Namespace, row.Key)
		} else {
			raw, e := hostJSON(row.Value)
			if e != nil {
				return zero, e
			}
			_, err = t.tx.ExecContext(ctx, `INSERT INTO host_command.documents(workspace,session,package_id,namespace,key,schema_hash,value,command_id,event_id) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9) ON CONFLICT(workspace,session,package_id,namespace,key) DO UPDATE SET schema_hash=EXCLUDED.schema_hash,value=EXCLUDED.value,command_id=EXCLUDED.command_id,event_id=EXCLUDED.event_id`, w, s, row.PackageID, row.Namespace, row.Key, row.SchemaHash, raw, id, eventID)
		}
		if err != nil {
			return zero, err
		}
	}
	for _, q := range c.Quantities {
		if _, e := model.ParsePackageID(q.PackageID); e != nil || q.Table != "quantity" || !store.ValidID(q.Key) || eventID == "" {
			return zero, data.ErrDenied
		}
		if _, err = t.tx.ExecContext(ctx, `INSERT INTO host_command.quantity(workspace,session,package_id,key,quantity,command_id,event_id) VALUES($1,$2,$3,$4,$5,$6,$7) ON CONFLICT(workspace,session,package_id,key) DO UPDATE SET quantity=EXCLUDED.quantity,command_id=EXCLUDED.command_id,event_id=EXCLUDED.event_id`, w, s, q.PackageID, q.Key, q.Value, id, eventID); err != nil {
			return zero, err
		}
	}
	if err = fault("after-package-data"); err != nil {
		return zero, err
	}
	for i, e := range c.Events {
		if !store.ValidID(e.ID) || e.Type == "" || checkpoint.Validate(e.Payload) != nil {
			return zero, data.ErrDenied
		}
		raw, err := hostJSON(e.Payload)
		if err != nil {
			return zero, err
		}
		if _, err = t.tx.ExecContext(ctx, `INSERT INTO host_command.events(workspace,session,sequence,version,command_id,event_id,event_type,payload) VALUES($1,$2,$3,$4,$5,$6,$7,$8)`, w, s, int64(t.eventSequence+uint64(i)+1), int64(version), id, e.ID, e.Type, raw); err != nil {
			return zero, err
		}
	}
	if err = fault("after-events"); err != nil {
		return zero, err
	}
	// Each intent family has a fixed table; no caller-supplied SQL identifier exists.
	for _, family := range []struct {
		items  []data.Intent
		insert string
		point  string
	}{
		{c.Tasks, `INSERT INTO host_command.tasks(workspace,session,command_id,id,package_id,kind,payload) VALUES($1,$2,$3,$4,$5,$6,$7)`, "after-tasks"},
		{c.Continuations, `INSERT INTO host_command.continuations(workspace,session,command_id,id,package_id,kind,payload) VALUES($1,$2,$3,$4,$5,$6,$7)`, "after-continuations"},
		{c.Outbox, `INSERT INTO host_command.outbox(workspace,session,command_id,id,package_id,kind,payload) VALUES($1,$2,$3,$4,$5,$6,$7)`, "after-outbox"},
	} {
		for _, intent := range family.items {
			if _, e := model.ParsePackageID(intent.PackageID); e != nil || !store.ValidID(intent.ID) || checkpoint.Validate(intent.Payload) != nil || eventID == "" {
				return zero, data.ErrDenied
			}
			raw, e := hostJSON(intent.Payload)
			if e != nil {
				return zero, e
			}
			if _, err = t.tx.ExecContext(ctx, family.insert, w, s, id, intent.ID, intent.PackageID, intent.Kind, raw); err != nil {
				return zero, err
			}
		}
		if err = fault(family.point); err != nil {
			return zero, err
		}
	}
	receipt := data.Receipt{Header: h, Version: version, Result: c.Result, Events: c.Events, Inputs: c.Inputs}
	raw, err := hostJSON(receipt)
	if err != nil {
		return zero, err
	}
	if _, err = t.tx.ExecContext(ctx, `INSERT INTO host_command.requests(workspace,session,command_id,principal,fingerprint,receipt) VALUES($1,$2,$3,$4,$5,$6)`, w, s, id, h.Principal, h.Fingerprint, raw); err != nil {
		return zero, err
	}
	if err = fault("after-idempotency"); err != nil {
		return zero, err
	}
	for i, a := range c.Audit {
		if a.Level != "AUDIT-0" && a.Level != "AUDIT-1" && a.Level != "AUDIT-2" && a.Level != "AUDIT-3" || !checkpoint.IsDigest(a.ArgumentsHash) || !checkpoint.IsDigest(a.ResultHash) {
			return zero, data.ErrDenied
		}
		raw, e := hostJSON(a)
		if e != nil {
			return zero, e
		}
		if _, err = t.tx.ExecContext(ctx, `INSERT INTO host_command.audit(workspace,session,command_id,ordinal,record) VALUES($1,$2,$3,$4,$5)`, w, s, id, i, raw); err != nil {
			return zero, err
		}
	}
	if len(c.Audit) == 0 || c.Audit[len(c.Audit)-1].Level != "AUDIT-0" {
		return zero, data.ErrDenied
	}
	if err = fault("after-audit"); err != nil {
		return zero, err
	}
	if err = fault("before-commit"); err != nil {
		return zero, err
	}
	err = t.tx.Commit()
	t.closed = true
	if err == nil {
		err = t.repository.fault(context.Background(), "after-commit")
	}
	if err != nil {
		// Lost commit acknowledgements resolve this exact immutable request. They
		// never re-run Lua or apply a second copy of its effects.
		resolveCtx, cancel := context.WithTimeout(context.Background(), 2*time.Second)
		defer cancel()
		saved, e := t.repository.Resolve(resolveCtx, h)
		if e == nil {
			return saved, nil
		}
		return zero, errors.Join(data.ErrUnknownCommit, err)
	}
	return hostClone(receipt), nil
}
func (r *HostRepository) Resolve(ctx context.Context, h data.Header) (data.Receipt, error) {
	var out data.Receipt
	if !validHeader(h) {
		return out, data.ErrDenied
	}
	var principal, fingerprint string
	var raw []byte
	err := r.options.DB.QueryRowContext(ctx, `SELECT principal,fingerprint,receipt FROM host_command.requests WHERE workspace=$1 AND session=$2 AND command_id=$3`, h.Binding.Workspace, h.Binding.Session, h.CommandID).Scan(&principal, &fingerprint, &raw)
	if errors.Is(err, sql.ErrNoRows) {
		return out, data.ErrNotFound
	}
	if err != nil {
		return out, err
	}
	if principal != h.Principal || fingerprint != h.Fingerprint || checkpoint.StrictDecode(raw, &out, 2<<20) != nil || out.Header != h {
		return data.Receipt{}, data.ErrConflict
	}
	return out, nil
}

// Inspect is an operator/test readback of the same explicit tenant/SID. It is
// intentionally data-only and uses no transaction or connection handle.
type HostInspection struct {
	Version                                                                               uint64
	State                                                                                 checkpoint.Value
	Documents, Quantities, Patches, Events, Requests, Tasks, Continuations, Outbox, Audit int
}

func (r *HostRepository) Inspect(ctx context.Context, b data.Binding) (HostInspection, error) {
	var out HostInspection
	if !validBinding(b) {
		return out, data.ErrDenied
	}
	var graph string
	var raw []byte
	err := r.options.DB.QueryRowContext(ctx, `SELECT graph_hash,version,state FROM host_command.sessions WHERE workspace=$1 AND session=$2`, b.Workspace, b.Session).Scan(&graph, &out.Version, &raw)
	if err != nil {
		return out, err
	}
	if graph != b.GraphHash || checkpoint.StrictDecode(raw, &out.State, checkpoint.MaxBytes*2) != nil {
		return out, data.ErrDenied
	}
	err = r.options.DB.QueryRowContext(ctx, `SELECT (SELECT count(*) FROM host_command.documents WHERE workspace=$1 AND session=$2),(SELECT count(*) FROM host_command.quantity WHERE workspace=$1 AND session=$2),(SELECT count(*) FROM host_command.patches WHERE workspace=$1 AND session=$2),(SELECT count(*) FROM host_command.events WHERE workspace=$1 AND session=$2),(SELECT count(*) FROM host_command.requests WHERE workspace=$1 AND session=$2),(SELECT count(*) FROM host_command.tasks WHERE workspace=$1 AND session=$2),(SELECT count(*) FROM host_command.continuations WHERE workspace=$1 AND session=$2),(SELECT count(*) FROM host_command.outbox WHERE workspace=$1 AND session=$2),(SELECT count(*) FROM host_command.audit WHERE workspace=$1 AND session=$2)`, b.Workspace, b.Session).Scan(&out.Documents, &out.Quantities, &out.Patches, &out.Events, &out.Requests, &out.Tasks, &out.Continuations, &out.Outbox, &out.Audit)
	return out, err
}

// WithTransactionAbort is an operator-only negative-test seam: the selected
// fixed point runs a literal PostgreSQL division-by-zero in the live transaction.
// It can only abort; it exposes no SQL string, table choice, connection or Tx.
func (r *HostRepository) WithTransactionAbort(point string) (*HostRepository, error) {
	allowed := map[string]bool{"after-state": true, "after-package-data": true, "after-events": true, "after-tasks": true, "after-continuations": true, "after-outbox": true, "after-idempotency": true, "after-audit": true, "before-commit": true}
	if !allowed[point] {
		return nil, data.ErrDenied
	}
	o := r.options
	o.AbortAt = point
	return &HostRepository{options: o}, nil
}
