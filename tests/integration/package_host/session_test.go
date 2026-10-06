//go:build linux && integration

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package packagehost_test

import (
	"bytes"
	"context"
	"errors"
	"fmt"
	"net/url"
	"os"
	"path/filepath"
	"sync"
	"sync/atomic"
	"syscall"
	"testing"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/hostapi"
	fixture "github.com/zyc14588/TRPG_PLATFORM/internal/hostapi/hostapitest"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/archive"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/extension"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/install"
	helper "github.com/zyc14588/TRPG_PLATFORM/internal/package/install/installtest"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	fixtures "github.com/zyc14588/TRPG_PLATFORM/internal/package/testdata/install"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/object"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/postgres"
)

var dsn, runner, runnerHash, runID string
var sequence atomic.Uint64

const credential = store.Credential("b013-synthetic-operator-credential")
const other = store.Credential("b013-synthetic-ungranted-credential")

func TestMain(m *testing.M) {
	dsn = os.Getenv("B013_POSTGRES_DSN")
	u, err := url.Parse(dsn)
	if err != nil || u.Scheme != "postgres" || u.Hostname() != "127.0.0.1" || u.Path != "/b013_fixture" {
		fmt.Fprintln(os.Stderr, "B013 owned local PostgreSQL fixture required; integration NOT_RUN")
		os.Exit(1)
	}
	dir, err := os.MkdirTemp("", "b013-pg-runner-")
	if err != nil {
		panic(err)
	}
	root, err := filepath.Abs("../../..")
	if err != nil {
		panic(err)
	}
	runner, err = fixture.BuildRunner(root, dir)
	if err != nil {
		panic(err)
	}
	raw, err := os.ReadFile(runner)
	if err != nil {
		panic(err)
	}
	runnerHash = checkpoint.Hash(raw)
	runID = fmt.Sprintf("b013-%d", time.Now().UnixNano())
	fmt.Printf("B013_PRODUCTION_RUNNER %s %s %s\n", runnerHash, profile.ID, profile.RuntimeVersion)
	code := m.Run()
	os.RemoveAll(dir)
	os.Exit(code)
}

type environment struct {
	root, dep                   *archive.Package
	config                      install.PolicyConfig
	policy                      *install.Policy
	objects                     *object.Directory
	repository                  *postgres.Repository
	host                        *postgres.HostRepository
	reader                      *store.Reader
	access                      *store.Access
	workspace, staging, storage string
	mu                          sync.Mutex
	executions                  []install.Execution
	installFault, hostFault     func(context.Context, string) error
	vmFault                     func(install.Execution) error
}

func setup(t *testing.T) *environment {
	t.Helper()
	e := &environment{workspace: fmt.Sprintf("%s-%d", runID, sequence.Add(1)), staging: t.TempDir(), storage: t.TempDir()}
	for _, p := range []string{e.staging, e.storage} {
		if err := os.Chmod(p, 0700); err != nil {
			t.Fatal(err)
		}
	}
	var err error
	e.root, e.dep, err = helper.Graph("")
	if err != nil {
		t.Fatal(err)
	}
	e.config, err = helper.Config(e.root, e.dep, e.runtime())
	if err != nil {
		t.Fatal(err)
	}
	e.policy, err = install.NewPolicy(e.config)
	if err != nil {
		t.Fatal(err)
	}
	e.objects, err = object.Open(e.storage)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { e.objects.Close() })
	e.repository, err = postgres.OpenInstallationRepository(context.Background(), dsn, e.objects, extension.DefaultSupport, func(ctx context.Context, p string) error {
		if e.installFault != nil {
			return e.installFault(ctx, p)
		}
		return nil
	})
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
	e.host, err = postgres.OpenHostRepository(context.Background(), dsn, func(ctx context.Context, p string) error {
		if e.hostFault != nil {
			return e.hostFault(ctx, p)
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
	e.access, err = store.NewAccess(map[store.Credential][]store.Membership{credential: {{Principal: "operator", Workspace: e.workspace, Read: true, Install: true}}, other: {{Principal: "ungranted", Workspace: e.workspace, Read: true, Install: true}}})
	if err != nil {
		t.Fatal(err)
	}
	e.reader, err = store.NewReader(e.repository, e.objects, e.access, extension.DefaultSupport)
	if err != nil {
		t.Fatal(err)
	}
	return e
}
func (e *environment) runtime() install.RuntimeConfig {
	return install.RuntimeConfig{Runner: runner, SHA256: runnerHash, Limits: profile.DefaultLimits()}
}
func (e *environment) observe(v install.Execution) error {
	e.mu.Lock()
	e.executions = append(e.executions, v)
	e.mu.Unlock()
	if e.vmFault != nil {
		return e.vmFault(v)
	}
	return nil
}
func (e *environment) installer(t *testing.T) *install.Installer {
	t.Helper()
	i, err := install.New(install.Options{StagingRoot: e.staging, Policy: e.policy, Access: e.access, Objects: e.objects, Repository: e.repository, Support: extension.DefaultSupport, Runtime: e.runtime(), Observe: func(string) error { return nil }, Execution: e.observe})
	if err != nil {
		t.Fatal(err)
	}
	return i
}
func (e *environment) installRequest(t *testing.T, id string) install.Request {
	t.Helper()
	input := func(p *archive.Package) install.Input {
		raw, err := fixtures.Archive(p)
		if err != nil {
			t.Fatal(err)
		}
		return install.Input{Archive: bytes.NewReader(raw)}
	}
	return install.Request{Credential: credential, Workspace: e.workspace, ID: id, Root: input(e.root), Dependencies: []install.Input{input(e.dep)}}
}
func (e *environment) publish(t *testing.T) {
	t.Helper()
	if _, err := e.installer(t).Install(context.Background(), e.installRequest(t, "initial-install")); err != nil {
		t.Fatal(err)
	}
}
func (e *environment) factory(t *testing.T, repo install.SessionRepository, policy *install.Policy) *install.SessionFactory {
	t.Helper()
	f, err := install.NewSessionFactory(install.SessionOptions{Reader: e.reader, Policy: policy, Repository: repo, Runtime: e.runtime(), Execution: e.observe, Audit: func(data.Audit) error { return nil }, Validate: func(context.Context, data.Commit) error { return nil }})
	if err != nil {
		t.Fatal(err)
	}
	return f
}
func (e *environment) sessionRequest(sid string) install.SessionRequest {
	return install.SessionRequest{Credential: credential, Workspace: e.workspace, Session: sid, Root: string(e.root.ArtifactIdentity().Digest()), Dependencies: []string{string(e.dep.ArtifactIdentity().Digest())}}
}
func (e *environment) assertAbsent(t *testing.T, sid string) {
	t.Helper()
	v, err := e.host.InspectGraphSession(context.Background(), e.workspace, sid)
	if err != nil {
		t.Fatal(err)
	}
	if v.Sessions+v.Graphs+v.DataTargets != 0 {
		t.Fatal("partial Session/graph/data registration survived", v)
	}
	i, err := e.repository.InspectInstallation(context.Background(), e.workspace)
	if err != nil {
		t.Fatal(err)
	}
	if i.Artifacts != 2 || i.Grants != 2 || i.Requests != 1 || i.DataTargets != 0 {
		t.Fatal("installation or data references changed", i)
	}
	t.Logf("B013_ATOMIC_CREATION_ABSENCE %+v %+v", v, i)
}
func (e *environment) assertReaped(t *testing.T) {
	t.Helper()
	e.mu.Lock()
	defer e.mu.Unlock()
	alive := map[int]bool{}
	for _, v := range e.executions {
		if v.PID > 0 {
			alive[v.PID] = !v.Reaped
		}
		t.Logf("B013_EXECUTION %+v", v)
	}
	for pid, live := range alive {
		if live || syscall.Kill(pid, 0) != syscall.ESRCH {
			t.Fatalf("owned runner %d not joined/reaped", pid)
		}
	}
}

func TestInstalledGraphLifecycleSevenEffectsAndAuthoritativeResume(t *testing.T) {
	e := setup(t)
	e.publish(t)
	f := e.factory(t, e.host, e.policy)
	request := e.sessionRequest("session")
	s, err := f.Create(context.Background(), request)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { s.Close() })
	v, err := e.host.InspectGraphSession(context.Background(), e.workspace, "session")
	if err != nil || v.Sessions != 1 || v.Graphs != 1 || v.DataTargets != 2 {
		t.Fatal("graph registration missing", v, err)
	}
	empty := checkpoint.Object(map[string]checkpoint.Value{})
	create, err := s.Commands.Execute(context.Background(), s.VM.Token(), hostapi.Command{Callback: "on_session_create", ID: "create", Principal: "operator", ExpectedVersion: 1, Input: empty, Time: 1000})
	if err != nil || create.Version != 2 {
		t.Fatal("lifecycle did not commit", err, create.Version)
	}
	result, err := s.Commands.Execute(context.Background(), s.VM.Token(), hostapi.Command{ID: "command", Principal: "operator", ExpectedVersion: 2, Input: empty, Time: 1000, Random: []int64{7}})
	if err != nil || result.Version != 3 || result.Result.Number != "2" {
		t.Fatal("installed command did not commit", err, result)
	}
	readback, err := e.host.Inspect(context.Background(), s.Binding)
	if err != nil {
		t.Fatal(err)
	}
	if readback.Version != 3 || readback.Documents != 1 || readback.Patches != 1 || readback.Events != 1 || readback.Tasks != 2 || readback.Continuations != 1 || readback.Outbox != 2 || readback.Requests != 2 || readback.Audit < 10 {
		t.Fatal("seven effects missing", readback)
	}
	t.Logf("B013_INSTALLED_SEVEN_EFFECTS %+v", readback)
	duplicate, err := f.Create(context.Background(), request)
	if !errors.Is(err, data.ErrConflict) || duplicate != nil {
		t.Fatal("duplicate creation overwrote live Session", err)
	}
	unchanged, err := e.host.Inspect(context.Background(), s.Binding)
	if err != nil || unchanged.Version != readback.Version || unchanged.Requests != readback.Requests || unchanged.Events != readback.Events {
		t.Fatal("duplicate creation changed authoritative effects", unchanged, err)
	}
	oldToken, pid := s.VM.Token(), s.VM.PID()
	if err = s.Close(); err != nil {
		t.Fatal(err)
	}
	restored, err := f.Resume(context.Background(), request)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { restored.Close() })
	if restored.VM.StateVersion() != 3 || restored.VM.PID() == pid {
		t.Fatal("did not reconstruct from exact authoritative Session")
	}
	if _, err = restored.Commands.Execute(context.Background(), oldToken, hostapi.Command{ID: "old-token", Principal: "operator", ExpectedVersion: 3, Input: empty}); err == nil {
		t.Fatal("old generation Token accepted")
	}
	result, err = restored.Commands.Execute(context.Background(), restored.VM.Token(), hostapi.Command{ID: "resumed", Principal: "operator", ExpectedVersion: 3, Input: empty, Time: 1000, Random: []int64{7}})
	if err != nil || result.Version != 4 || result.Result.Number != "3" {
		t.Fatal("reconstructed installed graph failed", result, err)
	}
	if _, err = e.installer(t).Install(context.Background(), e.installRequest(t, "fresh-install")); !errors.Is(err, store.ErrMigration) {
		t.Fatal("affected Session data did not block fresh install", err)
	}
	i, err := e.repository.InspectInstallation(context.Background(), e.workspace)
	if err != nil || i.Artifacts != 2 || i.Grants != 2 || i.Requests != 1 || i.DataTargets != 2 {
		t.Fatal("failed fresh install changed published metadata", i, err)
	}
	if err = restored.Close(); err != nil {
		t.Fatal(err)
	}
	e.assertReaped(t)
}

func TestInstalledSessionDeniesCredentialTenantGraphAndPolicySubstitution(t *testing.T) {
	e := setup(t)
	e.publish(t)
	f := e.factory(t, e.host, e.policy)
	for _, name := range []string{"credential", "ungranted-principal", "tenant", "missing", "duplicate", "substituted", "policy-context", "policy-schema", "policy-input"} {
		t.Run(name, func(t *testing.T) {
			request := e.sessionRequest(name)
			factory := f
			switch name {
			case "credential":
				request.Credential = store.Credential("wrong-credential")
			case "ungranted-principal":
				request.Credential = other
			case "tenant":
				request.Workspace = "other-workspace"
			case "missing":
				request.Dependencies = nil
			case "duplicate":
				request.Dependencies = append(request.Dependencies, request.Dependencies[0])
			case "substituted":
				request.Dependencies = []string{request.Root}
			default:
				config, err := helper.Config(e.root, e.dep, e.runtime())
				if err != nil {
					t.Fatal(err)
				}
				id := string(e.root.ArtifactIdentity().Digest())
				a := config.Artifacts[id]
				if name == "policy-context" {
					config.Context = "development"
				}
				if name == "policy-schema" {
					a.Host.State.Digest = checkpoint.Hash([]byte("wrong"))
				}
				if name == "policy-input" {
					a.Tests[1].Host.Time++
				}
				config.Artifacts[id] = a
				p, err := install.NewPolicy(config)
				if err != nil {
					t.Fatal(err)
				}
				factory = e.factory(t, e.host, p)
			}
			s, err := factory.Create(context.Background(), request)
			if err == nil || s != nil {
				if s != nil {
					s.Close()
				}
				t.Fatal("untrusted installed Session created", name)
			}
			e.assertAbsent(t, name)
		})
	}
	e.assertReaped(t)
}

func TestLiveSQLCreationAbortRollsBackSessionGraphAndEveryDataReference(t *testing.T) {
	for _, point := range []string{"graph-workspace-locked", "graph-after-session", "graph-after-data-target", "graph-before-commit"} {
		t.Run(point, func(t *testing.T) {
			e := setup(t)
			e.publish(t)
			repo, err := e.host.WithGraphTransactionAbort(point)
			if err != nil {
				t.Fatal(err)
			}
			s, err := e.factory(t, repo, e.policy).Create(context.Background(), e.sessionRequest("aborted"))
			if err == nil || s != nil {
				t.Fatal("actual SQL abort created Session")
			}
			e.assertAbsent(t, "aborted")
			e.assertReaped(t)
		})
	}
}
func TestLiveVMPreparationFailureCreatesNoSessionOrDataReference(t *testing.T) {
	e := setup(t)
	e.publish(t)
	e.vmFault = func(v install.Execution) error {
		if v.Case == "session-vm-ready" {
			return errors.New("fixture VM preparation audit denied")
		}
		return nil
	}
	s, err := e.factory(t, e.host, e.policy).Create(context.Background(), e.sessionRequest("vm-denied"))
	if err == nil || s != nil {
		t.Fatal("failed configured VM was provisioned")
	}
	e.assertAbsent(t, "vm-denied")
	e.assertReaped(t)
}
func TestMandatoryVMAuditFailureCreatesNoSessionOrDataReference(t *testing.T) {
	e := setup(t)
	e.publish(t)
	f, err := install.NewSessionFactory(install.SessionOptions{Reader: e.reader, Policy: e.policy, Repository: e.host, Runtime: e.runtime(), Execution: e.observe, Audit: func(data.Audit) error { return errors.New("fixture mandatory audit denied") }, Validate: func(context.Context, data.Commit) error { return nil }})
	if err != nil {
		t.Fatal(err)
	}
	s, err := f.Create(context.Background(), e.sessionRequest("audit-denied"))
	if err == nil || s != nil {
		t.Fatal("mandatory VM audit denial did not abort initialization")
	}
	e.assertAbsent(t, "audit-denied")
	e.assertReaped(t)
}
func TestWorkspaceLockSerializesSessionCreationAndFreshInstall(t *testing.T) {
	e := setup(t)
	e.publish(t)
	locked, publishing, release := make(chan struct{}), make(chan struct{}), make(chan struct{})
	e.hostFault = func(ctx context.Context, point string) error {
		if point == "graph-workspace-locked" {
			close(locked)
			select {
			case <-release:
				return nil
			case <-ctx.Done():
				return ctx.Err()
			}
		}
		return nil
	}
	e.installFault = func(ctx context.Context, point string) error {
		if point == "before-transaction" {
			close(publishing)
		}
		return nil
	}
	ctx, cancel := context.WithTimeout(context.Background(), 15*time.Second)
	defer cancel()
	type creation struct {
		s   *install.InstalledSession
		err error
	}
	created := make(chan creation, 1)
	installed := make(chan error, 1)
	factory := e.factory(t, e.host, e.policy)
	installer := e.installer(t)
	request := e.installRequest(t, "racing-install")
	go func() { s, err := factory.Create(ctx, e.sessionRequest("racing-session")); created <- creation{s, err} }()
	select {
	case <-locked:
	case <-ctx.Done():
		t.Fatal("creation did not take real workspace lock")
	}
	go func() { _, err := installer.Install(ctx, request); installed <- err }()
	select {
	case <-publishing:
	case <-ctx.Done():
		close(release)
		t.Fatal("install did not reach actual publication transaction")
	}
	close(release)
	v := <-created
	if v.err != nil {
		t.Fatal(v.err)
	}
	t.Cleanup(func() { v.s.Close() })
	err := <-installed
	if !errors.Is(err, store.ErrMigration) {
		t.Fatal("racing publish skipped affected data recheck", err)
	}
	if err = v.s.Close(); err != nil {
		t.Fatal(err)
	}
	i, err := e.repository.InspectInstallation(context.Background(), e.workspace)
	if err != nil || i.Artifacts != 2 || i.Grants != 2 || i.Requests != 1 || i.DataTargets != 2 {
		t.Fatal("racing fresh install changed metadata", i, err)
	}
	t.Logf("B013_ACTUAL_WORKSPACE_LOCK_RACE_REJECTED %+v", i)
	e.assertReaped(t)
}
func TestInstalledObjectLossRejectsBeforeSessionPublication(t *testing.T) {
	e := setup(t)
	e.publish(t)
	rows, err := os.ReadDir(e.storage)
	if err != nil || len(rows) == 0 {
		t.Fatal(err)
	}
	if err = os.Remove(filepath.Join(e.storage, rows[0].Name())); err != nil {
		t.Fatal(err)
	}
	s, err := e.factory(t, e.host, e.policy).Create(context.Background(), e.sessionRequest("object-loss"))
	if err == nil || s != nil {
		t.Fatal("lost installed immutable object accepted")
	}
	e.assertAbsent(t, "object-loss")
	e.assertReaped(t)
}
