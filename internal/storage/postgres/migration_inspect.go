// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package postgres

import (
	"context"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
)

type MigrationInspection struct {
	Points, Stages, Documents, Quantities, Pins int
	OriginHash, ActiveHash, PointsHash          string
}

func (r *HostRepository) InspectMigration(ctx context.Context, b data.Binding) (MigrationInspection, error) {
	var out MigrationInspection
	if !validBinding(b) {
		return out, data.ErrDenied
	}
	var origin, active []byte
	err := r.options.DB.QueryRowContext(ctx, `SELECT (SELECT count(*) FROM host_command.migration_points WHERE workspace=$1 AND session=$2),(SELECT count(*) FROM host_command.migration_stage WHERE workspace=$1 AND session=$2),(SELECT count(*) FROM host_command.migration_documents WHERE workspace=$1 AND session=$2),(SELECT count(*) FROM host_command.migration_quantity WHERE workspace=$1 AND session=$2),(SELECT count(*) FROM package_install.history_pins WHERE workspace=$1 AND session=$2),origin,active FROM host_command.session_locks WHERE workspace=$1 AND session=$2`, b.Workspace, b.Session).Scan(&out.Points, &out.Stages, &out.Documents, &out.Quantities, &out.Pins, &origin, &active)
	if err != nil {
		return out, err
	}
	out.OriginHash = checkpoint.Hash(origin)
	out.ActiveHash = checkpoint.Hash(active)
	rows, err := r.options.DB.QueryContext(ctx, `SELECT point FROM host_command.migration_points WHERE workspace=$1 AND session=$2 ORDER BY point_id LIMIT 257`, b.Workspace, b.Session)
	if err != nil {
		return out, err
	}
	defer rows.Close()
	raw := []byte{}
	for rows.Next() {
		var p []byte
		if err = rows.Scan(&p); err != nil {
			return out, err
		}
		if len(raw)+len(p) > data.MaxPointBytes {
			return out, data.ErrDenied
		}
		raw = append(raw, p...)
	}
	if err = rows.Err(); err != nil {
		return out, err
	}
	out.PointsHash = checkpoint.Hash(raw)
	return out, nil
}
func (r *HostRepository) ReadMigrationPoint(ctx context.Context, b data.Binding, id string) (data.RecoveryPoint, error) {
	var p data.RecoveryPoint
	var raw []byte
	var graph string
	if !validBinding(b) || !store.ValidID(id) {
		return p, data.ErrDenied
	}
	err := r.options.DB.QueryRowContext(ctx, `SELECT s.graph_hash,p.point FROM host_command.sessions s JOIN host_command.migration_points p USING(workspace,session) WHERE s.workspace=$1 AND s.session=$2 AND p.point_id=$3`, b.Workspace, b.Session, id).Scan(&graph, &raw)
	if err != nil {
		return p, err
	}
	if graph != b.GraphHash || checkpoint.StrictDecode(raw, &p, data.MaxPointBytes) != nil || data.ValidatePoint(p) != nil {
		return p, data.ErrDenied
	}
	return p, nil
}
