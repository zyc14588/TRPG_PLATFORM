// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package postgres

import (
	"context"
	"database/sql"
	"encoding/json"

	"github.com/zyc14588/TRPG_PLATFORM/internal/eventstore"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/projection"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
)

// These operator seams manipulate derived caches only. They expose no SQL,
// transaction, or authoritative-history mutation to a package or test caller.
func (r *HostRepository) recoveryOperator(ctx context.Context, b data.Binding) (*sql.Tx, error) {
	if !validBinding(b) {
		return nil, data.ErrDenied
	}
	tx, err := r.options.DB.BeginTx(ctx, nil)
	if err != nil {
		return nil, err
	}
	var graph string
	err = tx.QueryRowContext(ctx, `SELECT graph_hash FROM host_command.sessions WHERE workspace=$1 AND session=$2 FOR UPDATE`, b.Workspace, b.Session).Scan(&graph)
	if err != nil || graph != b.GraphHash {
		tx.Rollback()
		if err != nil {
			return nil, err
		}
		return nil, data.ErrDenied
	}
	return tx, nil
}

func (r *HostRepository) DropDerived(ctx context.Context, b data.Binding) error {
	tx, err := r.recoveryOperator(ctx, b)
	if err != nil {
		return err
	}
	defer tx.Rollback()
	for _, q := range []string{
		`UPDATE host_command.sessions SET state='{"kind":"nil"}'::jsonb WHERE workspace=$1 AND session=$2`,
		`DELETE FROM host_command.documents WHERE workspace=$1 AND session=$2`,
		`DELETE FROM host_command.quantity WHERE workspace=$1 AND session=$2`,
		`DELETE FROM host_command.checkpoints WHERE workspace=$1 AND session=$2`,
		`DELETE FROM host_command.projection_caches WHERE workspace=$1 AND session=$2`,
	} {
		if _, err = tx.ExecContext(ctx, q, b.Workspace, b.Session); err != nil {
			return err
		}
	}
	return tx.Commit()
}

// StageRecoveryCache deliberately does not trust a cache's self-hash or metadata.
// It allows bounded invalid-cache probes; the authenticated reader must reject
// them against immutable history. It never changes Session head metadata.
func (r *HostRepository) StageRecoveryCache(ctx context.Context, b data.Binding, image *projection.Cache, cp *data.CheckpointCache) error {
	tx, err := r.recoveryOperator(ctx, b)
	if err != nil {
		return err
	}
	defer tx.Rollback()
	if image != nil {
		raw, e := json.Marshal(image)
		if e != nil || len(raw) > 1<<20 {
			return checkpoint.ErrRejected
		}
		if _, err = tx.ExecContext(ctx, `INSERT INTO host_command.projection_caches(workspace,session,cache) VALUES($1,$2,$3) ON CONFLICT(workspace,session) DO UPDATE SET cache=EXCLUDED.cache`, b.Workspace, b.Session, raw); err != nil {
			return err
		}
	}
	if cp != nil {
		raw, e := json.Marshal(cp)
		if e != nil || len(raw) > checkpoint.MaxBytes*2 {
			return checkpoint.ErrRejected
		}
		if _, err = tx.ExecContext(ctx, `INSERT INTO host_command.checkpoints(workspace,session,cache) VALUES($1,$2,$3) ON CONFLICT(workspace,session) DO UPDATE SET cache=EXCLUDED.cache`, b.Workspace, b.Session, raw); err != nil {
			return err
		}
	}
	return tx.Commit()
}

type RecoveryInspection struct {
	ImmutableHash string
	DerivedHash   string
	Records       int
}

// InspectRecovery hashes exact original bytea values and typed head metadata,
// separately from rebuildable state/data/caches. All statements and ordering
// are fixed and bounded. No private payload is emitted into the evidence log.
func (r *HostRepository) InspectRecovery(ctx context.Context, b data.Binding) (RecoveryInspection, error) {
	return r.inspectRecovery(ctx, b, 32<<20)
}

// InspectRecoveryCapacity is a fixed operator-only readback for dense capacity
// fixtures. JSON bytea hex, receipts, events and audit bytes can together exceed
// the ordinary inspection bound while replay evidence still fits 4 MiB. This
// exposes neither SQL nor a configurable production limit, and writes nothing.
func (r *HostRepository) InspectRecoveryCapacity(ctx context.Context, b data.Binding) (RecoveryInspection, error) {
	return r.inspectRecovery(ctx, b, 64<<20)
}

func (r *HostRepository) inspectRecovery(ctx context.Context, b data.Binding, maxBytes int) (RecoveryInspection, error) {
	tx, err := r.recoveryOperator(ctx, b)
	if err != nil {
		return RecoveryInspection{}, err
	}
	defer tx.Rollback()
	var immutable, derived []json.RawMessage
	size := 0
	read := func(q string, dst *[]json.RawMessage) error {
		tag, _ := json.Marshal(q)
		*dst = append(*dst, json.RawMessage(tag))
		rows, e := tx.QueryContext(ctx, q, b.Workspace, b.Session)
		if e != nil {
			return e
		}
		defer rows.Close()
		n := 0
		for rows.Next() {
			var raw []byte
			if e = rows.Scan(&raw); e != nil {
				return e
			}
			n++
			size += len(raw)
			if n > eventstore.MaxRecords*128 || size > maxBytes {
				return eventstore.ErrHistory
			}
			*dst = append(*dst, append(json.RawMessage(nil), raw...))
		}
		return rows.Err()
	}
	for _, q := range []string{
		`SELECT jsonb_build_array(graph_hash,version,event_sequence,schema_hash) FROM host_command.sessions WHERE workspace=$1 AND session=$2`,
		`SELECT to_jsonb(t) FROM host_command.creation t WHERE workspace=$1 AND session=$2`,
		`SELECT to_jsonb(t) FROM host_command.installed_graphs t WHERE workspace=$1 AND session=$2`,
		`SELECT to_jsonb(t) FROM host_command.replay_effects t WHERE workspace=$1 AND session=$2 ORDER BY version`,
		`SELECT to_jsonb(t) FROM host_command.requests t WHERE workspace=$1 AND session=$2 ORDER BY command_id`,
		`SELECT to_jsonb(t) FROM host_command.events t WHERE workspace=$1 AND session=$2 ORDER BY sequence`,
		`SELECT to_jsonb(t) FROM host_command.patches t WHERE workspace=$1 AND session=$2 ORDER BY command_id,ordinal`,
		`SELECT to_jsonb(t) FROM host_command.tasks t WHERE workspace=$1 AND session=$2 ORDER BY id`,
		`SELECT to_jsonb(t) FROM host_command.continuations t WHERE workspace=$1 AND session=$2 ORDER BY id`,
		`SELECT to_jsonb(t) FROM host_command.outbox t WHERE workspace=$1 AND session=$2 ORDER BY id`,
		`SELECT to_jsonb(t) FROM host_command.audit t WHERE workspace=$1 AND session=$2 ORDER BY command_id,ordinal`,
		`SELECT to_jsonb(t) FROM host_command.endings t WHERE workspace=$1 AND session=$2`,
		`SELECT to_jsonb(t) FROM package_install.data_targets t WHERE workspace=$1 AND state_reference='host-session:'||$2 ORDER BY package_id`,
	} {
		if err = read(q, &immutable); err != nil {
			return RecoveryInspection{}, err
		}
	}
	for _, q := range []string{
		`SELECT state FROM host_command.sessions WHERE workspace=$1 AND session=$2`,
		`SELECT to_jsonb(t) FROM host_command.documents t WHERE workspace=$1 AND session=$2 ORDER BY package_id,namespace,key`,
		`SELECT to_jsonb(t) FROM host_command.quantity t WHERE workspace=$1 AND session=$2 ORDER BY package_id,key`,
		`SELECT to_jsonb(t) FROM host_command.checkpoints t WHERE workspace=$1 AND session=$2`,
		`SELECT to_jsonb(t) FROM host_command.projection_caches t WHERE workspace=$1 AND session=$2`,
	} {
		if err = read(q, &derived); err != nil {
			return RecoveryInspection{}, err
		}
	}
	var records int
	if err = tx.QueryRowContext(ctx, `SELECT count(*) FROM host_command.replay_effects WHERE workspace=$1 AND session=$2`, b.Workspace, b.Session).Scan(&records); err != nil {
		return RecoveryInspection{}, err
	}
	if err = tx.Commit(); err != nil {
		return RecoveryInspection{}, err
	}
	return RecoveryInspection{eventstore.Digest(immutable), eventstore.Digest(derived), records}, nil
}
