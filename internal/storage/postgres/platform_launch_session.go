// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package postgres

import (
	"context"
	"encoding/json"
	"fmt"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/migration"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
	"io"
	"math"
	"sort"
)

// A SessionFactory may provision through the existing authenticated lobby
// transaction. It cannot begin/commit SQL, escape the transaction, or accept
// caller-supplied SQL. The legacy factory and storage implementation stay intact.
type platformLaunchSessionRepository struct{ data **platformLaunchSessionData }
type platformLaunchSessionData struct {
	host    *HostRepository
	core    *platformCoreTransaction
	scope   core.Scope
	binding data.Binding
}

func (r *platformLaunchSessionRepository) state() *platformLaunchSessionData {
	if r == nil || r.data == nil {
		return nil
	}
	return *r.data
}
func (*platformLaunchSessionRepository) Format(f fmt.State, _ rune) {
	_, _ = io.WriteString(f, "<launch session repository>")
}
func (*platformLaunchSessionRepository) MarshalJSON() ([]byte, error) { return nil, auth.ErrDenied }
func (r *platformLaunchSessionRepository) Begin(ctx context.Context, h data.Header) (data.Transaction, error) {
	d := r.state()
	if d == nil || d.core != nil {
		return nil, data.ErrDenied
	}
	tx, e := d.host.Begin(ctx, h)
	if e != nil {
		return nil, launchRuntimeError(e)
	}
	v := &platformLaunchHostTxData{tx: tx}
	return &platformLaunchHostTransaction{data: &v}, nil
}
func (r *platformLaunchSessionRepository) ReadGraphSession(ctx context.Context, g *store.Graph, b data.Binding) (data.Snapshot, error) {
	v, e := r.state().host.ReadGraphSession(ctx, g, b)
	return v, launchRuntimeError(e)
}
func (r *platformLaunchSessionRepository) ProvisionGraph(ctx context.Context, g *store.Graph, b data.Binding, version uint64, schema string, state checkpoint.Value) error {
	d := r.state()
	if d == nil || d.core == nil || d.core.tx == nil || ctx == nil || ctx.Err() != nil || g == nil || g.Root() == nil || !validBinding(b) || b != d.binding || b.Workspace != d.scope.WorkspaceID || version == 0 || version >= math.MaxInt64 || !checkpoint.IsDigest(schema) || checkpoint.Validate(state) != nil {
		return data.ErrDenied
	}
	m := g.Membership()
	lock, e := g.Root().ExactLock().Digest()
	if e != nil || m.Workspace != b.Workspace || !m.Read || string(lock) != b.GraphHash {
		return data.ErrDenied
	}
	artifacts := g.Artifacts()
	if len(artifacts) < 1 || len(artifacts) > 128 {
		return data.ErrDenied
	}
	sort.Slice(artifacts, func(i, j int) bool { return artifacts[i].PackageID < artifacts[j].PackageID })
	graph, e := json.Marshal(artifacts)
	if e != nil {
		return data.ErrDenied
	}
	tx := d.core.tx
	var workspace string
	// This is the same workspace lock and exact artifact/grant recheck used by
	// the accepted standalone provisioner. It lasts through the lobby commit.
	if e = tx.QueryRowContext(ctx, `SELECT workspace FROM package_install.workspaces WHERE workspace=$1 FOR UPDATE`, b.Workspace).Scan(&workspace); e != nil {
		return authStorageError(e)
	}
	for _, expected := range artifacts {
		var actual store.GraphArtifact
		e = tx.QueryRowContext(ctx, `SELECT a.identity,a.package_id,a.content_hash,a.policy_digest,a.validation_digest FROM package_install.artifacts a JOIN package_install.grants g ON a.workspace=g.workspace AND a.identity=g.identity WHERE a.workspace=$1 AND g.principal=$2 AND a.identity=$3`, b.Workspace, m.Principal, expected.Identity).Scan(&actual.Identity, &actual.PackageID, &actual.ContentHash, &actual.PolicyDigest, &actual.ValidationDigest)
		if e != nil {
			return authStorageError(e)
		}
		if actual != expected {
			return data.ErrDenied
		}
	}
	raw, e := hostJSON(state)
	if e != nil {
		return data.ErrDenied
	}
	created, e := tx.ExecContext(ctx, `INSERT INTO host_command.sessions(workspace,session,graph_hash,version,state,schema_hash) VALUES($1,$2,$3,$4,$5::jsonb,$6) ON CONFLICT DO NOTHING`, b.Workspace, b.Session, b.GraphHash, int64(version), string(raw), schema)
	if e != nil {
		return authStorageError(e)
	}
	n, e := created.RowsAffected()
	if e != nil || n != 1 {
		return data.ErrConflict
	}
	if e = d.core.inject(ctx, "launch-after-session"); e != nil {
		return auth.SafeError(e)
	}
	if _, e = tx.ExecContext(ctx, `INSERT INTO host_command.installed_graphs(workspace,session,graph_bytes) VALUES($1,$2,$3)`, b.Workspace, b.Session, graph); e != nil {
		return authStorageError(e)
	}
	exact, e := migration.ExactLock(g)
	if e != nil {
		return data.ErrDenied
	}
	lockBytes, e := hostJSON(exact)
	if e != nil {
		return data.ErrDenied
	}
	if _, e = tx.ExecContext(ctx, `INSERT INTO host_command.session_locks(workspace,session,origin,active) VALUES($1,$2,$3,$3)`, b.Workspace, b.Session, lockBytes); e != nil {
		return authStorageError(e)
	}
	creation := data.Creation{Binding: b, Version: version, SchemaHash: schema, Seed: state, SeedHash: checkpoint.Hash(raw), ArtifactsHash: checkpoint.Hash(graph)}
	evidence, e := hostJSON(creation)
	if e != nil {
		return data.ErrDenied
	}
	if _, e = tx.ExecContext(ctx, `INSERT INTO host_command.creation(workspace,session,evidence) VALUES($1,$2,$3)`, b.Workspace, b.Session, evidence); e != nil {
		return authStorageError(e)
	}
	for _, a := range artifacts {
		if _, e = tx.ExecContext(ctx, `INSERT INTO package_install.data_targets(workspace,package_id,state_reference) VALUES($1,$2,$3)`, b.Workspace, a.PackageID, "host-session:"+b.Session); e != nil {
			return authStorageError(e)
		}
	}
	return auth.SafeError(d.core.inject(ctx, "launch-after-data-targets"))
}

type platformLaunchHostTransaction struct{ data **platformLaunchHostTxData }
type platformLaunchHostTxData struct{ tx data.Transaction }

func (t *platformLaunchHostTransaction) state() *platformLaunchHostTxData {
	if t == nil || t.data == nil {
		return nil
	}
	return *t.data
}
func (platformLaunchHostTransaction) Format(f fmt.State, _ rune) {
	_, _ = io.WriteString(f, "<launch host transaction>")
}
func (platformLaunchHostTransaction) MarshalJSON() ([]byte, error) { return nil, auth.ErrDenied }
func (t *platformLaunchHostTransaction) Snapshot() data.Snapshot   { return t.state().tx.Snapshot() }
func (t *platformLaunchHostTransaction) Commit(ctx context.Context, c data.Commit) (data.Receipt, error) {
	v, e := t.state().tx.Commit(ctx, c)
	return v, launchRuntimeError(e)
}
func (t *platformLaunchHostTransaction) Rollback() error {
	return launchRuntimeError(t.state().tx.Rollback())
}
