// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package postgres

import (
	"bytes"
	"context"
	"database/sql"
	"errors"
	"reflect"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/eventstore"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/migration"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	"github.com/zyc14588/TRPG_PLATFORM/internal/projection"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
)

// MigrationLease owns the same workspace/Session locks as installation and
// commands. Its SQL handle is private; it exposes only bounded data operations.
// Every VM opened against this lease is read-only, including the source VM.
type MigrationLease struct {
	repository           *HostRepository
	tx                   *sql.Tx
	source, target, view data.SessionLock
	binding              data.Binding
	history              data.ReplayHistory
	staged               bool
	point                *data.RecoveryPoint
	reserved             [2]string
	cancel               context.CancelFunc
	closed               bool
}

func (r *HostRepository) AcquireMigration(ctx context.Context, from, to *store.Graph, b data.Binding, expected uint64) (*MigrationLease, error) {
	if from == nil || to == nil {
		return nil, data.ErrDenied
	}
	a, z := from.Membership(), to.Membership()
	if !a.Install || !a.Read || !z.Install || !z.Read || a.Principal != z.Principal || a.Workspace != b.Workspace || z.Workspace != b.Workspace {
		return nil, data.ErrDenied
	}
	first, err := migration.ExactLock(from)
	if err != nil {
		return nil, err
	}
	last, err := migration.ExactLock(to)
	if err != nil {
		return nil, err
	}
	if first.GraphHash != b.GraphHash || first.Hash == last.Hash {
		return nil, data.ErrDenied
	}
	ctx, cancel := context.WithTimeout(ctx, 15*time.Second)
	tx, _, err := r.installedTransaction(ctx, from, b)
	if err != nil {
		cancel()
		return nil, err
	}
	l := &MigrationLease{repository: r, tx: tx, source: first, target: last, view: first, binding: b, cancel: cancel}
	fail := func(e error) (*MigrationLease, error) { l.Rollback(); return nil, e }
	var version uint64
	if err = tx.QueryRowContext(ctx, `SELECT version FROM host_command.sessions WHERE workspace=$1 AND session=$2 FOR UPDATE NOWAIT`, b.Workspace, b.Session).Scan(&version); err != nil {
		return fail(data.ErrConflict)
	}
	if expected == 0 || expected != version {
		return fail(data.ErrConflict)
	}
	var critical bool
	if err = tx.QueryRowContext(ctx, `SELECT EXISTS(SELECT 1 FROM host_command.continuations WHERE workspace=$1 AND session=$2)`, b.Workspace, b.Session).Scan(&critical); err != nil {
		return fail(err)
	}
	if critical {
		return fail(data.ErrDenied)
	}
	if err = l.authenticate(ctx, to, last); err != nil {
		return fail(err)
	}
	l.history, err = readReplay(ctx, tx, b, first.ArtifactSet)
	if err != nil {
		return fail(err)
	}
	if l.history.OriginLock == nil {
		origin := first
		l.history.OriginLock = &origin
	}
	if _, err = historyLocks(l.history, first); err != nil {
		return fail(err)
	}
	if err = r.graphFault(ctx, tx, "migration-locked"); err != nil {
		return fail(err)
	}
	return l, nil
}
func (l *MigrationLease) authenticate(ctx context.Context, g *store.Graph, want data.SessionLock) error {
	if l.closed || g == nil {
		return data.ErrDenied
	}
	m := g.Membership()
	lock, err := migration.ExactLock(g)
	if err != nil || !m.Read || !m.Install || m.Workspace != l.binding.Workspace || !reflect.DeepEqual(lock, want) {
		return data.ErrDenied
	}
	for _, p := range want.Packages {
		var actual store.GraphArtifact
		err = l.tx.QueryRowContext(ctx, `SELECT a.identity,a.package_id,a.content_hash,a.policy_digest,a.validation_digest FROM package_install.artifacts a JOIN package_install.grants g ON a.workspace=g.workspace AND a.identity=g.identity WHERE a.workspace=$1 AND g.principal=$2 AND a.identity=$3`, m.Workspace, m.Principal, p.Identity).Scan(&actual.Identity, &actual.PackageID, &actual.ContentHash, &actual.PolicyDigest, &actual.ValidationDigest)
		if err != nil {
			return err
		}
		if actual.Identity != p.Identity || actual.PackageID != p.PackageID || actual.ContentHash != p.ContentHash || actual.PolicyDigest != p.PolicyDigest || actual.ValidationDigest != p.ValidationDigest {
			return data.ErrDenied
		}
	}
	return nil
}
func (l *MigrationLease) Rollback() error {
	if l.closed {
		return nil
	}
	l.closed = true
	defer l.cancel()
	return l.tx.Rollback()
}
func (l *MigrationLease) ProvisionGraph(context.Context, *store.Graph, data.Binding, uint64, string, checkpoint.Value) error {
	return data.ErrDenied
}
func (l *MigrationLease) ReadReplayHistory(ctx context.Context, g *store.Graph, b data.Binding) (data.ReplayHistory, error) {
	if b != l.binding || l.authenticate(ctx, g, l.view) != nil {
		return data.ReplayHistory{}, data.ErrDenied
	}
	if !l.staged {
		return eventstore.Copy(l.history), nil
	}
	var raw []byte
	if err := l.tx.QueryRowContext(ctx, `SELECT history FROM host_command.migration_stage WHERE workspace=$1 AND session=$2`, b.Workspace, b.Session).Scan(&raw); err != nil {
		return data.ReplayHistory{}, err
	}
	var h data.ReplayHistory
	if checkpoint.StrictDecode(raw, &h, data.MaxPointBytes) != nil {
		return h, eventstore.ErrHistory
	}
	if _, err := historyLocks(h, l.view); err != nil {
		return h, err
	}
	return h, nil
}
func (l *MigrationLease) ReadSessionLocks(ctx context.Context, g *store.Graph, b data.Binding) ([]data.SessionLock, error) {
	h, err := l.ReadReplayHistory(ctx, g, b)
	if err != nil {
		return nil, err
	}
	return historyLocks(h, l.view)
}
func (l *MigrationLease) ReadGraphSession(ctx context.Context, g *store.Graph, b data.Binding) (data.Snapshot, error) {
	if b != l.binding || l.authenticate(ctx, g, l.view) != nil {
		return data.Snapshot{}, data.ErrDenied
	}
	return l.snapshot(ctx)
}

type migrationRead struct{ snapshot data.Snapshot }

func (t *migrationRead) Snapshot() data.Snapshot { return eventstore.Copy(t.snapshot) }
func (*migrationRead) Commit(context.Context, data.Commit) (data.Receipt, error) {
	return data.Receipt{}, data.ErrDenied
}
func (*migrationRead) Rollback() error { return nil }
func (l *MigrationLease) Begin(ctx context.Context, h data.Header) (data.Transaction, error) {
	if l.closed || !h.ReadOnly || !validHeader(h) || h.Binding != l.binding {
		return nil, data.ErrDenied
	}
	s, err := l.snapshot(ctx)
	if err != nil {
		return nil, err
	}
	if s.Version != h.ExpectedVersion {
		return nil, data.ErrConflict
	}
	return &migrationRead{snapshot: s}, nil
}
func (l *MigrationLease) snapshot(ctx context.Context) (data.Snapshot, error) {
	s := data.Snapshot{Binding: l.binding}
	var raw []byte
	if l.staged {
		if err := l.tx.QueryRowContext(ctx, `SELECT state FROM host_command.migration_stage WHERE workspace=$1 AND session=$2`, l.binding.Workspace, l.binding.Session).Scan(&raw); err != nil {
			return s, err
		}
		if checkpoint.StrictDecode(raw, &s, 1<<20) != nil || s.Binding != l.binding {
			return s, eventstore.ErrHistory
		}
	} else {
		var graph string
		if err := l.tx.QueryRowContext(ctx, `SELECT graph_hash,version,state,schema_hash FROM host_command.sessions WHERE workspace=$1 AND session=$2`, s.Binding.Workspace, s.Binding.Session).Scan(&graph, &s.Version, &raw, &s.SchemaHash); err != nil {
			return s, err
		}
		if graph != s.Binding.GraphHash || checkpoint.StrictDecode(raw, &s.State, checkpoint.MaxBytes*2) != nil {
			return s, eventstore.ErrHistory
		}
	}
	s.Rows = nil
	s.Quantities = nil
	docs := `SELECT package_id,namespace,key,schema_hash,value FROM host_command.documents WHERE workspace=$1 AND session=$2 ORDER BY package_id,namespace,key LIMIT 129`
	quantities := `SELECT package_id,key,quantity FROM host_command.quantity WHERE workspace=$1 AND session=$2 ORDER BY package_id,key LIMIT 129`
	if l.staged {
		docs = `SELECT package_id,namespace,key,schema_hash,value FROM host_command.migration_documents WHERE workspace=$1 AND session=$2 ORDER BY package_id,namespace,key LIMIT 129`
		quantities = `SELECT package_id,key,quantity FROM host_command.migration_quantity WHERE workspace=$1 AND session=$2 ORDER BY package_id,key LIMIT 129`
	}
	rows, err := l.tx.QueryContext(ctx, docs, l.binding.Workspace, l.binding.Session)
	if err != nil {
		return s, err
	}
	size := 0
	for rows.Next() {
		var v data.Row
		var value []byte
		if err = rows.Scan(&v.PackageID, &v.Namespace, &v.Key, &v.SchemaHash, &value); err != nil {
			rows.Close()
			return s, err
		}
		size += len(value)
		if len(s.Rows) >= 128 || size > 256<<10 || checkpoint.StrictDecode(value, &v.Value, checkpoint.MaxBytes*2) != nil {
			rows.Close()
			return s, eventstore.ErrHistory
		}
		s.Rows = append(s.Rows, v)
	}
	err = rows.Err()
	rows.Close()
	if err != nil {
		return s, err
	}
	rows, err = l.tx.QueryContext(ctx, quantities, l.binding.Workspace, l.binding.Session)
	if err != nil {
		return s, err
	}
	for rows.Next() {
		var v data.Quantity
		v.Table = "quantity"
		if err = rows.Scan(&v.PackageID, &v.Key, &v.Value); err != nil {
			rows.Close()
			return s, err
		}
		if len(s.Quantities) >= 128 {
			rows.Close()
			return s, eventstore.ErrHistory
		}
		s.Quantities = append(s.Quantities, v)
	}
	err = rows.Err()
	rows.Close()
	return s, err
}
func (l *MigrationLease) ReadProjectionCache(ctx context.Context, b data.Binding) (projection.Cache, error) {
	var c projection.Cache
	var raw []byte
	if l.closed || b != l.binding {
		return c, data.ErrDenied
	}
	query := `SELECT cache FROM host_command.projection_caches WHERE workspace=$1 AND session=$2`
	if l.staged {
		query = `SELECT cache FROM host_command.migration_stage WHERE workspace=$1 AND session=$2`
	}
	err := l.tx.QueryRowContext(ctx, query, b.Workspace, b.Session).Scan(&raw)
	if errors.Is(err, sql.ErrNoRows) || err == nil && raw == nil {
		return c, data.ErrNotFound
	}
	if err != nil {
		return c, err
	}
	if checkpoint.StrictDecode(raw, &c, 1<<20) != nil || c.Image.Binding != b {
		return c, checkpoint.ErrRejected
	}
	return c, nil
}
func (l *MigrationLease) ReadCheckpoint(ctx context.Context, b data.Binding) (data.CheckpointCache, error) {
	var c data.CheckpointCache
	var raw []byte
	if l.closed || b != l.binding {
		return c, data.ErrDenied
	}
	query := `SELECT cache FROM host_command.checkpoints WHERE workspace=$1 AND session=$2`
	if l.staged {
		query = `SELECT checkpoint FROM host_command.migration_stage WHERE workspace=$1 AND session=$2`
	}
	err := l.tx.QueryRowContext(ctx, query, b.Workspace, b.Session).Scan(&raw)
	if errors.Is(err, sql.ErrNoRows) || err == nil && raw == nil {
		return c, data.ErrNotFound
	}
	if err != nil {
		return c, err
	}
	if checkpoint.StrictDecode(raw, &c, 1<<20) != nil || c.Binding != b {
		return c, checkpoint.ErrRejected
	}
	return c, nil
}
func (l *MigrationLease) SaveCheckpoint(ctx context.Context, c data.CheckpointCache) error {
	if !l.staged || l.closed || c.Binding != l.binding {
		return data.ErrDenied
	}
	raw, err := hostJSON(c)
	if err != nil || len(raw) > 1<<20 {
		return data.ErrDenied
	}
	_, err = l.tx.ExecContext(ctx, `UPDATE host_command.migration_stage SET checkpoint=$3 WHERE workspace=$1 AND session=$2`, l.binding.Workspace, l.binding.Session, raw)
	return err
}
func (l *MigrationLease) RepairDerived(ctx context.Context, g *store.Graph, c projection.Cache) error {
	h, err := l.ReadReplayHistory(ctx, g, c.Image.Binding)
	if err != nil {
		return err
	}
	i, _, err := projection.Rebuild(h, c.Metadata, nil, nil)
	if err != nil || !projection.Compatible(c, c.Metadata, i) {
		return eventstore.ErrHistory
	}
	if !l.staged {
		return nil
	} // Source rehearsal cannot repair or mutate its baseline.
	raw, err := hostJSON(c)
	if err != nil {
		return err
	}
	_, err = l.tx.ExecContext(ctx, `UPDATE host_command.migration_stage SET cache=$3 WHERE workspace=$1 AND session=$2`, l.binding.Workspace, l.binding.Session, raw)
	return err
}
func sameBytes(a, b []byte) bool { return bytes.Equal(a, b) }
