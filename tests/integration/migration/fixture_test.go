//go:build linux && integration

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package migration_test

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"net/url"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"sync"
	"sync/atomic"
	"syscall"
	"testing"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/eventstore"
	"github.com/zyc14588/TRPG_PLATFORM/internal/hostapi"
	runnerfixture "github.com/zyc14588/TRPG_PLATFORM/internal/hostapi/hostapitest"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/archive"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/extension"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/install"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	fixture "github.com/zyc14588/TRPG_PLATFORM/internal/package/testdata/migration"
	sessionmigration "github.com/zyc14588/TRPG_PLATFORM/internal/session/migration"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/recovery"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/object"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/postgres"
)

var dsn, runner, runnerHash, candidate, runID, daemonBinary string
var sequence atomic.Uint64

const credential = "b007-synthetic-credential"
const readOnlyCredential = "b007-read-only-synthetic-credential"

func TestMain(m *testing.M) {
	dsn = os.Getenv("B007_POSTGRES_DSN")
	u, err := url.Parse(dsn)
	if err != nil || u.Scheme != "postgres" || u.Hostname() != "127.0.0.1" || u.Path != "/b007_fixture" {
		fmt.Fprintln(os.Stderr, "B007 owned local PostgreSQL required; integration NOT_RUN")
		os.Exit(1)
	}
	dir, err := os.MkdirTemp("", "b007-production-runner-")
	if err != nil {
		panic(err)
	}
	root, err := filepath.Abs("../../..")
	if err != nil {
		panic(err)
	}
	raw, err := exec.Command("git", "-C", root, "rev-parse", "HEAD").Output()
	if err != nil {
		panic(err)
	}
	candidate = string(bytes.TrimSpace(raw))
	if len(candidate) != 40 {
		panic("exact candidate SHA required")
	}
	runner, err = runnerfixture.BuildRunner(root, dir)
	if err != nil {
		panic(err)
	}
	raw, err = os.ReadFile(runner)
	if err != nil {
		panic(err)
	}
	runnerHash = checkpoint.Hash(raw)
	daemonBinary = filepath.Join(dir, "platformd")
	build := exec.Command("go", "build", "-trimpath", "-o", daemonBinary, "./cmd/platformd")
	build.Dir = root
	if raw, err := build.CombinedOutput(); err != nil {
		panic(fmt.Sprintf("source platformd build failed: %v: %s", err, raw))
	}
	raw, err = os.ReadFile(daemonBinary)
	if err != nil {
		panic(err)
	}
	fmt.Printf("B007_PRODUCTION_PLATFORMD %s CANDIDATE %s\n", checkpoint.Hash(raw), candidate)
	runID = fmt.Sprintf("b007-%d", time.Now().UnixNano())
	fmt.Printf("B007_PRODUCTION_RUNNER %s %s %s CANDIDATE %s\n", runnerHash, profile.ID, profile.RuntimeVersion, candidate)
	code := m.Run()
	os.RemoveAll(dir)
	os.Exit(code)
}
func runtime() install.RuntimeConfig {
	return install.RuntimeConfig{Runner: runner, SHA256: runnerHash, Limits: profile.DefaultLimits()}
}

type environment struct {
	pair       fixture.Pair
	factory    *install.SessionFactory
	host       *postgres.HostRepository
	repository *postgres.Repository
	reader     *store.Reader
	workspace  string
	mu         sync.Mutex
	executions []install.Execution
}

func setup(t *testing.T, safe, critical bool) *environment {
	return setupOptions(t, fixture.Options{Safe: safe, Critical: critical})
}
func setupOptions(t *testing.T, options fixture.Options) *environment {
	t.Helper()
	e := &environment{workspace: fmt.Sprintf("%s-%d", runID, sequence.Add(1))}
	var err error
	e.pair, err = fixture.BuildPairWithOptions(runtime(), options)
	if err != nil {
		t.Fatal("build pair", err)
	}
	policy, err := install.NewPolicy(e.pair.Config)
	if err != nil {
		t.Fatal(err)
	}
	root, staging := t.TempDir(), t.TempDir()
	if err = os.Chmod(root, 0700); err != nil {
		t.Fatal(err)
	}
	if err = os.Chmod(staging, 0700); err != nil {
		t.Fatal(err)
	}
	objects, err := object.Open(root)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		if err := objects.Close(); err != nil {
			t.Error(err)
		}
	})
	e.repository, err = postgres.OpenInstallationRepository(context.Background(), dsn, objects, extension.DefaultSupport, nil)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { e.repository.Close() })
	if err = e.repository.Bootstrap(context.Background()); err != nil {
		t.Fatal(err)
	}
	if err = e.repository.ProvisionWorkspace(context.Background(), e.workspace); err != nil {
		t.Fatal(err)
	}
	e.host, err = postgres.OpenHostRepository(context.Background(), dsn, nil)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { e.host.Close() })
	if err = e.host.Bootstrap(context.Background()); err != nil {
		t.Fatal(err)
	}
	access, err := store.NewAccess(map[store.Credential][]store.Membership{store.Credential(credential): {{Principal: "operator", Workspace: e.workspace, Read: true, Install: true}}, readOnlyCredential: {{Principal: "reader", Workspace: e.workspace, Read: true}}})
	if err != nil {
		t.Fatal(err)
	}
	e.reader, err = store.NewReader(e.repository, objects, access, extension.DefaultSupport)
	if err != nil {
		t.Fatal(err)
	}
	installer, err := install.New(install.Options{StagingRoot: staging, Policy: policy, Access: access, Objects: objects, Repository: e.repository, Support: extension.DefaultSupport, Runtime: runtime(), Observe: func(string) error { return nil }, Execution: e.observe})
	if err != nil {
		t.Fatal(err)
	}
	input := func(p *archive.Package) install.Input {
		raw, err := p.Export()
		if err != nil {
			t.Fatal(err)
		}
		return install.Input{Archive: bytes.NewReader(raw.Bytes()), Evidence: e.pair.Evidence[string(p.ArtifactIdentity().Digest())]}
	}
	for n, g := range []fixture.Graph{e.pair.Old, e.pair.New} {
		deps := []install.Input{}
		for _, p := range g.Dependencies {
			deps = append(deps, input(p))
		}
		if _, err = installer.Install(context.Background(), install.Request{Credential: store.Credential(credential), Workspace: e.workspace, ID: fmt.Sprintf("install-%d", n), Root: input(g.Root), Dependencies: deps}); err != nil {
			t.Fatalf("install-%d: %v; executions=%+v", n, err, e.executions)
		}
	}
	e.factory, err = install.NewSessionFactory(install.SessionOptions{Reader: e.reader, Policy: policy, Repository: e.host, Runtime: runtime(), Execution: e.observe, Audit: func(data.Audit) error { return nil }, Validate: func(context.Context, data.Commit) error { return nil }})
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { e.assertReaped(t) })
	return e
}
func (e *environment) observe(v install.Execution) error {
	e.mu.Lock()
	defer e.mu.Unlock()
	e.executions = append(e.executions, v)
	return nil
}
func (e *environment) assertReaped(t *testing.T) {
	t.Helper()
	e.mu.Lock()
	defer e.mu.Unlock()
	pids := map[int]bool{}
	for _, v := range e.executions {
		if v.PID > 0 {
			pids[v.PID] = !v.Reaped
		}
	}
	for id, alive := range pids {
		if alive || syscall.Kill(id, 0) != syscall.ESRCH {
			t.Errorf("runner %d not reaped", id)
		}
	}
	t.Logf("B007_RUNNER_REAP actual ESRCH for %d production PIDs", len(pids))
}
func (e *environment) request(sid string, next bool) install.SessionRequest {
	return e.pair.Request(credential, e.workspace, sid, next)
}
func (e *environment) create(t *testing.T, sid string, commands int) data.Binding {
	t.Helper()
	s, err := e.factory.Create(context.Background(), e.request(sid, false))
	if err != nil {
		t.Fatal(err)
	}
	b := s.Binding
	for n := 0; n < commands; n++ {
		_, err = s.Commands.Execute(context.Background(), s.VM.Token(), hostapi.Command{ID: fmt.Sprintf("increment-%d", n), Principal: "operator", ExpectedVersion: s.VM.StateVersion(), Input: fixture.Command(1)})
		if err != nil {
			s.Close()
			t.Fatal(err)
		}
	}
	if err = s.Close(); err != nil {
		t.Fatal(err)
	}
	return b
}
func (e *environment) operator(t *testing.T, r *postgres.HostRepository) *sessionmigration.Operator {
	t.Helper()
	o, err := sessionmigration.New(e.factory, func(ctx context.Context, from, to *store.Graph, b data.Binding, v uint64) (sessionmigration.Lease, error) {
		return r.AcquireMigration(ctx, from, to, b, v)
	})
	if err != nil {
		t.Fatal(err)
	}
	return o
}
func (e *environment) migrationRequest(sid string, version uint64) sessionmigration.Request {
	return sessionmigration.Request{From: e.request(sid, false), To: e.request(sid, true), ExpectedVersion: version, PointID: "before-upgrade", CommandID: "upgrade", Plan: e.pair.Plan()}
}
func (e *environment) physical(t *testing.T, b data.Binding, version uint64) data.Snapshot {
	t.Helper()
	tx, err := e.host.Begin(context.Background(), data.Header{Binding: b, Principal: "operator", CommandID: "inspect-physical", Fingerprint: eventstore.Digest("physical-readback"), ExpectedVersion: version, ReadOnly: true})
	if err != nil {
		t.Fatal(err)
	}
	s := tx.Snapshot()
	if err = tx.Rollback(); err != nil {
		t.Fatal(err)
	}
	return s
}
func (e *environment) reconstruct(t *testing.T, sid string, next bool) recovery.Report {
	t.Helper()
	service, err := recovery.New(e.host)
	if err != nil {
		t.Fatal(err)
	}
	s, err := e.factory.Recover(context.Background(), e.request(sid, next), service.Build)
	if err != nil {
		t.Fatal(err)
	}
	report, err := service.Reconstruct(context.Background(), s.Recovery)
	closeErr := s.Close()
	if err != nil || closeErr != nil {
		t.Fatal(err, closeErr)
	}
	return report
}
func evidence(t *testing.T, name string, v any) {
	t.Helper()
	// Receipt inputs contain private fixture state; evidence keeps their digest.
	rawValue, err := json.Marshal(v)
	if err != nil {
		t.Fatal(err)
	}
	var metadata any
	if err = json.Unmarshal(rawValue, &metadata); err != nil {
		t.Fatal(err)
	}
	var redact func(any)
	redact = func(v any) {
		switch x := v.(type) {
		case map[string]any:
			for key, value := range x {
				if strings.EqualFold(key, "receipt") {
					raw, _ := json.Marshal(value)
					x[key] = map[string]any{"hash": checkpoint.Hash(raw)}
				} else {
					redact(value)
				}
			}
		case []any:
			for _, value := range x {
				redact(value)
			}
		}
	}
	redact(metadata)
	raw, err := json.Marshal(struct {
		Case, CandidateSHA string
		Result             any
	}{name, candidate, metadata})
	if err != nil {
		t.Fatal(err)
	}
	t.Logf("B007_MIGRATION_RESULT %s", raw)
}
