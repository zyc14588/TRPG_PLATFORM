// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// Package postgres implements the single installation visibility transaction.
// A standard PostgreSQL database/sql driver must be linked by the composition
// root. No package-provided SQL or schema is executed here.
package postgres

import (
	"bytes"
	"context"
	"database/sql"
	"errors"
	"fmt"
	"sort"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/budget"
	aicontext "github.com/zyc14588/TRPG_PLATFORM/internal/ai/context"
	"github.com/zyc14588/TRPG_PLATFORM/internal/eventstore"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/extension"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/model"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/player"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/object"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
	"github.com/zyc14588/TRPG_PLATFORM/internal/task"
)

type Options struct {
	DB      *sql.DB
	Objects object.Store
	Support extension.Support
	// Fault is an operator/test seam. It can only abort a transition, never skip
	// a validation. The transaction is passed to let tests terminate its backend.
	Fault func(context.Context, string, *sql.Tx) error
}
type Repository struct {
	options Options
	owned   bool
}

func New(o Options) (*Repository, error) {
	if o.DB == nil || o.Objects == nil {
		return nil, fmt.Errorf("database and object backend required")
	}
	if o.Support.HostAPIMajor == 0 {
		o.Support = extension.DefaultSupport
	}
	return &Repository{options: o}, nil
}

// OpenInstallationRepository keeps driver/connection handles in the storage
// layer. Its optional callback can abort or delay a fixed transition only.
func OpenInstallationRepository(ctx context.Context, dsn string, objects object.Store, support extension.Support, fault func(context.Context, string) error) (*Repository, error) {
	db, err := sql.Open("pgx", dsn)
	if err != nil {
		return nil, err
	}
	if err = db.PingContext(ctx); err != nil {
		db.Close()
		return nil, err
	}
	r, err := New(Options{DB: db, Objects: objects, Support: support, Fault: func(ctx context.Context, point string, _ *sql.Tx) error {
		if fault != nil {
			return fault(ctx, point)
		}
		return nil
	}})
	if err != nil {
		db.Close()
		return nil, err
	}
	r.owned = true
	return r, nil
}
func (r *Repository) Close() error {
	if r.owned {
		return r.options.DB.Close()
	}
	return nil
}

type InstallationInspection struct{ Artifacts, Grants, Requests, DataTargets int }

func (r *Repository) InspectInstallation(ctx context.Context, workspace string) (InstallationInspection, error) {
	var v InstallationInspection
	if !store.ValidID(workspace) {
		return v, store.ErrDenied
	}
	err := r.options.DB.QueryRowContext(ctx, `SELECT (SELECT count(*) FROM package_install.artifacts WHERE workspace=$1),(SELECT count(*) FROM package_install.grants WHERE workspace=$1),(SELECT count(*) FROM package_install.requests WHERE workspace=$1),(SELECT count(*) FROM package_install.data_targets WHERE workspace=$1)`, workspace).Scan(&v.Artifacts, &v.Grants, &v.Requests, &v.DataTargets)
	return v, err
}
func (r *Repository) fault(ctx context.Context, point string, tx *sql.Tx) error {
	if r.options.Fault != nil {
		return r.options.Fault(ctx, point, tx)
	}
	return nil
}

// Bootstrap is an explicit operator action. Install never runs schema DDL.
// Package data/migration targets stay empty until a future owning component
// records them; any existing affected state makes this initial-install API fail.
func (r *Repository) Bootstrap(ctx context.Context) error {
	tx, err := r.options.DB.BeginTx(ctx, nil)
	if err != nil {
		return err
	}
	defer tx.Rollback()
	for _, statement := range []string{
		`CREATE SCHEMA IF NOT EXISTS package_install`,
		`CREATE TABLE IF NOT EXISTS package_install.workspaces (workspace text PRIMARY KEY)`,
		`CREATE TABLE IF NOT EXISTS package_install.data_targets (workspace text NOT NULL REFERENCES package_install.workspaces, package_id text NOT NULL, state_reference text NOT NULL, PRIMARY KEY (workspace,package_id,state_reference))`,
		`CREATE TABLE IF NOT EXISTS package_install.history_pins (workspace text NOT NULL REFERENCES package_install.workspaces, session text NOT NULL, identity text NOT NULL, package_id text NOT NULL, PRIMARY KEY(workspace,session,identity))`,
		`CREATE TABLE IF NOT EXISTS package_install.artifacts (
		workspace text NOT NULL REFERENCES package_install.workspaces, identity text NOT NULL,
		package_id text NOT NULL, version text NOT NULL, content_hash text NOT NULL, lock_hash text NOT NULL,
		source_archive_hash text NOT NULL, archive_key text NOT NULL,
		manifest bytea NOT NULL, identity_bytes bytea NOT NULL, lock_bytes bytea NOT NULL, rights bytea NOT NULL,
		policy_digest text NOT NULL, retention text NOT NULL, validation_digest text NOT NULL,
		PRIMARY KEY (workspace,identity))`,
		`CREATE INDEX IF NOT EXISTS package_install_package_idx ON package_install.artifacts(workspace,package_id)`,
		`CREATE TABLE IF NOT EXISTS package_install.objects (workspace text NOT NULL, identity text NOT NULL, path text NOT NULL, object_key text NOT NULL, PRIMARY KEY(workspace,identity,path), FOREIGN KEY(workspace,identity) REFERENCES package_install.artifacts)`,
		`CREATE TABLE IF NOT EXISTS package_install.grants (workspace text NOT NULL, principal text NOT NULL, identity text NOT NULL, PRIMARY KEY(workspace,principal,identity), FOREIGN KEY(workspace,identity) REFERENCES package_install.artifacts)`,
		`CREATE TABLE IF NOT EXISTS package_install.requests (workspace text NOT NULL, request_id text NOT NULL, principal text NOT NULL, fingerprint text NOT NULL, root text NOT NULL, PRIMARY KEY(workspace,request_id), FOREIGN KEY(workspace,root) REFERENCES package_install.artifacts(workspace,identity))`,
		`CREATE TABLE IF NOT EXISTS package_install.request_artifacts (workspace text NOT NULL, request_id text NOT NULL, identity text NOT NULL, source_archive_hash text NOT NULL, PRIMARY KEY(workspace,request_id,identity), FOREIGN KEY(workspace,request_id) REFERENCES package_install.requests, FOREIGN KEY(workspace,identity) REFERENCES package_install.artifacts)`,
	} {
		if _, err = tx.ExecContext(ctx, statement); err != nil {
			return err
		}
	}
	return tx.Commit()
}

func (r *Repository) ProvisionWorkspace(ctx context.Context, workspace string) error {
	if !store.ValidID(workspace) {
		return store.ErrDenied
	}
	_, err := r.options.DB.ExecContext(ctx, `INSERT INTO package_install.workspaces(workspace) VALUES($1) ON CONFLICT DO NOTHING`, workspace)
	return err
}

func (r *Repository) Preflight(ctx context.Context, workspace string, ids []string) error {
	if !store.ValidID(workspace) || len(ids) == 0 {
		return store.ErrMigration
	}
	var exists bool
	if err := r.options.DB.QueryRowContext(ctx, `SELECT EXISTS(SELECT 1 FROM package_install.workspaces WHERE workspace=$1)`, workspace).Scan(&exists); err != nil {
		return err
	}
	if !exists {
		return store.ErrDenied
	}
	for _, id := range ids {
		if err := r.options.DB.QueryRowContext(ctx, `SELECT EXISTS(SELECT 1 FROM package_install.data_targets WHERE workspace=$1 AND package_id=$2 UNION ALL SELECT 1 FROM package_install.history_pins WHERE workspace=$1 AND package_id=$2)`, workspace, id).Scan(&exists); err != nil {
			return err
		}
		if exists {
			return store.ErrMigration
		}
	}
	return nil
}

func (r *Repository) Publish(ctx context.Context, p store.Publication) (store.Result, error) {
	var zero store.Result
	if _, err := model.ParseContentHash(p.Fingerprint); err != nil {
		return zero, store.ErrConflict
	}
	if !store.ValidID(p.Workspace) || !store.ValidID(p.Principal) || !store.ValidID(p.RequestID) || len(p.Artifacts) == 0 || len(p.Artifacts) > 128 {
		return zero, store.ErrDenied
	}
	seen := map[string]bool{}
	for _, a := range p.Artifacts {
		if seen[a.Identity] || a.ValidationDigest == "" || a.PolicyDigest == "" || a.Retention == "" || len(a.Objects) == 0 {
			return zero, store.ErrConflict
		}
		seen[a.Identity] = true
		if _, err := store.VerifyArtifact(ctx, r.options.Objects, a, r.options.Support); err != nil {
			return zero, err
		}
	}
	if !seen[p.Root] {
		return zero, store.ErrConflict
	}
	if err := r.fault(ctx, "before-transaction", nil); err != nil {
		return zero, err
	}
	tx, err := r.options.DB.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelReadCommitted})
	if err != nil {
		return zero, err
	}
	defer tx.Rollback()
	if _, err = tx.ExecContext(ctx, `SET LOCAL statement_timeout='5s'`); err != nil {
		return zero, err
	}
	// One workspace row serializes publication/preflight. Other workspaces and
	// readers remain independent. Every installation writer takes this lock.
	var locked string
	if err = tx.QueryRowContext(ctx, `SELECT workspace FROM package_install.workspaces WHERE workspace=$1 FOR UPDATE`, p.Workspace).Scan(&locked); err != nil {
		return zero, err
	}
	var fingerprint, principal string
	err = tx.QueryRowContext(ctx, `SELECT fingerprint,principal FROM package_install.requests WHERE workspace=$1 AND request_id=$2`, p.Workspace, p.RequestID).Scan(&fingerprint, &principal)
	if err == nil {
		if fingerprint != p.Fingerprint || principal != p.Principal {
			return zero, store.ErrConflict
		}
		if err = tx.Rollback(); err != nil {
			return zero, err
		}
		return r.Resolve(ctx, p.Workspace, p.Principal, p.RequestID, p.Fingerprint)
	}
	if !errors.Is(err, sql.ErrNoRows) {
		return zero, err
	}
	artifacts := append([]store.Artifact(nil), p.Artifacts...)
	sort.Slice(artifacts, func(i, j int) bool { return artifacts[i].Identity < artifacts[j].Identity })
	for _, a := range artifacts {
		var affected bool
		if err = tx.QueryRowContext(ctx, `SELECT EXISTS(SELECT 1 FROM package_install.data_targets WHERE workspace=$1 AND package_id=$2 UNION ALL SELECT 1 FROM package_install.history_pins WHERE workspace=$1 AND package_id=$2)`, p.Workspace, a.PackageID).Scan(&affected); err != nil {
			return zero, err
		}
		if affected {
			return zero, store.ErrMigration
		}
		result, e := tx.ExecContext(ctx, `INSERT INTO package_install.artifacts(workspace,identity,package_id,version,content_hash,lock_hash,source_archive_hash,archive_key,manifest,identity_bytes,lock_bytes,rights,policy_digest,retention,validation_digest) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15) ON CONFLICT(workspace,identity) DO NOTHING`, p.Workspace, a.Identity, a.PackageID, a.Version, a.ContentHash, a.LockHash, a.SourceArchiveHash, a.ArchiveKey, a.Manifest, a.IdentityBytes, a.LockBytes, a.Rights, a.PolicyDigest, a.Retention, a.ValidationDigest)
		if e != nil {
			return zero, e
		}
		count, e := result.RowsAffected()
		if e != nil {
			return zero, e
		}
		if count == 0 {
			var identityBytes, rights, manifest, lock []byte
			var archiveKey, retention string
			if e = tx.QueryRowContext(ctx, `SELECT identity_bytes,rights,manifest,lock_bytes,archive_key,retention FROM package_install.artifacts WHERE workspace=$1 AND identity=$2`, p.Workspace, a.Identity).Scan(&identityBytes, &rights, &manifest, &lock, &archiveKey, &retention); e != nil {
				return zero, e
			}
			if !bytes.Equal(identityBytes, a.IdentityBytes) || !bytes.Equal(rights, a.Rights) || !bytes.Equal(manifest, a.Manifest) || !bytes.Equal(lock, a.LockBytes) || archiveKey != a.ArchiveKey || retention != a.Retention {
				return zero, store.ErrConflict
			}
		} else {
			for _, ref := range a.Objects {
				if _, e = tx.ExecContext(ctx, `INSERT INTO package_install.objects(workspace,identity,path,object_key) VALUES($1,$2,$3,$4)`, p.Workspace, a.Identity, ref.Path, ref.Key); e != nil {
					return zero, e
				}
			}
		}
		if _, e = tx.ExecContext(ctx, `INSERT INTO package_install.grants(workspace,principal,identity) VALUES($1,$2,$3) ON CONFLICT DO NOTHING`, p.Workspace, p.Principal, a.Identity); e != nil {
			return zero, e
		}
	}
	if _, err = tx.ExecContext(ctx, `INSERT INTO package_install.requests(workspace,request_id,principal,fingerprint,root) VALUES($1,$2,$3,$4,$5)`, p.Workspace, p.RequestID, p.Principal, p.Fingerprint, p.Root); err != nil {
		return zero, err
	}
	for _, a := range artifacts {
		if _, err = tx.ExecContext(ctx, `INSERT INTO package_install.request_artifacts(workspace,request_id,identity,source_archive_hash) VALUES($1,$2,$3,$4)`, p.Workspace, p.RequestID, a.Identity, a.SourceArchiveHash); err != nil {
			return zero, err
		}
	}
	if err = r.fault(ctx, "after-metadata-before-commit", tx); err != nil {
		return zero, err
	}
	if err = ctx.Err(); err != nil {
		return zero, err
	}
	if err = r.fault(ctx, "before-commit", tx); err != nil {
		return zero, err
	}
	// Storage interruption during metadata staging must roll back the still
	// private transaction. Recheck complete durable references at publication.
	for _, a := range artifacts {
		for _, ref := range a.Objects {
			if err = r.options.Objects.Verify(ctx, ref.Key); err != nil {
				return zero, err
			}
		}
	}
	if err = tx.Commit(); err != nil {
		return zero, errors.Join(store.ErrUnknownCommit, err)
	}
	if err = r.fault(ctx, "after-commit-before-ack", nil); err != nil {
		return zero, errors.Join(store.ErrUnknownCommit, err)
	}
	// Always read the committed result. A lost connection here is an unknown
	// acknowledgement, not permission to undo or garbage-collect shared bytes.
	readCtx, cancel := context.WithTimeout(context.WithoutCancel(ctx), 5*time.Second)
	defer cancel()
	result, err := r.Resolve(readCtx, p.Workspace, p.Principal, p.RequestID, p.Fingerprint)
	if err != nil {
		return zero, errors.Join(store.ErrUnknownCommit, err)
	}
	return result, nil
}

func (r *Repository) Resolve(ctx context.Context, workspace, principal, requestID, fingerprint string) (store.Result, error) {
	var result store.Result
	var owner string
	err := r.options.DB.QueryRowContext(ctx, `SELECT principal,fingerprint,root FROM package_install.requests WHERE workspace=$1 AND request_id=$2`, workspace, requestID).Scan(&owner, &result.Fingerprint, &result.Root)
	if errors.Is(err, sql.ErrNoRows) {
		return result, store.ErrNotFound
	}
	if err != nil {
		return result, err
	}
	if owner != principal || result.Fingerprint != fingerprint {
		return store.Result{}, store.ErrConflict
	}
	result.Workspace = workspace
	result.RequestID = requestID
	rows, err := r.options.DB.QueryContext(ctx, `SELECT identity FROM package_install.request_artifacts WHERE workspace=$1 AND request_id=$2 ORDER BY identity`, workspace, requestID)
	if err != nil {
		return store.Result{}, err
	}
	for rows.Next() {
		var id string
		if err = rows.Scan(&id); err != nil {
			rows.Close()
			return store.Result{}, err
		}
		result.Artifacts = append(result.Artifacts, id)
	}
	err = rows.Err()
	rows.Close()
	if err != nil {
		return store.Result{}, err
	}
	if len(result.Artifacts) == 0 {
		return store.Result{}, store.ErrConflict
	}
	rootFound := false
	for _, id := range result.Artifacts {
		rootFound = rootFound || id == result.Root
		a, e := r.Lookup(ctx, workspace, principal, id)
		if e != nil {
			return store.Result{}, e
		}
		if _, e = store.VerifyArtifact(ctx, r.options.Objects, a, r.options.Support); e != nil {
			return store.Result{}, e
		}
	}
	if !rootFound {
		return store.Result{}, object.ErrIntegrity
	}
	return result, nil
}

func (r *Repository) Lookup(ctx context.Context, workspace, principal, identity string) (store.Artifact, error) {
	var a store.Artifact
	err := r.options.DB.QueryRowContext(ctx, `SELECT a.identity,a.package_id,a.version,a.content_hash,a.lock_hash,a.source_archive_hash,a.archive_key,a.manifest,a.identity_bytes,a.lock_bytes,a.rights,a.policy_digest,a.retention,a.validation_digest FROM package_install.artifacts a JOIN package_install.grants g ON a.workspace=g.workspace AND a.identity=g.identity WHERE a.workspace=$1 AND g.principal=$2 AND a.identity=$3`, workspace, principal, identity).Scan(&a.Identity, &a.PackageID, &a.Version, &a.ContentHash, &a.LockHash, &a.SourceArchiveHash, &a.ArchiveKey, &a.Manifest, &a.IdentityBytes, &a.LockBytes, &a.Rights, &a.PolicyDigest, &a.Retention, &a.ValidationDigest)
	if errors.Is(err, sql.ErrNoRows) {
		return a, store.ErrNotFound
	}
	if err != nil {
		return a, err
	}
	rows, err := r.options.DB.QueryContext(ctx, `SELECT path,object_key FROM package_install.objects WHERE workspace=$1 AND identity=$2 ORDER BY path`, workspace, identity)
	if err != nil {
		return a, err
	}
	defer rows.Close()
	for rows.Next() {
		var ref store.ObjectRef
		if err = rows.Scan(&ref.Path, &ref.Key); err != nil {
			return a, err
		}
		a.Objects = append(a.Objects, ref)
	}
	if err = rows.Err(); err != nil {
		return a, err
	}
	if len(a.Objects) == 0 {
		return a, store.ErrConflict
	}
	return a, nil
}

// ReadM2ProviderLease and ReadM2ProviderReservation form one private deployment
// read guard. They borrow ONLY the existing cookie-bound core transaction; no
// database handle, SQL endpoint, token issuer or domain write is introduced.
// Lease is checked before Context's host-session SHARE lock; reservation is
// checked after it. This preserves job -> host session -> control -> budget,
// with the original five counter locks in their original order.
func ReadM2ProviderLease(ctx context.Context, tx core.Transaction, worker task.Worker, job task.Job) error {
	c, e := budgetCore(tx)
	if e != nil || ctx == nil || ctx.Err() != nil {
		return auth.ErrDenied
	}
	now, e := c.Now(ctx)
	if e != nil {
		return auth.SafeError(e)
	}
	_, e = readM2ProviderLease(ctx, c.tx, worker, job, now)
	return e
}

func readM2ProviderLease(ctx context.Context, tx *sql.Tx, worker task.Worker, job task.Job, now time.Time) (task.JobData, error) {
	v, e := job.StorageValue()
	if e != nil || v.Status != task.Running || v.ResultWorker != "" || v.Attempt < 1 || v.Token.Digest() == "" {
		return task.JobData{}, auth.ErrDenied
	}
	r, e := leaseTask(ctx, tx, worker, job, now, false)
	if e != nil || r.job.Status != task.Running || r.job.Attempt != v.Attempt || r.job.DeliveryAttempt != v.DeliveryAttempt || !r.job.LeaseUntil.Equal(v.LeaseUntil) || r.job.ResultWorker != "" {
		return task.JobData{}, auth.ErrDenied
	}
	current, e := task.NewJob(r.job)
	if e != nil {
		return task.JobData{}, auth.ErrDenied
	}
	a, e := task.EncodeForStorage(current)
	defer clear(a)
	if e != nil {
		return task.JobData{}, auth.ErrDenied
	}
	b, e := task.EncodeForStorage(job)
	defer clear(b)
	if e != nil || !bytes.Equal(a, b) || taskLive(ctx, tx, r.job) != nil || worker.Check(ctx, v.Binding.Workspace) != nil {
		return task.JobData{}, auth.ErrDenied
	}
	return r.job, nil
}

// M2ProviderSourcePrincipal is a platform-private, one-hop association lookup.
// The original Worker authenticates this initial read, before a browser Caller
// is known. It returns no credential or permission. The same proof is repeated
// in the cookie-bound begin transaction below; lookup commit failure rejects.
func M2ProviderSourcePrincipal(ctx context.Context, repo *PlatformAuthRepository, worker task.Worker, job task.Job) (string, error) {
	s, e := NewPlatformTaskStorage(PlatformTaskOptions{Repository: repo, Lease: 15 * time.Second, Lifetime: 10 * time.Minute})
	if e != nil {
		return "", auth.ErrDenied
	}
	tx, now, e := s.begin(ctx, worker)
	if e != nil {
		return "", auth.ErrDenied
	}
	defer tx.Rollback()
	v, e := readM2ProviderLease(ctx, tx, worker, job, now)
	if e != nil {
		return "", e
	}
	id, e := readM2ProviderSourcePrincipal(ctx, tx, v)
	if e != nil || worker.Check(ctx, v.Binding.Workspace) != nil || ctx.Err() != nil {
		return "", auth.ErrDenied
	}
	if e = tx.Commit(); e != nil {
		return "", auth.ErrOutcomeUnknown
	}
	return id, nil
}

func ReadM2ProviderSourcePrincipal(ctx context.Context, tx core.Transaction, worker task.Worker, job task.Job) (string, error) {
	c, e := budgetCore(tx)
	if e != nil || ctx == nil || ctx.Err() != nil {
		return "", auth.ErrDenied
	}
	now, e := c.Now(ctx)
	if e != nil {
		return "", auth.SafeError(e)
	}
	v, e := readM2ProviderLease(ctx, c.tx, worker, job, now)
	if e != nil {
		return "", e
	}
	return readM2ProviderSourcePrincipal(ctx, c.tx, v)
}

func readM2ProviderSourcePrincipal(ctx context.Context, tx *sql.Tx, child task.JobData) (string, error) {
	if child.OriginPrincipal != "task-system" {
		return child.OriginPrincipal, nil
	}
	var source, saved []byte
	e := tx.QueryRowContext(ctx, `SELECT r.evidence,q.receipt FROM host_command.replay_effects r JOIN host_command.requests q ON q.workspace=r.workspace AND q.session=r.session AND q.command_id=r.command_id WHERE r.workspace=$1 AND r.session=$2 AND r.command_id=$3 AND q.principal='task-system'`, child.Binding.Workspace, child.Binding.Session, child.SourceCommand).Scan(&source, &saved)
	defer clear(source)
	defer clear(saved)
	if e != nil || checkpoint.Hash(source) != child.SourceHash {
		return "", auth.ErrDenied
	}
	r, e := eventstore.Decode(source)
	if e != nil || r.Header.Binding != child.Binding || r.Header.CommandID != child.SourceCommand || r.Header.Principal != "task-system" || r.Version != child.OriginVersion || r.Inputs.Callback != "resume_continuation" || r.Inputs.Envelope == nil || r.Inputs.Envelope.Seat != "task-system" || r.Inputs.Envelope.Type != "resume-continuation" {
		return "", auth.ErrDenied
	}
	var receipt data.Receipt
	if checkpoint.StrictDecode(saved, &receipt, 2<<20) != nil || !sameTaskReceipt(receipt, data.Receipt{Header: r.Header, Version: r.Version, Cursor: r.Cursor, Inputs: r.Inputs, Result: r.Result, Events: r.Events}) {
		return "", auth.ErrDenied
	}
	input := r.Inputs.Command
	if input.Kind != "table" || len(input.Table) != 4 || input.Table["type"].Kind != "string" || input.Table["type"].String != "resume-continuation" || input.Table["seat_id"].Kind != "string" || input.Table["seat_id"].String != "task-system" || input.Table["payload"].Kind != "table" || len(input.Table["payload"].Table) != 5 {
		return "", auth.ErrDenied
	}
	payload := input.Table["payload"].Table
	id, continuation := payload["task_id"], payload["continuation_id"]
	if id.Kind != "string" || continuation.Kind != "string" || !store.ValidID(id.String) || !store.ValidID(continuation.String) || id.String == child.TaskID {
		return "", auth.ErrDenied
	}
	// Read exactly one parent row. No history scan, recursion, token adoption or
	// domain write is allowed. Native terminal results and receipts are stable.
	parent, e := scanTask(tx.QueryRowContext(ctx, `SELECT `+taskColumns+` FROM platform_task.jobs WHERE workspace=$1 AND session=$2 AND task_id=$3`, child.Binding.Workspace, child.Binding.Session, id.String))
	if e != nil {
		return "", auth.ErrDenied
	}
	p := parent.job
	if p.Scope != child.Scope || p.Binding != child.Binding || p.PackageID != child.PackageID || p.ConfigurationID != child.ConfigurationID || p.ConfigurationHash != child.ConfigurationHash || p.OriginPrincipal == "task-system" || p.OriginVersion >= child.OriginVersion || p.SourceCommand == child.SourceCommand || p.ResultWorker == "" || (p.Status != task.Done && p.Status != task.Delivering) {
		return "", auth.ErrDenied
	}
	var original []byte
	e = tx.QueryRowContext(ctx, `SELECT evidence FROM host_command.replay_effects WHERE workspace=$1 AND session=$2 AND command_id=$3`, p.Binding.Workspace, p.Binding.Session, p.SourceCommand).Scan(&original)
	defer clear(original)
	if e != nil {
		return "", auth.ErrDenied
	}
	projected, e := projectTask(original, p)
	if e != nil || projected.SourceHash != p.SourceHash || projected.OriginVersion != p.OriginVersion || projected.OriginPrincipal != p.OriginPrincipal || projected.Payload.Digest() != p.Payload.Digest() || len(projected.Continuations) != len(p.Continuations) {
		return "", auth.ErrDenied
	}
	index := -1
	for i, c := range projected.Continuations {
		if c.ID != p.Continuations[i].ID || c.Value.Digest() != p.Continuations[i].Value.Digest() {
			return "", auth.ErrDenied
		}
		if c.ID == continuation.String {
			if index != -1 {
				return "", auth.ErrDenied
			}
			index = i
		}
	}
	if index < 0 || (p.Status == task.Done && p.Next <= index) || (p.Status == task.Delivering && p.Next != index) {
		return "", auth.ErrDenied
	}
	// This comparison copy is never returned or sent to any state operation.
	// Keep original NewJob/Envelope/taskReceipt validation intact, including
	// all result, time, random, tool-input and continuation identity checks.
	historical := p
	historical.Next = index
	j, e := task.NewJob(historical)
	if e != nil {
		return "", auth.ErrDenied
	}
	envelope, e := j.Envelope()
	if e != nil || envelope.CommandID != child.SourceCommand || envelope.ExpectedStateVersion != r.Header.ExpectedVersion {
		return "", auth.ErrDenied
	}
	committed, e := taskReceipt(ctx, tx, historical)
	if e != nil || !sameTaskReceipt(committed, receipt) || ctx.Err() != nil {
		return "", auth.ErrDenied
	}
	return p.OriginPrincipal, nil
}

func ReadM2ProviderReservation(ctx context.Context, tx core.Transaction, observed budget.Reservation, subject aicontext.SubjectData, bound budget.Units, promptBytes uint64, caps budget.Caps) error {
	c, e := budgetCore(tx)
	if e != nil || ctx == nil || ctx.Err() != nil {
		return auth.ErrDenied
	}
	r := observed.StorageValue()
	if r.Task.Subject != subject || r.Status != "reserved" || r.Task.State != "reserved" && r.Task.State != "open" || r.Amount != bound || r.Spent != (budget.Units{}) || r.PromptBytes != promptBytes || promptBytes > bound.ContextBytes || !budget.Fits(bound, subject.Budget) || !checkpoint.IsDigest("sha256:"+r.RequestHash) || !budget.ValidCaps(caps) {
		return auth.ErrDenied
	}
	pt := playerSQL(c.tx)
	state, e := pt.State(ctx, subject.Binding)
	if e != nil {
		return auth.SafeError(e)
	}
	control := state.StorageValue()
	leases, e := pt.Leases(ctx, subject.Binding)
	if e != nil {
		return auth.SafeError(e)
	}
	now, e := pt.Now(ctx)
	if e != nil || control.Scope != subject.Scope || control.ConfigurationHash != subject.ConfigurationHash || control.PreparationRevision != subject.PreparationRevision || control.Paused || control.Quiescing || player.Disconnected(control, leases, now) {
		return auth.ErrDenied
	}
	// This path never calls controlledTaskReady: its disconnect branch writes
	// a durable pause. Here the original control service owns that transition.
	d := &platformBudgetTxData{core: c}
	st := &platformBudgetTransaction{data: &d}
	current, e := st.Task(ctx, subject, r.Task.ID)
	if e != nil || current.StorageValue().State != "dispatched" || current.StorageValue().Advice != r.Task.Advice {
		return auth.ErrDenied
	}
	reservation, e := st.Reservation(ctx, current)
	if e != nil {
		return auth.SafeError(e)
	}
	fresh := reservation.StorageValue()
	if fresh.Status != "dispatched" || fresh.Task != r.Task || fresh.RequestHash != r.RequestHash || fresh.Amount != bound || fresh.PromptBytes != promptBytes || fresh.Spent != (budget.Units{}) {
		return auth.ErrDenied
	}
	// create=false has only SELECT FOR UPDATE. No INSERT, saveCells, reserve,
	// dispatch, settle, status promotion or hold release runs in this guard.
	cs, e := st.lockCells(ctx, r.Task, caps, false)
	if e != nil {
		return auth.SafeError(e)
	}
	expected := cells(r.Task, caps)
	for i, cell := range cs {
		if cell.cap != expected[i].cap || !budget.Fits(bound, cell.held) || !budget.CanReserve(cell.used, cell.held, budget.Units{}, cell.cap) {
			return auth.ErrDenied
		}
	}
	var blocked bool
	e = c.tx.QueryRowContext(ctx, `SELECT EXISTS(SELECT 1 FROM platform_budget.pauses WHERE workspace_id=$1 AND session_id=$2 AND seat_id=$3) OR EXISTS(SELECT 1 FROM platform_budget.reservations WHERE workspace_id=$1 AND session_id=$2 AND seat_id=$3 AND task_id<>$4 AND status IN('dispatched','uncertain'))`, subject.Scope.WorkspaceID, subject.Binding.Session, subject.SeatID, r.Task.ID).Scan(&blocked)
	if e != nil || blocked || ctx.Err() != nil {
		return auth.ErrDenied
	}
	return nil
}
