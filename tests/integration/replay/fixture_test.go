//go:build linux && integration

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package replay_test

import (
	"bytes"
	"context"
	"fmt"
	runnerfixture "github.com/zyc14588/TRPG_PLATFORM/internal/hostapi/hostapitest"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/archive"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/extension"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/install"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/actor"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/command"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/persistence"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/realtime"
	fixture "github.com/zyc14588/TRPG_PLATFORM/internal/session/testdata"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/object"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/postgres"
	"net/url"
	"os"
	"os/exec"
	"path/filepath"
	"sync"
	"sync/atomic"
	"syscall"
	"testing"
	"time"
)

var dsn, runner, runnerHash, runID, daemonBinary string
var sequence atomic.Uint64

const credential = store.Credential("b006-synthetic-install-credential")

func TestMain(m *testing.M) {
	dsn = os.Getenv("B006_POSTGRES_DSN")
	u, err := url.Parse(dsn)
	if err != nil || u.Scheme != "postgres" || u.Hostname() != "127.0.0.1" || u.Path != "/b006_fixture" {
		fmt.Fprintln(os.Stderr, "B006 owned local PostgreSQL required; integration NOT_RUN")
		os.Exit(1)
	}
	dir, err := os.MkdirTemp("", "b006-pg-runner-")
	if err != nil {
		panic(err)
	}
	root, err := filepath.Abs("../../..")
	if err != nil {
		panic(err)
	}
	runner, err = runnerfixture.BuildRunner(root, dir)
	if err != nil {
		panic(err)
	}
	raw, err := os.ReadFile(runner)
	if err != nil {
		panic(err)
	}
	runnerHash = checkpoint.Hash(raw)
	daemonBinary = filepath.Join(dir, "platformd")
	build := exec.Command("go", "build", "-trimpath", "-o", daemonBinary, "./cmd/platformd")
	build.Dir = root
	if raw, err := build.CombinedOutput(); err != nil {
		panic(fmt.Sprintf("platformd build %s: %v", raw, err))
	}
	daemonRaw, err := os.ReadFile(daemonBinary)
	if err != nil {
		panic(err)
	}
	fmt.Printf("B006_PRODUCTION_PLATFORMD %s\n", checkpoint.Hash(daemonRaw))
	runID = fmt.Sprintf("b006-%d", time.Now().UnixNano())
	fmt.Printf("B006_PRODUCTION_RUNNER %s %s %s\n", runnerHash, profile.ID, profile.RuntimeVersion)
	code := m.Run()
	os.RemoveAll(dir)
	os.Exit(code)
}

type environment struct {
	pkg        *archive.Package
	policy     *install.Policy
	config     install.PolicyConfig
	objects    *object.Directory
	repository *postgres.Repository
	host       *postgres.HostRepository
	reader     *store.Reader
	workspace  string
	mu         sync.Mutex
	executions []install.Execution
	fault      func(context.Context, string) error
}

func runtime() install.RuntimeConfig {
	return install.RuntimeConfig{Runner: runner, SHA256: runnerHash, Limits: profile.DefaultLimits()}
}
func setup(t *testing.T, source string) *environment {
	t.Helper()
	e := &environment{workspace: fmt.Sprintf("%s-%d", runID, sequence.Add(1))}
	pkg, config, err := fixture.Build(runtime(), source)
	if err != nil {
		t.Fatal(err)
	}
	e.pkg = pkg
	e.config = config
	e.policy, err = install.NewPolicy(config)
	if err != nil {
		t.Fatal(err)
	}
	storage, stage := t.TempDir(), t.TempDir()
	for _, p := range []string{storage, stage} {
		if err = os.Chmod(p, 0700); err != nil {
			t.Fatal(err)
		}
	}
	objects, err := object.Open(storage)
	if err != nil {
		t.Fatal(err)
	}
	e.objects = objects
	t.Cleanup(func() { objects.Close() })
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
	e.host, err = postgres.OpenHostRepository(context.Background(), dsn, func(ctx context.Context, point string) error {
		e.mu.Lock()
		f := e.fault
		e.mu.Unlock()
		if f != nil {
			return f(ctx, point)
		}
		return nil
	})
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { e.host.Close() })
	if err = e.host.Bootstrap(context.Background()); err != nil {
		t.Fatal(err)
	}
	access, err := store.NewAccess(map[store.Credential][]store.Membership{credential: {{Principal: "operator", Workspace: e.workspace, Read: true, Install: true}}})
	if err != nil {
		t.Fatal(err)
	}
	e.reader, err = store.NewReader(e.repository, objects, access, extension.DefaultSupport)
	if err != nil {
		t.Fatal(err)
	}
	installer, err := install.New(install.Options{StagingRoot: stage, Policy: e.policy, Access: access, Objects: objects, Repository: e.repository, Support: extension.DefaultSupport, Runtime: runtime(), Observe: func(string) error { return nil }, Execution: e.observe})
	if err != nil {
		t.Fatal(err)
	}
	exported, exportErr := e.pkg.Export()
	if exportErr != nil {
		t.Fatal(exportErr)
	}
	raw := exported.Bytes()
	if _, err = installer.Install(context.Background(), install.Request{Credential: credential, Workspace: e.workspace, ID: "install", Root: install.Input{Archive: bytes.NewReader(raw)}}); err != nil {
		t.Fatal(err)
	}
	return e
}
func (e *environment) observe(v install.Execution) error {
	e.mu.Lock()
	defer e.mu.Unlock()
	e.executions = append(e.executions, v)
	return nil
}
func (e *environment) request(sid string) install.SessionRequest {
	return install.SessionRequest{Credential: credential, Workspace: e.workspace, Session: sid, Root: string(e.pkg.ArtifactIdentity().Digest())}
}
func (e *environment) factory(t *testing.T, host *postgres.HostRepository) *install.SessionFactory {
	t.Helper()
	f, err := install.NewSessionFactory(install.SessionOptions{Reader: e.reader, Policy: e.policy, Repository: host, Runtime: runtime(), Execution: e.observe, Audit: func(data.Audit) error { return nil }, Validate: func(context.Context, data.Commit) error { return nil }})
	if err != nil {
		t.Fatal(err)
	}
	return f
}
func (e *environment) create(t *testing.T, sid string) data.Binding {
	t.Helper()
	s, err := e.factory(t, e.host).Create(context.Background(), e.request(sid))
	if err != nil {
		t.Fatal(err)
	}
	b := s.Binding
	if err = s.Close(); err != nil {
		t.Fatal(err)
	}
	return b
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
		t.Logf("B006_EXECUTION %+v", v)
	}
	for pid, alive := range pids {
		if alive || syscall.Kill(pid, 0) != syscall.ESRCH {
			t.Fatalf("runner %d not joined/reaped", pid)
		}
	}
}
func (e *environment) executionCount() int {
	e.mu.Lock()
	defer e.mu.Unlock()
	return len(e.executions)
}
func (e *environment) setFault(f func(context.Context, string) error) {
	e.mu.Lock()
	defer e.mu.Unlock()
	e.fault = f
}

type rig struct {
	registry   *actor.Registry
	authority  *command.Authority
	hub        *realtime.Hub
	gm, player command.Identity
	binding    data.Binding
}

func seatToken(sid, seat string) string {
	return "b006-fixture-" + sid + "-" + seat + "-0123456789abcdef"
}
func payload(delta int64) checkpoint.Value {
	return checkpoint.Object(map[string]checkpoint.Value{"delta": checkpoint.Int(delta)})
}
func validate(v checkpoint.Value) error {
	if v.Kind != "table" || len(v.Table) != 1 || v.Table["delta"].Kind != "integer" {
		return command.ErrEnvelope
	}
	n := v.Table["delta"].Number
	if n != "1" {
		return command.ErrEnvelope
	}
	return nil
}
func newRig(t *testing.T, e *environment, b data.Binding, repo *postgres.HostRepository) *rig {
	t.Helper()
	seats := []command.FixtureSeat{}
	for _, seat := range []string{"gm", "player"} {
		fields := []string{"counter"}
		if seat == "gm" {
			fields = append(fields, "secret")
		}
		seats = append(seats, command.FixtureSeat{Credential: seatToken(b.Session, seat), Binding: b, Principal: seat, Seat: seat, Commands: map[string]func(checkpoint.Value) error{"increment": validate, "fail": validate, "end": validate}, Views: command.ViewPolicy{ViewFields: fields, EventFields: map[string][]string{fixture.PackageID + "/change": fields}, ScalarResult: true}})
	}
	a, err := command.NewFixtureAuthority(seats)
	if err != nil {
		t.Fatal(err)
	}
	hub, err := realtime.New(realtime.Options{Authority: a, Capacity: 4, PerSession: 4, Queue: 8})
	if err != nil {
		t.Fatal(err)
	}
	backend, err := persistence.New(persistence.Options{Factory: e.factory(t, repo), Repository: repo, Request: func(binding data.Binding) (install.SessionRequest, error) { return e.request(binding.Session), nil }, Time: 1000, Random: []int64{7}, ToolResults: []checkpoint.Value{checkpoint.Int(7)}})
	if err != nil {
		t.Fatal(err)
	}
	r, err := actor.New(context.Background(), actor.Options{Authority: a, Hub: hub, Backend: backend, MaxSessions: 2, Mailbox: 4, Idle: time.Hour, CommandTimeout: 5 * time.Second})
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { r.Close() })
	gm, err := a.Authenticate(seatToken(b.Session, "gm"), b.Session, "gm")
	if err != nil {
		t.Fatal(err)
	}
	player, err := a.Authenticate(seatToken(b.Session, "player"), b.Session, "player")
	if err != nil {
		t.Fatal(err)
	}
	return &rig{r, a, hub, gm, player, b}
}
func envelope(b data.Binding, id, typ string, version uint64) command.Envelope {
	return command.Envelope{CommandID: id, SessionID: b.Session, SeatID: "gm", Type: typ, Payload: payload(1), ExpectedStateVersion: version, CorrelationID: "correlation-" + id}
}
