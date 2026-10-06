// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package postgres

import (
	"context"
	"reflect"

	"github.com/zyc14588/TRPG_PLATFORM/internal/eventstore"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	"github.com/zyc14588/TRPG_PLATFORM/internal/projection"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
)

func (l *MigrationLease) Stage(ctx context.Context, r data.EffectRecord, c projection.Cache, cp data.CheckpointCache) error {
	if l.closed || l.staged || l.point == nil || r.Migration == nil || r.Migration.PointHash != l.point.Hash || r.Header.Binding != l.binding || r.Header.ExpectedVersion != l.history.Version || r.Migration.From.Hash != l.source.Hash || r.Migration.To.Hash != l.target.Hash || eventstore.Validate(r) != nil {
		return data.ErrDenied
	}
	if r.Migration.Direction == "upgrade" && r.Hash != l.reserved[0] {
		return data.ErrDenied
	}
	h := eventstore.Copy(l.history)
	h.Records = append(h.Records, r)
	h.Version = r.Version
	h.Cursor = r.Cursor
	h.Ended = r.Ended
	i, _, err := projection.Rebuild(h, c.Metadata, nil, nil)
	if err != nil || !projection.Compatible(c, c.Metadata, i) || cp.Recovery == nil || !reflect.DeepEqual(*cp.Recovery, c.Metadata) || cp.Binding != i.Binding || cp.Version != i.Version || cp.Cursor != i.Cursor || cp.HistoryHash != i.HistoryHash || cp.Hash != eventstore.Digest(i.State) || eventstore.Digest(cp.Value) != cp.Hash {
		return eventstore.ErrHistory
	}
	raw, err := hostJSON(h)
	if err != nil || len(raw) > data.MaxPointBytes {
		return eventstore.ErrHistory
	}
	snapshot := data.Snapshot{Binding: i.Binding, Version: i.Version, State: i.State, SchemaHash: i.StateSchema, Rows: i.Rows, Quantities: i.Quantities}
	state, _ := hostJSON(snapshot)
	cache, _ := hostJSON(c)
	checkpointBytes, _ := hostJSON(cp)
	if _, err = l.tx.ExecContext(ctx, `INSERT INTO host_command.migration_stage(workspace,session,point_id,history,state,cache,checkpoint) VALUES($1,$2,$3,$4,$5,$6,$7)`, l.binding.Workspace, l.binding.Session, l.point.ID, raw, state, cache, checkpointBytes); err != nil {
		return err
	}
	for _, v := range i.Rows {
		id, eid := provenance(h, v.PackageID, v.Namespace, v.Key, false)
		value, _ := hostJSON(v.Value)
		if id == "" || eid == "" {
			return eventstore.ErrHistory
		}
		if _, err = l.tx.ExecContext(ctx, `INSERT INTO host_command.migration_documents(workspace,session,package_id,namespace,key,schema_hash,value,command_id,event_id) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9)`, l.binding.Workspace, l.binding.Session, v.PackageID, v.Namespace, v.Key, v.SchemaHash, value, id, eid); err != nil {
			return err
		}
	}
	for _, v := range i.Quantities {
		id, eid := provenance(h, v.PackageID, v.Table, v.Key, true)
		if id == "" || eid == "" {
			return eventstore.ErrHistory
		}
		if _, err = l.tx.ExecContext(ctx, `INSERT INTO host_command.migration_quantity(workspace,session,package_id,key,quantity,command_id,event_id) VALUES($1,$2,$3,$4,$5,$6,$7)`, l.binding.Workspace, l.binding.Session, v.PackageID, v.Key, v.Value, id, eid); err != nil {
			return err
		}
	}
	l.staged = true
	l.view = l.target
	l.binding = i.Binding
	if err = l.repository.graphFault(ctx, l.tx, "migration-stage"); err != nil {
		return err
	}
	return nil
}

// CommitMigration publishes the already rehearsed SQL image atomically. The
// immutable old event, creation, request and receipt rows are never updated.
func (l *MigrationLease) CommitMigration(ctx context.Context, g *store.Graph, r data.EffectRecord, c projection.Cache, cp data.CheckpointCache) (data.Receipt, error) {
	if l.closed || !l.staged || l.point == nil || r.Migration == nil || r.Migration.PointHash != l.point.Hash || r.Migration.To.Hash != l.view.Hash || l.authenticate(ctx, g, l.view) != nil {
		return data.Receipt{}, data.ErrDenied
	}
	h, err := l.ReadReplayHistory(ctx, g, l.binding)
	if err != nil {
		return data.Receipt{}, err
	}
	if len(h.Records) == 0 || h.Records[len(h.Records)-1].Hash != r.Hash {
		return data.Receipt{}, eventstore.ErrHistory
	}
	i, _, err := projection.Rebuild(h, c.Metadata, nil, nil)
	if err != nil || !projection.Compatible(c, c.Metadata, i) {
		return data.Receipt{}, eventstore.ErrHistory
	}
	actual, err := l.snapshot(ctx)
	if err != nil || eventstore.Digest(actual) != eventstore.Digest(data.Snapshot{Binding: i.Binding, Version: i.Version, State: i.State, SchemaHash: i.StateSchema, Rows: i.Rows, Quantities: i.Quantities}) {
		return data.Receipt{}, eventstore.ErrHistory
	}
	stagedCP, err := l.ReadCheckpoint(ctx, l.binding)
	if err != nil || !reflect.DeepEqual(stagedCP, cp) {
		return data.Receipt{}, eventstore.ErrHistory
	}
	if err = l.repository.graphFault(ctx, l.tx, "migration-validated"); err != nil {
		return data.Receipt{}, err
	}
	b := l.binding
	restore := r.Migration.Direction == "restore-point"
	state, _ := hostJSON(i.State)
	cache, _ := hostJSON(c)
	checkpointBytes, _ := hostJSON(cp)
	graph := l.view.ArtifactSet
	if restore {
		state = l.point.State
		cache = l.point.Projection
		checkpointBytes = l.point.Checkpoint
		graph = l.point.Graph
	}
	if !sameBytes(graph, l.view.ArtifactSet) {
		return data.Receipt{}, data.ErrDenied
	}
	if _, err = l.tx.ExecContext(ctx, `UPDATE host_command.sessions SET graph_hash=$3,version=$4,state=$5::jsonb,schema_hash=$6 WHERE workspace=$1 AND session=$2`, b.Workspace, b.Session, b.GraphHash, int64(i.Version), string(state), i.StateSchema); err != nil {
		return data.Receipt{}, err
	}
	for _, q := range []string{`DELETE FROM host_command.documents WHERE workspace=$1 AND session=$2`, `DELETE FROM host_command.quantity WHERE workspace=$1 AND session=$2`} {
		if _, err = l.tx.ExecContext(ctx, q, b.Workspace, b.Session); err != nil {
			return data.Receipt{}, err
		}
	}
	if restore {
		for _, v := range l.point.Documents {
			if _, err = l.tx.ExecContext(ctx, `INSERT INTO host_command.documents(workspace,session,package_id,namespace,key,schema_hash,value,command_id,event_id) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9)`, b.Workspace, b.Session, v.Row.PackageID, v.Row.Namespace, v.Row.Key, v.Row.SchemaHash, v.Bytes, v.CommandID, v.EventID); err != nil {
				return data.Receipt{}, err
			}
		}
		for _, v := range l.point.Quantities {
			if _, err = l.tx.ExecContext(ctx, `INSERT INTO host_command.quantity(workspace,session,package_id,key,quantity,command_id,event_id) VALUES($1,$2,$3,$4,$5,$6,$7)`, b.Workspace, b.Session, v.Quantity.PackageID, v.Quantity.Key, v.Quantity.Value, v.CommandID, v.EventID); err != nil {
				return data.Receipt{}, err
			}
		}
	} else {
		if _, err = l.tx.ExecContext(ctx, `INSERT INTO host_command.documents SELECT * FROM host_command.migration_documents WHERE workspace=$1 AND session=$2`, b.Workspace, b.Session); err != nil {
			return data.Receipt{}, err
		}
		if _, err = l.tx.ExecContext(ctx, `INSERT INTO host_command.quantity SELECT * FROM host_command.migration_quantity WHERE workspace=$1 AND session=$2`, b.Workspace, b.Session); err != nil {
			return data.Receipt{}, err
		}
	}
	if err = l.repository.graphFault(ctx, l.tx, "migration-data"); err != nil {
		return data.Receipt{}, err
	}
	receipt := effectReceipt(r)
	receiptBytes, _ := hostJSON(receipt)
	evidence, _ := hostJSON(r)
	if _, err = l.tx.ExecContext(ctx, `INSERT INTO host_command.requests(workspace,session,command_id,principal,fingerprint,receipt) VALUES($1,$2,$3,$4,$5,$6)`, b.Workspace, b.Session, r.Header.CommandID, r.Header.Principal, r.Header.Fingerprint, receiptBytes); err != nil {
		return data.Receipt{}, err
	}
	if _, err = l.tx.ExecContext(ctx, `INSERT INTO host_command.replay_effects(workspace,session,version,command_id,evidence) VALUES($1,$2,$3,$4,$5)`, b.Workspace, b.Session, int64(r.Version), r.Header.CommandID, evidence); err != nil {
		return data.Receipt{}, err
	}
	for n, p := range r.Patches {
		raw, _ := hostJSON(p)
		if _, err = l.tx.ExecContext(ctx, `INSERT INTO host_command.patches(workspace,session,command_id,ordinal,event_id,patch) VALUES($1,$2,$3,$4,$5,$6)`, b.Workspace, b.Session, r.Header.CommandID, n, p.EventID, raw); err != nil {
			return data.Receipt{}, err
		}
	}
	audit, _ := hostJSON(data.Audit{Workspace: b.Workspace, Session: b.Session, CommandID: r.Header.CommandID, Level: "info", Operation: "package-migration", Phase: r.Migration.Direction, ArgumentsHash: eventstore.Digest([]string{l.source.Hash, l.target.Hash, l.point.Hash}), ResultHash: eventstore.Digest(receipt), Outcome: "PASS", StateVersion: r.Version})
	if _, err = l.tx.ExecContext(ctx, `INSERT INTO host_command.audit(workspace,session,command_id,ordinal,record) VALUES($1,$2,$3,0,$4)`, b.Workspace, b.Session, r.Header.CommandID, audit); err != nil {
		return data.Receipt{}, err
	}
	if err = l.repository.graphFault(ctx, l.tx, "migration-effect"); err != nil {
		return data.Receipt{}, err
	}
	active, _ := hostJSON(l.view)
	if _, err = l.tx.ExecContext(ctx, `INSERT INTO host_command.session_locks(workspace,session,origin,active) VALUES($1,$2,$3,$4) ON CONFLICT(workspace,session) DO UPDATE SET active=EXCLUDED.active`, b.Workspace, b.Session, l.point.OriginLock, active); err != nil {
		return data.Receipt{}, err
	}
	if _, err = l.tx.ExecContext(ctx, `UPDATE host_command.installed_graphs SET graph_bytes=$3 WHERE workspace=$1 AND session=$2`, b.Workspace, b.Session, graph); err != nil {
		return data.Receipt{}, err
	}
	if _, err = l.tx.ExecContext(ctx, `DELETE FROM package_install.data_targets WHERE workspace=$1 AND state_reference=$2`, b.Workspace, "host-session:"+b.Session); err != nil {
		return data.Receipt{}, err
	}
	for _, v := range l.view.Packages {
		if _, err = l.tx.ExecContext(ctx, `INSERT INTO package_install.data_targets(workspace,package_id,state_reference) VALUES($1,$2,$3)`, b.Workspace, v.PackageID, "host-session:"+b.Session); err != nil {
			return data.Receipt{}, err
		}
	}
	for _, lock := range []data.SessionLock{l.source, l.target} {
		for _, v := range lock.Packages {
			if _, err = l.tx.ExecContext(ctx, `INSERT INTO package_install.history_pins(workspace,session,identity,package_id) VALUES($1,$2,$3,$4) ON CONFLICT DO NOTHING`, b.Workspace, b.Session, v.Identity, v.PackageID); err != nil {
				return data.Receipt{}, err
			}
		}
	}
	if err = l.repository.graphFault(ctx, l.tx, "migration-lock"); err != nil {
		return data.Receipt{}, err
	}
	if _, err = l.tx.ExecContext(ctx, `DELETE FROM host_command.projection_caches WHERE workspace=$1 AND session=$2`, b.Workspace, b.Session); err != nil {
		return data.Receipt{}, err
	}
	if cache != nil {
		if _, err = l.tx.ExecContext(ctx, `INSERT INTO host_command.projection_caches(workspace,session,cache) VALUES($1,$2,$3)`, b.Workspace, b.Session, cache); err != nil {
			return data.Receipt{}, err
		}
	}
	if _, err = l.tx.ExecContext(ctx, `DELETE FROM host_command.checkpoints WHERE workspace=$1 AND session=$2`, b.Workspace, b.Session); err != nil {
		return data.Receipt{}, err
	}
	if checkpointBytes != nil {
		if _, err = l.tx.ExecContext(ctx, `INSERT INTO host_command.checkpoints(workspace,session,cache) VALUES($1,$2,$3)`, b.Workspace, b.Session, checkpointBytes); err != nil {
			return data.Receipt{}, err
		}
	}
	if err = l.repository.graphFault(ctx, l.tx, "migration-cache"); err != nil {
		return data.Receipt{}, err
	}
	if restore {
		_, err = l.tx.ExecContext(ctx, `UPDATE host_command.migration_points SET restored=true WHERE workspace=$1 AND session=$2 AND point_id=$3`, b.Workspace, b.Session, l.point.ID)
	} else {
		_, err = l.tx.ExecContext(ctx, `UPDATE host_command.migration_points SET applied_version=$4 WHERE workspace=$1 AND session=$2 AND point_id=$3`, b.Workspace, b.Session, l.point.ID, int64(r.Version))
	}
	if err != nil {
		return data.Receipt{}, err
	}
	if _, err = l.tx.ExecContext(ctx, `DELETE FROM host_command.migration_stage WHERE workspace=$1 AND session=$2`, b.Workspace, b.Session); err != nil {
		return data.Receipt{}, err
	}
	if err = l.repository.graphFault(ctx, l.tx, "migration-commit"); err != nil {
		return data.Receipt{}, err
	}
	if err = l.tx.Commit(); err != nil {
		return data.Receipt{}, err
	}
	l.closed = true
	l.cancel()
	return receipt, nil
}
func (r *HostRepository) WithMigrationTransactionAbort(point string) (*HostRepository, error) {
	allowed := map[string]bool{"migration-locked": true, "migration-point": true, "migration-stage": true, "migration-validated": true, "migration-data": true, "migration-effect": true, "migration-lock": true, "migration-cache": true, "migration-commit": true}
	if !allowed[point] {
		return nil, data.ErrDenied
	}
	o := r.options
	o.AbortAt = point
	return &HostRepository{options: o}, nil
}
