// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package postgres

import (
	"context"
	"database/sql"
	"errors"
	"reflect"

	"github.com/zyc14588/TRPG_PLATFORM/internal/eventstore"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	"github.com/zyc14588/TRPG_PLATFORM/internal/projection"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
)

func (l *MigrationLease) CapturePoint(ctx context.Context, id string, verified data.Snapshot, historyHash string, cp data.CheckpointCache) (data.RecoveryPoint, error) {
	p := data.RecoveryPoint{ID: id, Lock: l.source, Version: l.history.Version, Cursor: l.history.Cursor, Ended: l.history.Ended, Verified: eventstore.Copy(verified), HistoryHash: historyHash, VerifiedCheckpoint: eventstore.Copy(cp)}
	if l.closed || l.staged || !store.ValidID(id) || verified.Binding != l.binding || verified.Version != p.Version || cp.Binding != l.binding || cp.Version != p.Version || cp.HistoryHash != historyHash || cp.Hash != eventstore.Digest(verified.State) {
		return p, data.ErrDenied
	}
	err := l.tx.QueryRowContext(ctx, `SELECT s.state::text,g.graph_bytes FROM host_command.sessions s JOIN host_command.installed_graphs g USING(workspace,session) WHERE s.workspace=$1 AND s.session=$2`, l.binding.Workspace, l.binding.Session).Scan(&p.State, &p.Graph)
	if err != nil {
		return p, err
	}
	if err = l.tx.QueryRowContext(ctx, `SELECT origin,active FROM host_command.session_locks WHERE workspace=$1 AND session=$2`, l.binding.Workspace, l.binding.Session).Scan(&p.OriginLock, &p.ActiveLock); err != nil && !errors.Is(err, sql.ErrNoRows) {
		return p, err
	}
	if p.ActiveLock == nil {
		p.OriginLock, _ = hostJSON(l.source)
		p.ActiveLock = append([]byte(nil), p.OriginLock...)
	}
	if err = l.tx.QueryRowContext(ctx, `SELECT cache FROM host_command.projection_caches WHERE workspace=$1 AND session=$2`, l.binding.Workspace, l.binding.Session).Scan(&p.Projection); err != nil && !errors.Is(err, sql.ErrNoRows) {
		return p, err
	}
	if err = l.tx.QueryRowContext(ctx, `SELECT cache FROM host_command.checkpoints WHERE workspace=$1 AND session=$2`, l.binding.Workspace, l.binding.Session).Scan(&p.Checkpoint); err != nil && !errors.Is(err, sql.ErrNoRows) {
		return p, err
	}
	rows, err := l.tx.QueryContext(ctx, `SELECT package_id,namespace,key,schema_hash,value,command_id,event_id FROM host_command.documents WHERE workspace=$1 AND session=$2 ORDER BY package_id,namespace,key LIMIT 129`, l.binding.Workspace, l.binding.Session)
	if err != nil {
		return p, err
	}
	for rows.Next() {
		var v data.PointDocument
		if err = rows.Scan(&v.Row.PackageID, &v.Row.Namespace, &v.Row.Key, &v.Row.SchemaHash, &v.Bytes, &v.CommandID, &v.EventID); err != nil {
			rows.Close()
			return p, err
		}
		if len(p.Documents) >= 128 || checkpoint.StrictDecode(v.Bytes, &v.Row.Value, checkpoint.MaxBytes*2) != nil {
			rows.Close()
			return p, eventstore.ErrHistory
		}
		p.Documents = append(p.Documents, v)
	}
	err = rows.Err()
	rows.Close()
	if err != nil {
		return p, err
	}
	rows, err = l.tx.QueryContext(ctx, `SELECT package_id,key,quantity,command_id,event_id FROM host_command.quantity WHERE workspace=$1 AND session=$2 ORDER BY package_id,key LIMIT 129`, l.binding.Workspace, l.binding.Session)
	if err != nil {
		return p, err
	}
	for rows.Next() {
		var v data.PointQuantity
		v.Quantity.Table = "quantity"
		if err = rows.Scan(&v.Quantity.PackageID, &v.Quantity.Key, &v.Quantity.Value, &v.CommandID, &v.EventID); err != nil {
			rows.Close()
			return p, err
		}
		if len(p.Quantities) >= 128 {
			rows.Close()
			return p, eventstore.ErrHistory
		}
		p.Quantities = append(p.Quantities, v)
	}
	err = rows.Err()
	rows.Close()
	if err != nil {
		return p, err
	}
	rows, err = l.tx.QueryContext(ctx, `SELECT package_id FROM package_install.data_targets WHERE workspace=$1 AND state_reference=$2 ORDER BY package_id LIMIT 129`, l.binding.Workspace, "host-session:"+l.binding.Session)
	if err != nil {
		return p, err
	}
	for rows.Next() {
		var id string
		if err = rows.Scan(&id); err != nil {
			rows.Close()
			return p, err
		}
		p.Targets = append(p.Targets, id)
	}
	err = rows.Err()
	rows.Close()
	if err != nil {
		return p, err
	}
	for _, a := range p.Lock.Packages {
		rows, err = l.tx.QueryContext(ctx, `SELECT identity,path,object_key FROM package_install.objects WHERE workspace=$1 AND identity=$2 ORDER BY path LIMIT 4097`, l.binding.Workspace, a.Identity)
		if err != nil {
			return p, err
		}
		for rows.Next() {
			var v data.PointObject
			if err = rows.Scan(&v.Identity, &v.Path, &v.Key); err != nil {
				rows.Close()
				return p, err
			}
			if len(p.Objects) >= 4096 {
				rows.Close()
				return p, eventstore.ErrHistory
			}
			p.Objects = append(p.Objects, v)
		}
		err = rows.Err()
		rows.Close()
		if err != nil {
			return p, err
		}
	}
	p.Hash = data.PointHash(p)
	if data.ValidatePoint(p) != nil {
		return p, data.ErrDenied
	}
	return eventstore.Copy(p), nil
}

// ReserveUpgrade checks the upgrade AND immediate point restoration before the
// point or isolation tables can be written. Head/history budgets are unchanged.
func (l *MigrationLease) ReserveUpgrade(ctx context.Context, p data.RecoveryPoint, up, restore data.EffectRecord) error {
	if l.closed || l.staged || data.ValidatePoint(p) != nil || p.Lock.Hash != l.source.Hash || p.Version != l.history.Version || eventstore.Validate(up) != nil || eventstore.Validate(restore) != nil || up.Migration == nil || restore.Migration == nil || up.Migration.PointHash != p.Hash || restore.Migration.PointHash != p.Hash || up.Migration.Direction != "upgrade" || restore.Migration.Direction != "restore-point" || up.Migration.From.Hash != l.source.Hash || up.Migration.To.Hash != l.target.Hash || restore.Migration.From.Hash != l.target.Hash || restore.Migration.To.Hash != l.source.Hash || restore.Version != up.Version+1 {
		return data.ErrDenied
	}
	if len(l.history.Records)+2 > eventstore.MaxRecords {
		return eventstore.ErrHistory
	}
	size, receipts := 0, 0
	for _, r := range append(eventstore.Copy(l.history.Records), up, restore) {
		raw, _ := hostJSON(r)
		receipt, _ := hostJSON(effectReceipt(r))
		size += len(raw)
		receipts += len(receipt)
	}
	if size > eventstore.MaxHistoryBytes || size+receipts > eventstore.MaxHistoryBytes*2 {
		return eventstore.ErrHistory
	}
	i, err := projection.Genesis(l.history.Creation)
	if err != nil {
		return err
	}
	for _, r := range append(eventstore.Copy(l.history.Records), up, restore) {
		i, err = projection.Apply(i, r)
		if err != nil {
			return err
		}
	}
	if !reflect.DeepEqual(i.State, p.Verified.State) || !reflect.DeepEqual(i.Rows, p.Verified.Rows) || !reflect.DeepEqual(i.Quantities, p.Verified.Quantities) || i.Binding.GraphHash != p.Lock.GraphHash {
		return eventstore.ErrHistory
	}
	for _, r := range []data.EffectRecord{up, restore} {
		var exists bool
		if err = l.tx.QueryRowContext(ctx, `SELECT EXISTS(SELECT 1 FROM host_command.requests WHERE workspace=$1 AND session=$2 AND command_id=$3)`, l.binding.Workspace, l.binding.Session, r.Header.CommandID).Scan(&exists); err != nil {
			return err
		}
		if exists {
			return data.ErrConflict
		}
	}
	l.reserved = [2]string{up.Hash, restore.Hash}
	return nil
}
func effectReceipt(r data.EffectRecord) data.Receipt {
	return data.Receipt{Header: r.Header, Version: r.Version, Result: r.Result, Events: r.Events, Inputs: r.Inputs, Cursor: r.Cursor}
}
func (l *MigrationLease) SavePoint(ctx context.Context, p data.RecoveryPoint) error {
	if l.closed || l.point != nil || l.reserved[0] == "" || data.ValidatePoint(p) != nil {
		return data.ErrDenied
	}
	var used int64
	if err := l.tx.QueryRowContext(ctx, `SELECT COALESCE(sum(octet_length(point)),0) FROM host_command.migration_points WHERE workspace=$1 AND session=$2`, l.binding.Workspace, l.binding.Session).Scan(&used); err != nil {
		return err
	}
	raw, _ := hostJSON(p)
	if used+int64(len(raw)) > data.MaxPointBytes {
		return eventstore.ErrHistory
	}
	if _, err := l.tx.ExecContext(ctx, `INSERT INTO host_command.migration_points(workspace,session,point_id,point) VALUES($1,$2,$3,$4)`, l.binding.Workspace, l.binding.Session, p.ID, raw); err != nil {
		return err
	}
	owned := eventstore.Copy(p)
	l.point = &owned
	return l.repository.graphFault(ctx, l.tx, "migration-point")
}
func (l *MigrationLease) LoadPoint(ctx context.Context, id string) (data.RecoveryPoint, error) {
	var p data.RecoveryPoint
	var raw []byte
	var applied uint64
	var restored bool
	if l.closed || !store.ValidID(id) {
		return p, data.ErrDenied
	}
	err := l.tx.QueryRowContext(ctx, `SELECT point,applied_version,restored FROM host_command.migration_points WHERE workspace=$1 AND session=$2 AND point_id=$3`, l.binding.Workspace, l.binding.Session, id).Scan(&raw, &applied, &restored)
	if err != nil {
		return p, err
	}
	if restored || applied != l.history.Version || checkpoint.StrictDecode(raw, &p, data.MaxPointBytes) != nil || data.ValidatePoint(p) != nil || p.Lock.Hash != l.target.Hash || p.Version+1 != applied {
		return p, data.ErrConflict
	}
	if len(l.history.Records) == 0 {
		return p, eventstore.ErrHistory
	}
	last := l.history.Records[len(l.history.Records)-1]
	if last.Migration == nil || last.Migration.Direction != "upgrade" || last.Migration.PointID != id || last.Migration.PointHash != p.Hash || last.Migration.To.Hash != l.source.Hash {
		return p, eventstore.ErrHistory
	}
	for _, v := range p.Objects {
		var key string
		if err = l.tx.QueryRowContext(ctx, `SELECT object_key FROM package_install.objects WHERE workspace=$1 AND identity=$2 AND path=$3`, l.binding.Workspace, v.Identity, v.Path).Scan(&key); err != nil || key != v.Key {
			return p, data.ErrDenied
		}
	}
	owned := eventstore.Copy(p)
	l.point = &owned
	return p, l.repository.graphFault(ctx, l.tx, "migration-point")
}
