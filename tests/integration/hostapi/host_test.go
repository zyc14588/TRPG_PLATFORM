//go:build integration

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package hostapi_test

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"sync/atomic"
	"testing"
	"time"

	host "github.com/zyc14588/TRPG_PLATFORM/internal/hostapi"
	fixture "github.com/zyc14588/TRPG_PLATFORM/internal/hostapi/hostapitest"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/vm"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/capability"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/postgres"
)

var dsn, runner string
var seq atomic.Uint64
var runID string

func TestMain(m *testing.M) {
	dsn = os.Getenv("TRPG_HOSTAPI_PG_DSN")
	if dsn == "" {
		fmt.Fprintln(os.Stderr, "required isolated PostgreSQL fixture not configured; integration NOT_RUN")
		os.Exit(1)
	}
	dir, e := os.MkdirTemp("", "b004-pg-runner-")
	if e != nil {
		panic(e)
	}
	root, e := filepath.Abs("../../..")
	if e != nil {
		panic(e)
	}
	runner, e = fixture.BuildRunner(root, dir)
	if e != nil {
		panic(e)
	}
	b, _ := os.ReadFile(runner)
	fmt.Printf("HOST_PG_RUNNER_IDENTITY %s %s %s\n", checkpoint.Hash(b), profile.ID, profile.RuntimeVersion)
	r, e := postgres.OpenHostRepository(context.Background(), dsn, nil)
	if e != nil {
		panic(e)
	}
	if e = r.Bootstrap(context.Background()); e != nil {
		panic(e)
	}
	r.Close()
	runID = fmt.Sprintf("pg-%d", time.Now().UnixNano())
	code := m.Run()
	os.RemoveAll(dir)
	os.Exit(code)
}
func setup(t *testing.T, source string, level capability.TrustLevel, abortPoint string, fault func(context.Context, string) error, change func(*host.Options)) (*host.Service, *vm.Session, *postgres.HostRepository, host.Options) {
	t.Helper()
	pkg, e := fixture.Package("", source, nil)
	if e != nil {
		t.Fatal(e)
	}
	sid := fmt.Sprintf("%s-%d", runID, seq.Add(1))
	s, e := fixture.Runtime(context.Background(), runner, sid, pkg, level)
	if e != nil {
		t.Fatal(e)
	}
	t.Cleanup(func() { s.Destroy() })
	base, e := postgres.OpenHostRepository(context.Background(), dsn, fault)
	if e != nil {
		t.Fatal(e)
	}
	t.Cleanup(func() { base.Close() })
	r := base
	if abortPoint != "" {
		r, e = base.WithTransactionAbort(abortPoint)
		if e != nil {
			t.Fatal(e)
		}
	}
	o, e := fixture.Options(s, pkg, r, "b004-owned-workspace")
	if e != nil {
		t.Fatal(e)
	}
	if e = r.ProvisionSession(context.Background(), o.Binding, 1, o.StateSchema.Digest(), fixture.State()); e != nil {
		t.Fatal(e)
	}
	if change != nil {
		change(&o)
	}
	service, e := host.New(o)
	if e != nil {
		t.Fatal(e)
	}
	return service, s, r, o
}
func inspect(t *testing.T, r *postgres.HostRepository, b data.Binding) postgres.HostInspection {
	t.Helper()
	x, e := r.Inspect(context.Background(), b)
	if e != nil {
		t.Fatal(e)
	}
	return x
}
func unchanged(t *testing.T, r *postgres.HostRepository, o host.Options, result data.Receipt, err error) {
	t.Helper()
	if err == nil || result.Version != 0 {
		t.Fatal("failure returned result", err, result)
	}
	x := inspect(t, r, o.Binding)
	if x.Version != 1 || x.State.Table["counter"].Number != "1" || x.Documents+x.Quantities+x.Patches+x.Events+x.Requests+x.Tasks+x.Continuations+x.Outbox+x.Audit != 0 {
		t.Fatal("effects survived failed live PostgreSQL transaction", x)
	}
	b, _ := json.Marshal(x)
	t.Logf("PG_ATOMIC_ROLLBACK %s", b)
}
func TestSevenEffectsCommitReadYourWritesAndRetry(t *testing.T) {
	service, s, r, o := setup(t, fixture.Source(""), capability.TrustPrivateUnverified, "", nil, nil)
	command := fixture.Command("atomic")
	result, e := service.Execute(context.Background(), s.Token(), command)
	if e != nil {
		t.Fatal(e)
	}
	x := inspect(t, r, o.Binding)
	if result.Version != 2 || x.Version != 2 || x.Documents != 1 || x.Patches != 1 || x.Events != 1 || x.Requests != 1 || x.Tasks != 2 || x.Continuations != 1 || x.Outbox != 2 || x.Audit < 10 {
		t.Fatal("missing committed effect", x)
	}
	again, e := service.Execute(context.Background(), s.Token(), command)
	if e != nil || !equalJSON(result, again) || !equalJSON(x, inspect(t, r, o.Binding)) {
		t.Fatal("duplicate reran Lua or effects", again, e)
	}
	command.Principal = "another-principal"
	if _, e = service.Execute(context.Background(), s.Token(), command); !errors.Is(e, data.ErrConflict) {
		t.Fatal("principal changed on retry", e)
	}
	b, _ := json.Marshal(x)
	t.Logf("PG_ATOMIC_COMMIT %s", b)
}

func TestOriginalOutputBudgetAgainstPostgres(t *testing.T) {
	for _, c := range []struct{ name, tail string }{
		{"raw-print", `print(string.rep("a",2000))`},
		{"print-plus-result", `print(string.rep("a",1010)); if true then return 1234567890123456789 end`},
		{"raw-host-log", `host.log.write(string.rep("a",2000))`},
		{"log-plus-print", `host.log.write(string.rep("a",600)); print(string.rep("b",600))`},
	} {
		t.Run(c.name, func(t *testing.T) {
			service, s, r, o := setup(t, fixture.Source(c.tail), capability.TrustPrivateUnverified, "", nil, func(o *host.Options) { o.Budget.OutputBytes = 1024 })
			pid := s.PID()
			result, e := service.Execute(context.Background(), s.Token(), fixture.Command("pg-output-budget"))
			unchanged(t, r, o, result, e)
			if profile.Code(e) != profile.ErrBudget {
				t.Fatal("raw output budget escaped", e)
			}
			if _, e := os.Stat(fmt.Sprintf("/proc/%d", pid)); !os.IsNotExist(e) {
				t.Fatal("over-budget runner not reaped", e)
			}
		})
	}
}
func equalJSON(a, b any) bool {
	x, _ := json.Marshal(a)
	y, _ := json.Marshal(b)
	return string(x) == string(y)
}
func TestLiveSQLAbortRollsBackEveryEffect(t *testing.T) {
	for _, point := range []string{"after-state", "after-package-data", "after-events", "after-tasks", "after-continuations", "after-outbox", "after-idempotency", "after-audit", "before-commit"} {
		t.Run(point, func(t *testing.T) {
			service, s, r, o := setup(t, fixture.Source(""), capability.TrustPrivateUnverified, point, nil, nil)
			result, e := service.Execute(context.Background(), s.Token(), fixture.Command("sql-abort"))
			unchanged(t, r, o, result, e)
			if !strings.Contains(e.Error(), "22012") {
				t.Fatal("did not execute actual PostgreSQL division-by-zero", e)
			}
			if _, e = s.Invoke(context.Background(), s.Token(), o.Binding.Session, "command", nil, func(context.Context, profile.HostCall) (checkpoint.Value, error) { return checkpoint.Int(0), nil }); profile.Code(e) != profile.ErrPoisoned {
				t.Fatal("database failure did not poison worker", e)
			}
		})
	}
}
func TestAcknowledgementLossResolvesSingleCommittedRequest(t *testing.T) {
	service, s, r, o := setup(t, fixture.Source(""), capability.TrustPrivateUnverified, "", func(_ context.Context, p string) error {
		if p == "after-commit" {
			return errors.New("simulated lost acknowledgement after actual COMMIT")
		}
		return nil
	}, nil)
	command := fixture.Command("ack-loss")
	result, e := service.Execute(context.Background(), s.Token(), command)
	if e != nil || result.Version != 2 {
		t.Fatal(result, e)
	}
	before := inspect(t, r, o.Binding)
	again, e := service.Execute(context.Background(), s.Token(), command)
	if e != nil || !equalJSON(result, again) || !equalJSON(before, inspect(t, r, o.Binding)) {
		t.Fatal("ack resolution duplicated an effect", again, e)
	}
}
func TestDatabaseTierAndScopeDenyMatrix(t *testing.T) {
	for _, c := range []struct{ name, tail string }{{"private-named", `pcall(function() host.db.named("increase",{key="one",delta=1}) end)`}, {"raw-sql", `pcall(function() host.db.named("SELECT * FROM core",{}) end)`}, {"ddl", `pcall(function() host.db.named("CREATE TABLE x",{}) end)`}, {"cross-package", `pcall(function() host.db.get("example.other/records","one") end)`}, {"cross-workspace", `pcall(function() host.db.get("another-workspace/docs","one") end)`}, {"core-table", `pcall(function() host.db.get("host_command.sessions","one") end)`}, {"undeclared-index", `pcall(function() host.db.list("docs","unknown",1,1) end)`}, {"schema", `pcall(function() host.db.put("docs","wrong",{score="text"}) end)`}} {
		t.Run(c.name, func(t *testing.T) {
			service, s, r, o := setup(t, fixture.Source(c.tail), capability.TrustPrivateUnverified, "", nil, nil)
			result, e := service.Execute(context.Background(), s.Token(), fixture.Command("denied"))
			unchanged(t, r, o, result, e)
		})
	}
}
func TestReviewedNamedRelationalPlansAndTableAudit(t *testing.T) {
	for _, level := range []capability.TrustLevel{capability.TrustOfficial, capability.TrustSigned} {
		t.Run(string(level), func(t *testing.T) {
			var audits []data.Audit
			source := fixture.Source(`assert(host.db.named("increase",{key="qty",delta=3})==3);assert(host.db.named("read",{key="qty",delta=0})==3)`)
			service, s, r, o := setup(t, source, level, "", nil, func(o *host.Options) { o.Audit = func(a data.Audit) error { audits = append(audits, a); return nil } })
			if _, e := service.Execute(context.Background(), s.Token(), fixture.Command("named")); e != nil {
				t.Fatal(e)
			}
			if inspect(t, r, o.Binding).Quantities != 1 {
				t.Fatal("named relational change absent")
			}
			seen := 0
			for _, a := range audits {
				if a.Operation == "host.db.named" {
					seen++
					if len(a.Tables) != 1 || a.Tables[0] != fixture.PackageID+"/quantity" {
						t.Fatal("table audit missing", a)
					}
				}
			}
			if seen != 2 {
				t.Fatal("named operation not audited")
			}
		})
	}
}
func TestActiveCallbackCancellationAgainstPostgres(t *testing.T) {
	entered := make(chan struct{})
	var once sync.Once
	service, s, r, o := setup(t, fixture.Source(""), capability.TrustPrivateUnverified, "", nil, func(o *host.Options) {
		o.Fault = func(ctx context.Context, p string) error {
			if p == "callback:host.log.write" {
				once.Do(func() { close(entered) })
				<-ctx.Done()
				return ctx.Err()
			}
			return nil
		}
	})
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	pid := s.PID()
	finished := make(chan struct {
		result data.Receipt
		err    error
	}, 1)
	go func() {
		result, e := service.Execute(ctx, s.Token(), fixture.Command("cancel"))
		finished <- struct {
			result data.Receipt
			err    error
		}{result, e}
	}()
	select {
	case <-entered:
	case <-time.After(time.Second):
		t.Fatal("active final callback not reached")
	}
	cancel()
	select {
	case out := <-finished:
		unchanged(t, r, o, out.result, out.err)
	case <-time.After(time.Second):
		t.Fatal("callback or process not joined")
	}
	if _, e := os.Stat(fmt.Sprintf("/proc/%d", pid)); !os.IsNotExist(e) {
		t.Fatal("runner not reaped", e)
	}
}
func TestRunnerLossRollsBackStagedPostgresEffects(t *testing.T) {
	var session *vm.Session
	service, s, r, o := setup(t, fixture.Source(""), capability.TrustPrivateUnverified, "", nil, func(o *host.Options) {
		session = o.Session
		pid := session.PID()
		o.Fault = func(_ context.Context, p string) error {
			if p == "callback:host.log.write" {
				process, e := os.FindProcess(pid)
				if e != nil {
					return e
				}
				return process.Kill()
			}
			return nil
		}
	})
	result, e := service.Execute(context.Background(), s.Token(), fixture.Command("worker-loss"))
	unchanged(t, r, o, result, e)
}
func TestExplicitTenantAndSessionStorageBoundary(t *testing.T) {
	service, s, r, o := setup(t, fixture.Source(""), capability.TrustPrivateUnverified, "", nil, nil)
	result, e := service.Execute(context.Background(), s.Token(), fixture.Command("owned"))
	if e != nil {
		t.Fatal(e)
	}
	header := result.Header
	header.Binding.Workspace = "unprovisioned-workspace"
	if tx, e := r.Begin(context.Background(), header); e == nil {
		tx.Rollback()
		t.Fatal("cross-workspace request found owned row")
	}
	header = result.Header
	header.Binding.Session = "another-session"
	if tx, e := r.Begin(context.Background(), header); e == nil {
		tx.Rollback()
		t.Fatal("cross-session request found owned row")
	}
	header = result.Header
	header.Binding.GraphHash = "sha256:" + strings.Repeat("a", 64)
	if tx, e := r.Begin(context.Background(), header); e == nil {
		tx.Rollback()
		t.Fatal("wrong graph found owned row")
	}
	if inspect(t, r, o.Binding).Version != 2 {
		t.Fatal("negative probe changed owner state")
	}
}
