// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package postgres

import (
	"bytes"
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

// All immutable evidence and head metadata are read while the same Session row
// lock used by commands is held. The workspace/installed graph is authenticated
// first. No mutable state, document, quantity, or checkpoint seeds this read.
func (r *HostRepository) ReadReplayHistory(ctx context.Context, g *store.Graph, b data.Binding) (data.ReplayHistory, error) {
	tx, expected, err := r.installedTransaction(ctx, g, b)
	if err != nil {
		return data.ReplayHistory{}, err
	}
	defer tx.Rollback()
	h, err := readReplay(ctx, tx, b, expected)
	if err != nil {
		return data.ReplayHistory{}, err
	}
	if err = tx.Commit(); err != nil {
		return data.ReplayHistory{}, err
	}
	return h, nil
}
func readReplay(ctx context.Context, tx *sql.Tx, b data.Binding, expected []byte) (data.ReplayHistory, error) {
	var h data.ReplayHistory
	var graph, schema string
	var raw, artifacts []byte
	err := tx.QueryRowContext(ctx, `SELECT s.graph_hash,s.version,s.event_sequence,s.schema_hash,g.graph_bytes,c.evidence,EXISTS(SELECT 1 FROM host_command.endings e WHERE e.workspace=s.workspace AND e.session=s.session) FROM host_command.sessions s JOIN host_command.installed_graphs g ON s.workspace=g.workspace AND s.session=g.session JOIN host_command.creation c ON s.workspace=c.workspace AND s.session=c.session WHERE s.workspace=$1 AND s.session=$2 FOR UPDATE OF s`, b.Workspace, b.Session).Scan(&graph, &h.Version, &h.Cursor, &schema, &artifacts, &raw, &h.Ended)
	if err != nil {
		return h, err
	}
	if graph != b.GraphHash || !bytes.Equal(artifacts, expected) || checkpoint.StrictDecode(raw, &h.Creation, checkpoint.MaxBytes*2) != nil || h.Creation.Binding != b || h.Creation.SchemaHash != schema || h.Creation.ArtifactsHash != checkpoint.Hash(expected) {
		return data.ReplayHistory{}, eventstore.ErrHistory
	}
	if _, err = projection.Genesis(h.Creation); err != nil {
		return data.ReplayHistory{}, err
	}
	var graphArtifacts []store.GraphArtifact
	if checkpoint.StrictDecode(expected, &graphArtifacts, 1<<20) != nil || len(graphArtifacts) == 0 || len(graphArtifacts) > 128 {
		return h, eventstore.ErrHistory
	}
	for _, a := range graphArtifacts {
		var exists bool
		if err = tx.QueryRowContext(ctx, `SELECT EXISTS(SELECT 1 FROM package_install.data_targets WHERE workspace=$1 AND package_id=$2 AND state_reference=$3)`, b.Workspace, a.PackageID, "host-session:"+b.Session).Scan(&exists); err != nil {
			return h, err
		}
		if !exists {
			return h, eventstore.ErrHistory
		}
	}
	rows, err := tx.QueryContext(ctx, `SELECT e.version,e.command_id,e.evidence,r.receipt FROM host_command.replay_effects e JOIN host_command.requests r ON e.workspace=r.workspace AND e.session=r.session AND e.command_id=r.command_id WHERE e.workspace=$1 AND e.session=$2 ORDER BY e.version LIMIT $3`, b.Workspace, b.Session, eventstore.MaxRecords+1)
	if err != nil {
		return h, err
	}
	size := 0
	for rows.Next() {
		var version uint64
		var id string
		var evidence, receipt []byte
		if err = rows.Scan(&version, &id, &evidence, &receipt); err != nil {
			rows.Close()
			return h, err
		}
		size += len(evidence) + len(receipt)
		if len(h.Records) >= eventstore.MaxRecords || size > eventstore.MaxHistoryBytes*2 {
			rows.Close()
			return h, eventstore.ErrHistory
		}
		record, e := eventstore.Decode(evidence)
		if e != nil || record.Header.Binding != b || record.Version != version || record.Header.CommandID != id || version != h.Creation.Version+uint64(len(h.Records))+1 {
			rows.Close()
			return h, eventstore.ErrHistory
		}
		want, _ := hostJSON(data.Receipt{Header: record.Header, Version: record.Version, Result: record.Result, Events: record.Events, Inputs: record.Inputs, Cursor: record.Cursor})
		if !bytes.Equal(want, receipt) {
			rows.Close()
			return h, eventstore.ErrHistory
		}
		h.Records = append(h.Records, record)
	}
	err = rows.Err()
	rows.Close()
	if err != nil {
		return h, err
	}
	if h.Version != h.Creation.Version+uint64(len(h.Records)) {
		return h, eventstore.ErrHistory
	}
	// Compare original event bytes/versions/order, not merely a cache digest.
	rows, err = tx.QueryContext(ctx, `SELECT sequence,version,command_id,event_id,event_type,payload,schema_version,schema_hash FROM host_command.events WHERE workspace=$1 AND session=$2 ORDER BY sequence LIMIT $3`, b.Workspace, b.Session, eventstore.MaxRecords*64+1)
	if err != nil {
		return h, err
	}
	defer rows.Close()
	index := 0
	var sequence uint64
	for _, record := range h.Records {
		for _, event := range record.Events {
			if !rows.Next() {
				return h, eventstore.ErrHistory
			}
			var version, sv, seq uint64
			var id, eid, typ, sh string
			var payload []byte
			if err = rows.Scan(&seq, &version, &id, &eid, &typ, &payload, &sv, &sh); err != nil {
				return h, err
			}
			want, _ := hostJSON(event.Payload)
			sequence++
			index++
			if seq != sequence || version != record.Version || id != record.Header.CommandID || eid != event.ID || typ != event.Type || sv != event.SchemaVersion || sh != event.SchemaHash || !bytes.Equal(payload, want) {
				return h, eventstore.ErrHistory
			}
		}
	}
	if rows.Next() || rows.Err() != nil || sequence != h.Cursor || index > eventstore.MaxRecords*64 {
		return h, eventstore.ErrHistory
	}
	return h, nil
}

func (r *HostRepository) ReadProjectionCache(ctx context.Context, b data.Binding) (projection.Cache, error) {
	var c projection.Cache
	var graph string
	var raw []byte
	if !validBinding(b) {
		return c, data.ErrDenied
	}
	err := r.options.DB.QueryRowContext(ctx, `SELECT s.graph_hash,c.cache FROM host_command.sessions s JOIN host_command.projection_caches c ON s.workspace=c.workspace AND s.session=c.session WHERE s.workspace=$1 AND s.session=$2`, b.Workspace, b.Session).Scan(&graph, &raw)
	if errors.Is(err, sql.ErrNoRows) {
		return c, data.ErrNotFound
	}
	if err != nil {
		return c, err
	}
	if graph != b.GraphHash || checkpoint.StrictDecode(raw, &c, 1<<20) != nil || c.Image.Binding != b {
		return projection.Cache{}, checkpoint.ErrRejected
	}
	return c, nil
}

// RepairDerived rechecks the locked immutable history and recomputes the image
// before repairing only caches. An arbitrary self-hashed snapshot cannot write
// authoritative facts, change a head/version, or dispatch any recorded intent.
func (r *HostRepository) RepairDerived(ctx context.Context, g *store.Graph, c projection.Cache) error {
	b := c.Image.Binding
	tx, expected, err := r.installedTransaction(ctx, g, b)
	if err != nil {
		return err
	}
	defer tx.Rollback()
	h, err := readReplay(ctx, tx, b, expected)
	if err != nil {
		return err
	}
	i, _, err := projection.Rebuild(h, c.Metadata, nil, nil)
	if err != nil || !projection.Compatible(c, c.Metadata, i) || !reflect.DeepEqual(i, c.Image) {
		return eventstore.ErrHistory
	}
	state, err := hostJSON(i.State)
	if err != nil {
		return err
	}
	if _, err = tx.ExecContext(ctx, `UPDATE host_command.sessions SET state=$3::jsonb WHERE workspace=$1 AND session=$2`, b.Workspace, b.Session, string(state)); err != nil {
		return err
	}
	if err = r.graphFault(ctx, tx, "recovery-after-state"); err != nil {
		return err
	}
	if _, err = tx.ExecContext(ctx, `DELETE FROM host_command.documents WHERE workspace=$1 AND session=$2`, b.Workspace, b.Session); err != nil {
		return err
	}
	if _, err = tx.ExecContext(ctx, `DELETE FROM host_command.quantity WHERE workspace=$1 AND session=$2`, b.Workspace, b.Session); err != nil {
		return err
	}
	for _, row := range i.Rows {
		id, event := provenance(h, row.PackageID, row.Namespace, row.Key, false)
		raw, _ := hostJSON(row.Value)
		if id == "" || event == "" {
			return eventstore.ErrHistory
		}
		if _, err = tx.ExecContext(ctx, `INSERT INTO host_command.documents(workspace,session,package_id,namespace,key,schema_hash,value,command_id,event_id) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9)`, b.Workspace, b.Session, row.PackageID, row.Namespace, row.Key, row.SchemaHash, raw, id, event); err != nil {
			return err
		}
	}
	for _, q := range i.Quantities {
		id, event := provenance(h, q.PackageID, q.Table, q.Key, true)
		if id == "" || event == "" {
			return eventstore.ErrHistory
		}
		if _, err = tx.ExecContext(ctx, `INSERT INTO host_command.quantity(workspace,session,package_id,key,quantity,command_id,event_id) VALUES($1,$2,$3,$4,$5,$6,$7)`, b.Workspace, b.Session, q.PackageID, q.Key, q.Value, id, event); err != nil {
			return err
		}
	}
	if err = r.graphFault(ctx, tx, "recovery-after-data"); err != nil {
		return err
	}
	raw, err := hostJSON(c)
	if err != nil || len(raw) > 1<<20 {
		return eventstore.ErrHistory
	}
	if _, err = tx.ExecContext(ctx, `INSERT INTO host_command.projection_caches(workspace,session,cache) VALUES($1,$2,$3) ON CONFLICT(workspace,session) DO UPDATE SET cache=EXCLUDED.cache`, b.Workspace, b.Session, raw); err != nil {
		return err
	}
	if err = r.graphFault(ctx, tx, "recovery-after-cache"); err != nil {
		return err
	}
	if err = r.graphFault(ctx, tx, "recovery-before-commit"); err != nil {
		return err
	}
	return tx.Commit()
}
func provenance(h data.ReplayHistory, pkg, namespace, key string, quantity bool) (string, string) {
	for n := len(h.Records) - 1; n >= 0; n-- {
		r := h.Records[n]
		found := false
		if quantity {
			for _, v := range r.Quantities {
				if v.PackageID == pkg && v.Table == namespace && v.Key == key {
					found = true
				}
			}
		} else {
			for _, v := range r.Rows {
				if v.PackageID == pkg && v.Namespace == namespace && v.Key == key && !v.Deleted {
					found = true
				}
			}
		}
		if found && len(r.Events) > 0 {
			return r.Header.CommandID, r.Events[0].ID
		}
	}
	return "", ""
}
func (r *HostRepository) WithRecoveryTransactionAbort(point string) (*HostRepository, error) {
	allowed := map[string]bool{"recovery-after-state": true, "recovery-after-data": true, "recovery-after-cache": true, "recovery-before-commit": true}
	if !allowed[point] {
		return nil, data.ErrDenied
	}
	o := r.options
	o.AbortAt = point
	return &HostRepository{options: o}, nil
}
