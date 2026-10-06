// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package postgres

import (
	"context"
	"database/sql"
	"encoding/json"
	"math"
	"sort"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
)

func (r *HostRepository) installedTransaction(ctx context.Context, g *store.Graph, b data.Binding) (*sql.Tx, []byte, error) {
	if g == nil || g.Root() == nil || !validBinding(b) {
		return nil, nil, data.ErrDenied
	}
	m := g.Membership()
	lock, err := g.Root().ExactLock().Digest()
	if err != nil || m.Workspace != b.Workspace || !m.Read || string(lock) != b.GraphHash {
		return nil, nil, data.ErrDenied
	}
	a := g.Artifacts()
	if len(a) == 0 || len(a) > 128 {
		return nil, nil, data.ErrDenied
	}
	sort.Slice(a, func(i, j int) bool { return a[i].PackageID < a[j].PackageID })
	raw, err := json.Marshal(a)
	if err != nil {
		return nil, nil, err
	}
	tx, err := r.options.DB.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelReadCommitted})
	if err != nil {
		return nil, nil, err
	}
	fail := func(e error) (*sql.Tx, []byte, error) { tx.Rollback(); return nil, nil, e }
	if _, err = tx.ExecContext(ctx, `SET LOCAL statement_timeout='5s'`); err != nil {
		return fail(err)
	}
	var workspace string
	// The installer takes this exact row lock before rechecking data_targets.
	if err = tx.QueryRowContext(ctx, `SELECT workspace FROM package_install.workspaces WHERE workspace=$1 FOR UPDATE`, b.Workspace).Scan(&workspace); err != nil {
		return fail(err)
	}
	if err = r.graphFault(ctx, tx, "graph-workspace-locked"); err != nil {
		return fail(err)
	}
	for _, expected := range a {
		var actual store.GraphArtifact
		err = tx.QueryRowContext(ctx, `SELECT a.identity,a.package_id,a.content_hash,a.policy_digest,a.validation_digest FROM package_install.artifacts a JOIN package_install.grants g ON a.workspace=g.workspace AND a.identity=g.identity WHERE a.workspace=$1 AND g.principal=$2 AND a.identity=$3`, b.Workspace, m.Principal, expected.Identity).Scan(&actual.Identity, &actual.PackageID, &actual.ContentHash, &actual.PolicyDigest, &actual.ValidationDigest)
		if err != nil {
			return fail(err)
		}
		if actual != expected {
			return fail(data.ErrDenied)
		}
	}
	return tx, raw, nil
}

// ProvisionGraph is called only after the installed graph, schemas, actual VM
// and Host service have been configured successfully. No partial registration
// survives an error; concurrent installation uses the same workspace lock.
func (r *HostRepository) ProvisionGraph(ctx context.Context, g *store.Graph, b data.Binding, version uint64, schema string, state checkpoint.Value) error {
	if version == 0 || version >= math.MaxInt64 || !checkpoint.IsDigest(schema) || checkpoint.Validate(state) != nil {
		return data.ErrDenied
	}
	tx, graph, err := r.installedTransaction(ctx, g, b)
	if err != nil {
		return err
	}
	defer tx.Rollback()
	raw, err := hostJSON(state)
	if err != nil {
		return err
	}
	result, err := tx.ExecContext(ctx, `INSERT INTO host_command.sessions(workspace,session,graph_hash,version,state,schema_hash) VALUES($1,$2,$3,$4,$5::jsonb,$6) ON CONFLICT DO NOTHING`, b.Workspace, b.Session, b.GraphHash, int64(version), string(raw), schema)
	if err != nil {
		return err
	}
	n, err := result.RowsAffected()
	if err != nil {
		return err
	}
	if n != 1 {
		return data.ErrConflict
	}
	if err = r.graphFault(ctx, tx, "graph-after-session"); err != nil {
		return err
	}
	if _, err = tx.ExecContext(ctx, `INSERT INTO host_command.installed_graphs(workspace,session,graph_bytes) VALUES($1,$2,$3)`, b.Workspace, b.Session, graph); err != nil {
		return err
	}
	creation := data.Creation{Binding: b, Version: version, SchemaHash: schema, Seed: state, SeedHash: checkpoint.Hash(raw), ArtifactsHash: checkpoint.Hash(graph)}
	evidence, err := hostJSON(creation)
	if err != nil {
		return err
	}
	if _, err = tx.ExecContext(ctx, `INSERT INTO host_command.creation(workspace,session,evidence) VALUES($1,$2,$3)`, b.Workspace, b.Session, evidence); err != nil {
		return err
	}
	for _, a := range g.Artifacts() {
		if _, err = tx.ExecContext(ctx, `INSERT INTO package_install.data_targets(workspace,package_id,state_reference) VALUES($1,$2,$3)`, b.Workspace, a.PackageID, "host-session:"+b.Session); err != nil {
			return err
		}
		if err = r.graphFault(ctx, tx, "graph-after-data-target"); err != nil {
			return err
		}
	}
	if err = r.graphFault(ctx, tx, "graph-before-commit"); err != nil {
		return err
	}
	return tx.Commit()
}

func (r *HostRepository) graphFault(ctx context.Context, tx *sql.Tx, point string) error {
	if r.options.AbortAt == point {
		_, err := tx.ExecContext(ctx, `SELECT 1/0`)
		return err
	}
	return r.fault(ctx, point)
}

// This fixed operator-only negative seam cannot supply SQL or skip a check.
func (r *HostRepository) WithGraphTransactionAbort(point string) (*HostRepository, error) {
	allowed := map[string]bool{"graph-workspace-locked": true, "graph-after-session": true, "graph-after-data-target": true, "graph-before-commit": true}
	if !allowed[point] {
		return nil, data.ErrDenied
	}
	o := r.options
	o.AbortAt = point
	return &HostRepository{options: o}, nil
}

// ReadGraphSession resumes only the exact installed artifact/evidence set used
// at creation. This is authoritative state, not a serialized Lua VM.
func (r *HostRepository) ReadGraphSession(ctx context.Context, g *store.Graph, b data.Binding) (data.Snapshot, error) {
	var s data.Snapshot
	s.Binding = b
	tx, expected, err := r.installedTransaction(ctx, g, b)
	if err != nil {
		return s, err
	}
	defer tx.Rollback()
	var graph string
	var raw, actual []byte
	err = tx.QueryRowContext(ctx, `SELECT s.graph_hash,s.version,s.state,s.schema_hash,g.graph_bytes FROM host_command.sessions s JOIN host_command.installed_graphs g ON s.workspace=g.workspace AND s.session=g.session WHERE s.workspace=$1 AND s.session=$2 FOR UPDATE OF s`, b.Workspace, b.Session).Scan(&graph, &s.Version, &raw, &s.SchemaHash, &actual)
	if err != nil {
		return s, err
	}
	if graph != b.GraphHash || string(expected) != string(actual) || checkpoint.StrictDecode(raw, &s.State, checkpoint.MaxBytes*2) != nil {
		return s, data.ErrDenied
	}
	for _, a := range g.Artifacts() {
		var exists bool
		if err = tx.QueryRowContext(ctx, `SELECT EXISTS(SELECT 1 FROM package_install.data_targets WHERE workspace=$1 AND package_id=$2 AND state_reference=$3)`, b.Workspace, a.PackageID, "host-session:"+b.Session).Scan(&exists); err != nil {
			return s, err
		}
		if !exists {
			return s, data.ErrDenied
		}
	}
	if err = tx.Commit(); err != nil {
		return s, err
	}
	return s, nil
}

type GraphSessionInspection struct{ Sessions, Graphs, DataTargets int }

func (r *HostRepository) InspectGraphSession(ctx context.Context, workspace, sid string) (GraphSessionInspection, error) {
	var out GraphSessionInspection
	if !store.ValidID(workspace) || !store.ValidID(sid) {
		return out, data.ErrDenied
	}
	err := r.options.DB.QueryRowContext(ctx, `SELECT (SELECT count(*) FROM host_command.sessions WHERE workspace=$1 AND session=$2),(SELECT count(*) FROM host_command.installed_graphs WHERE workspace=$1 AND session=$2),(SELECT count(*) FROM package_install.data_targets WHERE workspace=$1 AND state_reference=$3)`, workspace, sid, "host-session:"+sid).Scan(&out.Sessions, &out.Graphs, &out.DataTargets)
	return out, err
}
