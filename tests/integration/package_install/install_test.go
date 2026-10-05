//go:build integration && linux

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package package_install_test

import (
	"bytes"
	"context"
	"crypto/rand"
	"database/sql"
	"encoding/hex"
	"errors"
	"fmt"
	_ "github.com/jackc/pgx/v5/stdlib"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/archive"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/extension"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/install"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/testdata/install"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/object"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/postgres"
	"net/url"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"sync"
	"testing"
	"time"
)

var injected = errors.New("fixture injected failure")

const alice = store.Credential("fixture-alice-private-credential")
const bob = store.Credential("fixture-bob-private-credential")

type environment struct {
	db                 *sql.DB
	repository         *postgres.Repository
	objects            *object.Directory
	access             *store.Access
	root, staging, dsn string
	fault              func(context.Context, string, *sql.Tx) error
}

func setup(t *testing.T) *environment {
	t.Helper()
	dsn := os.Getenv("B003_POSTGRES_DSN")
	if dsn == "" {
		t.Fatal("B003_POSTGRES_DSN is required: no real-service gate may silently skip")
	}
	u, err := url.Parse(dsn)
	if err != nil || u.Scheme != "postgres" || (u.Hostname() != "127.0.0.1" && u.Hostname() != "localhost") || u.Path != "/b003" {
		t.Fatal("use the dedicated local b003 fixture PostgreSQL database")
	}
	admin, err := sql.Open("pgx", dsn)
	if err != nil {
		t.Fatal(err)
	}
	defer admin.Close()
	ctx, cancel := context.WithTimeout(context.Background(), 15*time.Second)
	defer cancel()
	if err = admin.PingContext(ctx); err != nil {
		t.Fatal(err)
	}
	var nonce [12]byte
	if _, err = rand.Read(nonce[:]); err != nil {
		t.Fatal(err)
	}
	name := "b003_" + hex.EncodeToString(nonce[:])
	if _, err = admin.ExecContext(ctx, `CREATE DATABASE `+name); err != nil {
		t.Fatal(err)
	}
	u.Path = "/" + name
	db, err := sql.Open("pgx", u.String())
	if err != nil {
		t.Fatal(err)
	}
	db.SetMaxOpenConns(12)
	t.Cleanup(func() {
		_ = db.Close()
		cleanup, err := sql.Open("pgx", dsn)
		if err != nil {
			t.Error(err)
			return
		}
		defer cleanup.Close()
		if _, err = cleanup.ExecContext(context.Background(), `DROP DATABASE `+name+` WITH (FORCE)`); err != nil {
			t.Error(err)
		}
	})
	root, staging := t.TempDir(), t.TempDir()
	for _, path := range []string{root, staging} {
		if err = os.Chmod(path, 0700); err != nil {
			t.Fatal(err)
		}
	}
	objects, err := object.Open(root)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = objects.Close() })
	e := &environment{db: db, objects: objects, root: root, staging: staging, dsn: u.String()}
	e.repository, err = postgres.New(postgres.Options{DB: db, Objects: objects, Support: extension.DefaultSupport, Fault: func(ctx context.Context, s string, tx *sql.Tx) error {
		if e.fault != nil {
			return e.fault(ctx, s, tx)
		}
		return nil
	}})
	if err != nil {
		t.Fatal(err)
	}
	if err = e.repository.Bootstrap(ctx); err != nil {
		t.Fatal(err)
	}
	for _, workspace := range []string{"a", "b"} {
		if err = e.repository.ProvisionWorkspace(ctx, workspace); err != nil {
			t.Fatal(err)
		}
	}
	e.access, err = store.NewAccess(map[store.Credential][]store.Membership{alice: {{Principal: "alice", Workspace: "a", Install: true, Read: true}}, bob: {{Principal: "bob", Workspace: "b", Install: true, Read: true}}})
	if err != nil {
		t.Fatal(err)
	}
	var version string
	if err = db.QueryRowContext(ctx, `SHOW server_version`).Scan(&version); err != nil {
		t.Fatal(err)
	}
	t.Log("real PostgreSQL", version, "isolated database", name)
	return e
}

func packageFixture(t *testing.T, kind, source string) *archive.Package {
	t.Helper()
	p, err := fixtures.Build(fixtures.Files("test.publisher/fixture", kind, source))
	if err != nil {
		t.Fatal(err)
	}
	return p
}

func approved(t *testing.T, p *archive.Package, retention string, tests []install.Test) install.Approval {
	t.Helper()
	d, err := p.Manifest()
	if err != nil {
		t.Fatal(err)
	}
	return install.Approval{RightsDigest: install.RightsDigest(*d.Package), Safety: "ACTIVE", Retention: retention, Tests: tests}
}

func (e *environment) installer(t *testing.T, p *archive.Package, retention string, observe func(string) error, runtime install.RuntimeConfig, tests []install.Test, events func(install.Execution) error) *install.Installer {
	t.Helper()
	policy, err := install.NewPolicy(install.PolicyConfig{Context: "ci", HostMajor: 1, Artifacts: map[string]install.Approval{string(p.ArtifactIdentity().Digest()): approved(t, p, retention, tests)}})
	if err != nil {
		t.Fatal(err)
	}
	if observe == nil {
		observe = func(string) error { return nil }
	}
	if events == nil {
		events = func(install.Execution) error { return nil }
	}
	i, err := install.New(install.Options{StagingRoot: e.staging, Policy: policy, Access: e.access, Objects: e.objects, Repository: e.repository, Support: extension.DefaultSupport, Runtime: runtime, Observe: observe, Execution: events})
	if err != nil {
		t.Fatal(err)
	}
	return i
}

func request(t *testing.T, p *archive.Package, id, workspace string, credential store.Credential) install.Request {
	t.Helper()
	raw, err := fixtures.Archive(p)
	if err != nil {
		t.Fatal(err)
	}
	return install.Request{Credential: credential, Workspace: workspace, ID: id, Root: install.Input{Archive: bytes.NewReader(raw)}}
}

func (e *environment) assertCounts(t *testing.T, artifacts, grants, requests int) {
	t.Helper()
	for table, want := range map[string]int{"artifacts": artifacts, "grants": grants, "requests": requests} {
		var n int
		if err := e.db.QueryRow(`SELECT count(*) FROM package_install.` + table).Scan(&n); err != nil {
			t.Fatal(err)
		}
		if n != want {
			t.Fatalf("%s visible rows=%d, want %d", table, n, want)
		}
	}
}

func (e *environment) reader(t *testing.T) *store.Reader {
	t.Helper()
	r, err := store.NewReader(e.repository, e.objects, e.access, extension.DefaultSupport)
	if err != nil {
		t.Fatal(err)
	}
	return r
}

func TestAtomicInstallOrderingAndIdempotentReadback(t *testing.T) {
	e := setup(t)
	p := packageFixture(t, "assets", "")
	var steps []string
	i := e.installer(t, p, "a-retention", func(s string) error {
		steps = append(steps, s)
		if s != "committed" {
			e.assertCounts(t, 0, 0, 0)
		}
		return nil
	}, install.RuntimeConfig{}, nil, nil)
	result, err := i.Install(context.Background(), request(t, p, "first", "a", alice))
	if err != nil {
		t.Fatal(err)
	}
	e.assertCounts(t, 1, 1, 1)
	want := []string{"authorized", "staged", "policy-validated", "runtime-validated", "fresh-install-preflight-no-affected-state", "before-object-persist", "objects-persisted", "before-publish", "committed"}
	if strings.Join(steps, ",") != strings.Join(want, ",") {
		t.Fatal("validation/publication ordering", steps)
	}
	loaded, err := e.reader(t).Load(context.Background(), alice, "a", result.Root)
	if err != nil || loaded.ArtifactIdentity().Digest() != p.ArtifactIdentity().Digest() {
		t.Fatal(err)
	}
	i = e.installer(t, p, "a-retention", nil, install.RuntimeConfig{}, nil, nil)
	again, err := i.Install(context.Background(), request(t, p, "first", "a", alice))
	if err != nil || again.Root != result.Root || again.Fingerprint != result.Fingerprint {
		t.Fatal("idempotent retry", again, err)
	}
	e.assertCounts(t, 1, 1, 1)
	rows, err := os.ReadDir(e.staging)
	if err != nil || len(rows) != 0 {
		t.Fatal("private staging not cleaned", err)
	}
}

func TestRealServicesFailureCancelAndCommitRecovery(t *testing.T) {
	for _, point := range []string{"authorized", "staged", "policy-validated", "runtime-validated", "fresh-install-preflight-no-affected-state", "before-object-persist", "objects-persisted", "before-publish", "before-transaction", "after-metadata-before-commit", "before-commit", "during-commit", "after-commit-before-ack", "committed", "cancel-before-commit", "cancel-after-commit"} {
		t.Run(point, func(t *testing.T) {
			e := setup(t)
			p := packageFixture(t, "assets", "")
			ctx, cancel := context.WithCancel(context.Background())
			defer cancel()
			observe := func(s string) error {
				if s == point {
					return injected
				}
				return nil
			}
			e.fault = func(ctx context.Context, s string, tx *sql.Tx) error {
				if s == "after-metadata-before-commit" {
					e.assertCounts(t, 0, 0, 0)
				}
				if point == "during-commit" && s == "before-commit" {
					var pid int
					if err := tx.QueryRowContext(ctx, `SELECT pg_backend_pid()`).Scan(&pid); err != nil {
						return err
					}
					var killed bool
					if err := e.db.QueryRowContext(ctx, `SELECT pg_terminate_backend($1)`, pid).Scan(&killed); err != nil {
						return err
					}
					if !killed {
						return fmt.Errorf("backend not terminated")
					}
					return nil
				}
				if point == "cancel-before-commit" && s == "after-metadata-before-commit" {
					cancel()
					return nil
				}
				if point == "cancel-after-commit" && s == "after-commit-before-ack" {
					cancel()
					return nil
				}
				if s == point {
					return injected
				}
				return nil
			}
			i := e.installer(t, p, "fixture", observe, install.RuntimeConfig{}, nil, nil)
			result, err := i.Install(ctx, request(t, p, "failure", "a", alice))
			committed := point == "after-commit-before-ack" || point == "committed" || point == "cancel-after-commit"
			if committed {
				e.assertCounts(t, 1, 1, 1)
				if point == "committed" {
					if !errors.Is(err, store.ErrUnknownCommit) {
						t.Fatal("lost acknowledgement classification", err)
					}
				} else if err != nil || result.Root == "" {
					t.Fatal("commit recovery", err)
				}
			} else {
				if err == nil {
					t.Fatal("failure ignored")
				}
				e.assertCounts(t, 0, 0, 0)
				if _, err = e.reader(t).Load(context.Background(), alice, "a", string(p.ArtifactIdentity().Digest())); err == nil {
					t.Fatal("partial install became readable")
				}
			}
			e.fault = nil
			i = e.installer(t, p, "fixture", nil, install.RuntimeConfig{}, nil, nil)
			recovered, err := i.Install(context.Background(), request(t, p, "failure", "a", alice))
			if err != nil || recovered.Root != string(p.ArtifactIdentity().Digest()) {
				t.Fatal("retry/recovery", err)
			}
			e.assertCounts(t, 1, 1, 1)
		})
	}
}

func TestPhysicalDeduplicationKeepsWorkspaceRightsAndPermissionsIndependent(t *testing.T) {
	e := setup(t)
	p := packageFixture(t, "assets", "")
	a := e.installer(t, p, "retain-a", nil, install.RuntimeConfig{}, nil, nil)
	b := e.installer(t, p, "retain-b", nil, install.RuntimeConfig{}, nil, nil)
	for _, call := range []struct {
		i       *install.Installer
		request install.Request
	}{{a, request(t, p, "a", "a", alice)}, {b, request(t, p, "b", "b", bob)}} {
		if _, err := call.i.Install(context.Background(), call.request); err != nil {
			t.Fatal(err)
		}
	}
	e.assertCounts(t, 2, 2, 2)
	rows, err := os.ReadDir(e.root)
	if err != nil || len(rows) != len(p.Entries())+1 {
		t.Fatal("physical deduplication failed", len(rows), err)
	}
	identity := string(p.ArtifactIdentity().Digest())
	for _, workspace := range []string{"a", "b"} {
		credential := alice
		principal := "alice"
		retention := "retain-a"
		if workspace == "b" {
			credential = bob
			principal = "bob"
			retention = "retain-b"
		}
		if _, err = e.reader(t).Load(context.Background(), credential, workspace, identity); err != nil {
			t.Fatal(err)
		}
		artifact, err := e.repository.Lookup(context.Background(), workspace, principal, identity)
		if err != nil || artifact.Retention != retention {
			t.Fatal("retention isolation", err)
		}
	}
	if _, err = e.reader(t).Load(context.Background(), bob, "a", identity); err == nil {
		t.Fatal("tenant bypass")
	}
	artifact, err := e.repository.Lookup(context.Background(), "a", "alice", identity)
	if err != nil {
		t.Fatal(err)
	}
	if _, err = e.reader(t).Load(context.Background(), alice, "a", artifact.ArchiveKey); err == nil {
		t.Fatal("object key acted as access grant")
	}
}

func TestConcurrentExactRetriesDoNotDuplicateOrDeleteSharedBytes(t *testing.T) {
	e := setup(t)
	p := packageFixture(t, "assets", "")
	i := e.installer(t, p, "fixture", nil, install.RuntimeConfig{}, nil, nil)
	var wg sync.WaitGroup
	for range 10 {
		r := request(t, p, "concurrent", "a", alice)
		wg.Go(func() {
			if _, err := i.Install(context.Background(), r); err != nil {
				t.Errorf("concurrent retry: %v", err)
			}
		})
	}
	wg.Wait()
	e.assertCounts(t, 1, 1, 1)
	if _, err := e.reader(t).Load(context.Background(), alice, "a", string(p.ArtifactIdentity().Digest())); err != nil {
		t.Fatal(err)
	}
	files := fixtures.Files("test.publisher/fixture", "assets", "")
	files["content/readme.txt"] = []byte("changed")
	changed, err := fixtures.Build(files)
	if err != nil {
		t.Fatal(err)
	}
	other := e.installer(t, changed, "fixture", nil, install.RuntimeConfig{}, nil, nil)
	if _, err = other.Install(context.Background(), request(t, changed, "concurrent", "a", alice)); !errors.Is(err, store.ErrConflict) {
		t.Fatal("request identity reused with different bytes", err)
	}
	e.assertCounts(t, 1, 1, 1)
}

func TestFreshInstallMigrationPreflightFailsClosed(t *testing.T) {
	e := setup(t)
	p := packageFixture(t, "assets", "")
	if _, err := e.db.Exec(`INSERT INTO package_install.data_targets(workspace,package_id,state_reference) VALUES('a','test.publisher/fixture','existing-session-state')`); err != nil {
		t.Fatal(err)
	}
	i := e.installer(t, p, "fixture", nil, install.RuntimeConfig{}, nil, nil)
	if _, err := i.Install(context.Background(), request(t, p, "migration", "a", alice)); !errors.Is(err, store.ErrMigration) {
		t.Fatal("unsupported affected data accepted", err)
	}
	e.assertCounts(t, 0, 0, 0)
	var state string
	if err := e.db.QueryRow(`SELECT state_reference FROM package_install.data_targets`).Scan(&state); err != nil || state != "existing-session-state" {
		t.Fatal("existing state changed", err)
	}
}

func TestObjectLossBeforeCommitRollsBackAllVisibleMetadata(t *testing.T) {
	e := setup(t)
	p := packageFixture(t, "assets", "")
	e.fault = func(ctx context.Context, point string, tx *sql.Tx) error {
		if point == "before-commit" {
			rows, err := os.ReadDir(e.root)
			if err != nil {
				return err
			}
			return os.Remove(filepath.Join(e.root, rows[0].Name()))
		}
		return nil
	}
	i := e.installer(t, p, "fixture", nil, install.RuntimeConfig{}, nil, nil)
	if _, err := i.Install(context.Background(), request(t, p, "object-loss", "a", alice)); err == nil {
		t.Fatal("missing immutable object accepted")
	}
	e.assertCounts(t, 0, 0, 0)
	e.fault = nil
	if _, err := i.Install(context.Background(), request(t, p, "object-loss", "a", alice)); err != nil {
		t.Fatal("retry after invisible failed install", err)
	}
	e.assertCounts(t, 1, 1, 1)
}

func TestInstalledExtensionsPreserveCanonicalAndOpaqueBytes(t *testing.T) {
	e := setup(t)
	schema := []byte(`{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","properties":{"count":{"type":"integer"}},"required":["count"],"additionalProperties":false}`)
	for index, contract := range []int{1, 2} {
		files := fixtures.Files(fmt.Sprintf("test.publisher/extension-%d", contract), "assets", "")
		files[archive.ManifestPath] = []byte(strings.Replace(string(files[archive.ManifestPath]), "schema_version = 1", "schema_version = 2", 1) + fmt.Sprintf("\n[[extensions]]\nnamespace = \"third.party.probe\"\nrequired = %t\ncontract_version = %d\nschema_path = \"extensions/third.party.probe/value.schema.json\"\nschema_sha256 = %q\npayload_path = \"extensions/third.party.probe/value.json\"\nhost_api_major = 1\nhost_api_min_minor = 0\nhost_api_max_minor = 0\n", contract == 1, contract, object.Hash(schema)))
		files["extensions/third.party.probe/value.schema.json"] = schema
		files["extensions/third.party.probe/value.json"] = []byte(" { \"count\" : 1 } \n")
		p, err := fixtures.Build(files)
		if err != nil {
			t.Fatal(err)
		}
		i := e.installer(t, p, "extension", nil, install.RuntimeConfig{}, nil, nil)
		result, err := i.Install(context.Background(), request(t, p, fmt.Sprintf("extension-%d", index), "a", alice))
		if err != nil {
			t.Fatal(err)
		}
		loaded, err := e.reader(t).Load(context.Background(), alice, "a", result.Root)
		if err != nil {
			t.Fatal(err)
		}
		original, _ := fixtures.Archive(p)
		again, _ := fixtures.Archive(loaded)
		if !bytes.Equal(original, again) || p.ContentHash() != loaded.ContentHash() {
			t.Fatal("canonical archive changed")
		}
		for _, entry := range p.Entries() {
			got, ok := loaded.Entry(entry.Path())
			if !ok || !bytes.Equal(entry.Bytes(), got.Bytes()) {
				t.Fatal("extension bytes lost", entry.Path())
			}
		}
	}
}

func TestProductionProfileFailureNeverPublishes(t *testing.T) {
	e := setup(t)
	runner := filepath.Join(t.TempDir(), "lua-runner")
	cmd := exec.Command("go", "build", "-trimpath", "-o", runner, "../../../cmd/lua-runner")
	if out, err := cmd.CombinedOutput(); err != nil {
		t.Fatalf("runner build: %v %s", err, out)
	}
	raw, err := os.ReadFile(runner)
	if err != nil {
		t.Fatal(err)
	}
	runtime := install.RuntimeConfig{Runner: runner, SHA256: object.Hash(raw), Limits: profile.DefaultLimits()}
	for index, source := range []string{"return true", "return os.getenv('HOME')", "while true do end", "return false"} {
		p := packageFixture(t, "game-system", "return {count=7}")
		var events []install.Execution
		i := e.installer(t, p, "runtime", nil, runtime, []install.Test{{Name: "real-production-test", Source: []byte(source)}}, func(event install.Execution) error { events = append(events, event); return nil })
		result, err := i.Install(context.Background(), request(t, p, fmt.Sprintf("runtime-%d", index), "a", alice))
		if index == 0 {
			if err != nil || result.Root == "" {
				t.Fatal(err)
			}
		} else if err == nil {
			t.Fatal("production failure accepted", source)
		}
		e.assertCounts(t, 1, 1, 1)
		for _, event := range events {
			if event.Reaped {
				if _, err = os.Stat(fmt.Sprintf("/proc/%d", event.PID)); !os.IsNotExist(err) {
					t.Fatal("runner leak", event.PID, err)
				}
			}
		}
		if len(events) == 0 {
			t.Fatal("production tests did not execute")
		}
	}
}

func TestInstallerProcessCrashLeavesOnlyCompleteCommittedInstall(t *testing.T) {
	helper := filepath.Join(t.TempDir(), "crash-helper")
	build := exec.Command("go", "build", "-trimpath", "-o", helper, "./testdata/crash")
	if out, err := build.CombinedOutput(); err != nil {
		t.Fatalf("helper build: %v %s", err, out)
	}
	for _, phase := range []string{"before-object-persist", "objects-persisted", "after-metadata-before-commit", "after-commit-before-ack"} {
		t.Run(phase, func(t *testing.T) {
			e := setup(t)
			p := packageFixture(t, "assets", "")
			cmd := exec.Command(helper)
			cmd.Env = append(os.Environ(), "B003_CRASH_DSN="+e.dsn, "B003_CRASH_OBJECTS="+e.root, "B003_CRASH_STAGING="+e.staging, "B003_CRASH_PHASE="+phase)
			err := cmd.Run()
			var failure *exec.ExitError
			if !errors.As(err, &failure) || failure.ExitCode() != 23 {
				t.Fatal("crash point was not executed", err)
			}
			if phase == "after-commit-before-ack" {
				e.assertCounts(t, 1, 1, 1)
			} else {
				e.assertCounts(t, 0, 0, 0)
			}
			i := e.installer(t, p, "fixture", nil, install.RuntimeConfig{}, nil, nil)
			result, err := i.Install(context.Background(), request(t, p, "crash", "a", alice))
			if err != nil || result.Root != string(p.ArtifactIdentity().Digest()) {
				t.Fatal("restart recovery", err)
			}
			e.assertCounts(t, 1, 1, 1)
			if _, err = e.reader(t).Load(context.Background(), alice, "a", result.Root); err != nil {
				t.Fatal(err)
			}
		})
	}
}
