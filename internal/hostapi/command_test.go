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
)

var runner string
var sequence atomic.Uint64

func TestMain(m *testing.M) {
	dir, e := os.MkdirTemp("", "b004-unit-runner-")
	if e != nil {
		panic(e)
	}
	root, e := filepath.Abs("../..")
	if e != nil {
		panic(e)
	}
	runner, e = fixture.BuildRunner(root, dir)
	if e != nil {
		panic(e)
	}
	b, _ := os.ReadFile(runner)
	fmt.Printf("HOST_RUNNER_IDENTITY %s %s %s\n", checkpoint.Hash(b), profile.ID, profile.RuntimeVersion)
	code := m.Run()
	os.RemoveAll(dir)
	os.Exit(code)
}
func copyValue[T any](v T) T { b, _ := json.Marshal(v); var out T; json.Unmarshal(b, &out); return out }

type memory struct {
	mu       sync.Mutex
	snapshot data.Snapshot
	commits  []data.Commit
	receipts map[string]data.Receipt
	fault    string
}
type transaction struct {
	m        *memory
	header   data.Header
	snapshot data.Snapshot
	closed   bool
}

func (m *memory) Begin(ctx context.Context, h data.Header) (data.Transaction, error) {
	m.mu.Lock()
	if h.Binding != m.snapshot.Binding {
		m.mu.Unlock()
		return nil, data.ErrDenied
	}
	s := copyValue(m.snapshot)
	if r, ok := m.receipts[h.CommandID]; ok {
		if r.Header != h {
			m.mu.Unlock()
			return nil, data.ErrConflict
		}
		s.Existing = &r
	} else if h.ExpectedVersion != s.Version {
		m.mu.Unlock()
		return nil, data.ErrConflict
	}
	return &transaction{m: m, header: h, snapshot: s}, nil
}
func (t *transaction) Snapshot() data.Snapshot { return copyValue(t.snapshot) }
func (t *transaction) Rollback() error {
	if !t.closed {
		t.closed = true
		t.m.mu.Unlock()
	}
	return nil
}
func (t *transaction) Commit(ctx context.Context, c data.Commit) (data.Receipt, error) {
	if ctx.Err() != nil {
		return data.Receipt{}, ctx.Err()
	}
	if t.m.fault != "" {
		return data.Receipt{}, errors.New(t.m.fault)
	}
	r := data.Receipt{Header: c.Header, Version: c.Header.ExpectedVersion + 1, Result: c.Result, Inputs: c.Inputs, Events: c.Events}
	t.m.snapshot.Version = r.Version
	t.m.snapshot.State = copyValue(c.State)
	t.m.snapshot.Rows = copyValue(c.Rows)
	t.m.snapshot.Quantities = copyValue(c.Quantities)
	t.m.commits = append(t.m.commits, copyValue(c))
	t.m.receipts[c.Header.CommandID] = copyValue(r)
	t.Rollback()
	return copyValue(r), nil
}
func setup(t *testing.T, source string, change func(*host.Options)) (*host.Service, *vm.Session, *memory, host.Options) {
	t.Helper()
	pkg, e := fixture.Package("", source, nil)
	if e != nil {
		t.Fatal(e)
	}
	s, e := fixture.Runtime(context.Background(), runner, fmt.Sprintf("unit-%d", sequence.Add(1)), pkg, capability.TrustPrivateUnverified)
	if e != nil {
		t.Fatal(e)
	}
	t.Cleanup(func() { s.Destroy() })
	m := &memory{receipts: map[string]data.Receipt{}}
	o, e := fixture.Options(s, pkg, m, "unit-workspace")
	if e != nil {
		t.Fatal(e)
	}
	m.snapshot = data.Snapshot{Binding: o.Binding, Version: 1, State: fixture.State(), SchemaHash: o.StateSchema.Digest()}
	if change != nil {
		change(&o)
	}
	service, e := host.New(o)
	if e != nil {
		t.Fatal(e)
	}
	return service, s, m, o
}
func assertRollback(t *testing.T, m *memory, result data.Receipt, err error) {
	t.Helper()
	if err == nil || result.Version != 0 {
		t.Fatal("failed command returned a visible result", result, err)
	}
	m.mu.Lock()
	defer m.mu.Unlock()
	if m.snapshot.Version != 1 || m.snapshot.State.Table["counter"].Number != "1" || len(m.commits) != 0 || len(m.receipts) != 0 || len(m.snapshot.Rows) != 0 || len(m.snapshot.Quantities) != 0 {
		t.Fatal("an effect escaped rollback", m.snapshot, m.commits)
	}
}
func assertPoisoned(t *testing.T, s *vm.Session) {
	t.Helper()
	_, e := s.Invoke(context.Background(), s.Token(), s.SessionID(), "command", []checkpoint.Value{checkpoint.Object(nil)}, func(context.Context, profile.HostCall) (checkpoint.Value, error) { return checkpoint.Int(0), nil })
	if profile.Code(e) != profile.ErrPoisoned {
		t.Fatal("failed workspace did not poison VM", e)
	}
}
func TestSevenEffectsAtomicCommitAndIdempotency(t *testing.T) {
	var audits []data.Audit
	service, s, m, o := setup(t, fixture.Source(""), func(o *host.Options) { o.Audit = func(a data.Audit) error { audits = append(audits, a); return nil } })
	command := fixture.Command("atomic")
	r, e := service.Execute(context.Background(), s.Token(), command)
	if e != nil {
		t.Fatal(e)
	}
	if r.Version != 2 || r.Result.Number != "2" || r.Inputs.Time != 1000 || len(r.Inputs.Random) != 1 || r.Inputs.Random[0] != 7 {
		t.Fatal(r)
	}
	c := m.commits[0]
	if len(c.Patches) != 1 || len(c.Rows) != 2 || len(c.Events) != 1 || len(c.Tasks) != 2 || len(c.Continuations) != 1 || len(c.Outbox) != 2 || len(c.Audit) < 10 {
		t.Fatal("seven-effect transaction incomplete", c)
	}
	if c.Patches[0].EventID != c.Events[0].ID || c.Patches[0].Module != "lua/main.lua" || c.Patches[0].Line < 1 {
		t.Fatal("unbound patch", c.Patches[0])
	}
	for _, a := range audits {
		b, _ := json.Marshal(a)
		if strings.Contains(string(b), "sensitive-fixture-value") || strings.Contains(string(b), "execution-token") {
			t.Fatal("secret in audit", string(b))
		}
	}
	again, e := service.Execute(context.Background(), s.Token(), command)
	if e != nil || !equalJSON(r, again) || len(m.commits) != 1 {
		t.Fatal("idempotency reran effects", again, e)
	}
	command.Input.Table["secret"] = checkpoint.Text("different")
	if _, e = service.Execute(context.Background(), s.Token(), command); !errors.Is(e, data.ErrConflict) {
		t.Fatal("changed input reused request", e)
	}
	// Reconstruction preserves authority and rotates an old execution token.
	old := s.Token()
	pid := s.PID()
	if e = s.Reconstruct(context.Background(), vm.State{Version: 2, Value: c.State}, nil); e != nil {
		t.Fatal(e)
	}
	if s.PID() == pid {
		t.Fatal("worker reused")
	}
	if e = s.AuthorizeToken(old, o.Binding.Session); profile.Code(e) != profile.ErrCapability {
		t.Fatal("old generation retained authority", e)
	}
}
func equalJSON(a, b any) bool {
	x, _ := json.Marshal(a)
	y, _ := json.Marshal(b)
	return string(x) == string(y)
}
func TestEveryStagedEffectRollsBackOnFailure(t *testing.T) {
	cases := []struct {
		name, tail string
		change     func(*host.Options)
		storage    bool
	}{
		{"script", `error("script failure")`, nil, false},
		{"caught-private-named", `pcall(function() host.db.named("increase",{key="one",delta=1}) end)`, nil, false},
		{"caught-unknown-schema", `pcall(function() host.db.put("docs","invalid",{score="wrong"}) end)`, nil, false},
		{"caught-cross-package", `pcall(function() host.db.get("example.other/data","one") end)`, nil, false},
		{"caught-undeclared-index", `pcall(function() host.db.list("docs","unknown",1,1) end)`, nil, false},
		{"caught-raw-sql", `pcall(function() host.db.named("SELECT * FROM core",{}) end)`, nil, false},
		{"caught-runtime-ddl", `pcall(function() host.db.named("CREATE TABLE x",{}) end)`, nil, false},
		{"state-schema", `host.state.put({"counter"},-1)`, nil, false},
		{"result-schema", "if true then return 'wrong result' end", nil, false},
		{"instruction", `while true do end`, nil, false},
		{"memory", `local allocated=string.rep("x",256*1024*1024)`, nil, false},
		{"recursion", `local function recurse() return 1+recurse() end;recurse()`, nil, false},
		{"output", `print(string.rep("a",65537))`, nil, false},
		{"go-validation", "", func(o *host.Options) {
			o.Validate = func(context.Context, data.Commit) error { return errors.New("denied") }
		}, false},
		{"go-fault-after-lua", "", func(o *host.Options) {
			o.Fault = func(_ context.Context, p string) error {
				if p == "after-lua" {
					return errors.New("injected")
				}
				return nil
			}
		}, false},
		{"audit", "", func(o *host.Options) {
			o.Audit = func(a data.Audit) error {
				if a.Operation == "command-commit" {
					return errors.New("audit sink unavailable")
				}
				return nil
			}
		}, false},
		{"database", "", nil, true},
	}
	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			service, s, m, _ := setup(t, fixture.Source(c.tail), c.change)
			if c.storage {
				m.fault = "database unavailable"
			}
			r, e := service.Execute(context.Background(), s.Token(), fixture.Command("rollback"))
			assertRollback(t, m, r, e)
			assertPoisoned(t, s)
		})
	}
}
func TestIndependentHostBudgets(t *testing.T) {
	cases := []struct {
		name, tail string
		adjust     func(*host.Budget)
	}{
		{"callbacks", "", func(b *host.Budget) { b.Callbacks = 15 }},
		{"patches", `host.state.put({"counter"},3)`, func(b *host.Budget) { b.Patches = 1 }},
		{"rows", "", func(b *host.Budget) { b.Rows = 4 }},
		{"bytes", `host.log.write(string.rep("s",200))`, func(b *host.Budget) { b.DataBytes = 200 }},
		{"events", `host.event.emit("change",{counter=2})`, func(b *host.Budget) { b.Events = 1 }},
		{"tasks", "", func(b *host.Budget) { b.Tasks = 1 }},
		{"continuations", `host.task.continuation(task,{value=3})`, func(b *host.Budget) { b.Continuations = 1 }},
		{"outbox", "", func(b *host.Budget) { b.Outbox = 1 }},
	}
	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			service, s, m, _ := setup(t, fixture.Source(c.tail), func(o *host.Options) {
				c.adjust(&o.Budget)
				for k, n := range o.Namespaces {
					if n.MaxRows > o.Budget.Rows {
						n.MaxRows = o.Budget.Rows
					}
					if n.MaxBytes > o.Budget.DataBytes {
						n.MaxBytes = o.Budget.DataBytes
					}
					o.Namespaces[k] = n
				}
			})
			r, e := service.Execute(context.Background(), s.Token(), fixture.Command("budget"))
			assertRollback(t, m, r, e)
			if profile.Code(e) != profile.ErrBudget {
				t.Fatal("expected hard budget", e)
			}
			assertPoisoned(t, s)
		})
	}
}

// ACC-M1-B004-001: all original output shares the command's lower bound;
// hashing a log or print must never make an oversized command committable.
func TestOriginalOutputBudgetRollsBackEveryEffect(t *testing.T) {
	for _, c := range []struct {
		name, tail string
		limit      int
	}{
		{"raw-print", `print(string.rep("a",2000))`, 1024},
		{"print-plus-result", `print(string.rep("a",1010)); if true then return 1234567890123456789 end`, 1024},
		{"raw-warn", `warn(string.rep("a",2000))`, 1024},
		{"raw-host-log", `host.log.write(string.rep("a",2000))`, 1024},
		{"log-plus-print", `host.log.write(string.rep("a",600)); print(string.rep("b",600))`, 1024},
		{"cumulative-logs", `host.log.write(string.rep("a",600)); host.log.write(string.rep("b",600))`, 1024},
		{"default-log-ceiling", `host.log.write(string.rep("a",65536))`, 65536},
		{"caught-log-excess", `pcall(function() host.log.write(string.rep("a",2000)) end)`, 1024},
	} {
		t.Run(c.name, func(t *testing.T) {
			service, s, m, _ := setup(t, fixture.Source(c.tail), func(o *host.Options) { o.Budget.OutputBytes = c.limit })
			r, e := service.Execute(context.Background(), s.Token(), fixture.Command("output-budget"))
			assertRollback(t, m, r, e)
			if profile.Code(e) != profile.ErrBudget {
				t.Fatal("original output budget bypass", e)
			}
			assertPoisoned(t, s)
		})
	}
	// Original bytes below the lowered total limit remain valid; large fixed
	// digest strings must not overcharge an otherwise small print.
	service, s, m, _ := setup(t, fixture.Source(`print("ok")`), func(o *host.Options) { o.Budget.OutputBytes = 32 })
	r, e := service.Execute(context.Background(), s.Token(), fixture.Command("output-within-limit"))
	if e != nil || r.Version != 2 || len(m.commits) != 1 {
		t.Fatal("bounded redacted output rejected", e, r)
	}
}
func TestActiveCallbackCancellationRollsBackAndReaps(t *testing.T) {
	entered := make(chan struct{})
	var once sync.Once
	service, s, m, _ := setup(t, fixture.Source(""), func(o *host.Options) {
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
		r data.Receipt
		e error
	}, 1)
	go func() {
		r, e := service.Execute(ctx, s.Token(), fixture.Command("cancel"))
		finished <- struct {
			r data.Receipt
			e error
		}{r, e}
	}()
	select {
	case <-entered:
	case <-time.After(time.Second):
		t.Fatal("Lua never reached final active callback after staging all effects")
	}
	cancel()
	select {
	case out := <-finished:
		assertRollback(t, m, out.r, out.e)
	case <-time.After(time.Second):
		t.Fatal("cancel left a callback or worker alive")
	}
	if _, e := os.Stat(fmt.Sprintf("/proc/%d", pid)); !os.IsNotExist(e) {
		t.Fatal("runner not reaped", e)
	}
	assertPoisoned(t, s)
}
func TestValidationCannotStageEffectsAndTokensDoNotCrossSessions(t *testing.T) {
	source := strings.Replace(fixture.Source(""), "return not command.reject", "host.state.put({'counter'},4);return true", 1)
	service, s, m, _ := setup(t, source, nil)
	r, e := service.Execute(context.Background(), s.Token(), fixture.Command("validate"))
	assertRollback(t, m, r, e)
	assertPoisoned(t, s)
	service2, other, m2, _ := setup(t, fixture.Source(""), nil)
	r, e = service2.Execute(context.Background(), s.Token(), fixture.Command("token"))
	assertRollback(t, m2, r, e)
	if _, e = other.Invoke(context.Background(), other.Token(), "wrong-session", "command", nil, func(context.Context, profile.HostCall) (checkpoint.Value, error) {
		t.Fatal("wrong SID reached dispatcher")
		return checkpoint.Int(0), nil
	}); profile.Code(e) != profile.ErrCapability {
		t.Fatal(e)
	}
}
func TestAuditControlsCannotDisableOrExposeSecrets(t *testing.T) {
	for _, level := range []string{"OFF", "AUDIT-2", "AUDIT-3"} {
		t.Run(level, func(t *testing.T) {
			_, s, _, o := setup(t, fixture.Source(""), nil)
			o.Session = s
			o.AuditPolicy = host.AuditPolicy{Level: level}
			if _, e := host.New(o); e == nil {
				t.Fatal("invalid production audit policy accepted")
			}
		})
	}
	for _, level := range []string{"AUDIT-0", "AUDIT-1", "AUDIT-2", "AUDIT-3"} {
		t.Run("accepted-"+level, func(t *testing.T) {
			var logs []data.Audit
			service, s, _, _ := setup(t, fixture.Source(""), func(o *host.Options) {
				o.AuditPolicy = host.AuditPolicy{Level: level, Development: level == "AUDIT-3", AuthorizedUntil: time.Now().Add(time.Minute)}
				o.Audit = func(a data.Audit) error { logs = append(logs, a); return nil }
			})
			if _, e := service.Execute(context.Background(), s.Token(), fixture.Command("audit")); e != nil {
				t.Fatal(e)
			}
			b, _ := json.Marshal(logs)
			if len(logs) < 10 || strings.Contains(string(b), "sensitive-fixture-value") {
				t.Fatal("audit lost baseline or leaked raw secret")
			}
		})
	}
}
func TestConcurrentSessionsKeepWorkspacesAndTokensSeparate(t *testing.T) {
	const n = 6
	var wg sync.WaitGroup
	errorsOut := make(chan error, n)
	for i := 0; i < n; i++ {
		service, s, m, _ := setup(t, fixture.Source(""), nil)
		wg.Add(1)
		go func() {
			defer wg.Done()
			r, e := service.Execute(context.Background(), s.Token(), fixture.Command("same-command"))
			if e != nil {
				errorsOut <- e
				return
			}
			if r.Version != 2 || m.snapshot.State.Table["counter"].Number != "2" {
				errorsOut <- fmt.Errorf("cross-session state")
			}
		}()
	}
	wg.Wait()
	close(errorsOut)
	for e := range errorsOut {
		t.Error(e)
	}
}

func TestAuthorityProposalCannotMutateCommittedWorkspace(t *testing.T) {
	service, s, m, _ := setup(t, fixture.Source(""), func(o *host.Options) {
		o.Validate = func(_ context.Context, c data.Commit) error {
			c.State.Table["counter"] = checkpoint.Int(999)
			c.Events[0].Payload.Table["counter"] = checkpoint.Int(999)
			return nil
		}
	})
	r, e := service.Execute(context.Background(), s.Token(), fixture.Command("defensive"))
	if e != nil || r.Result.Number != "2" || m.snapshot.State.Table["counter"].Number != "2" || m.commits[0].Events[0].Payload.Table["counter"].Number != "2" {
		t.Fatal("Go proposal alias modified authority", r, e, m.snapshot)
	}
}
func TestFailedCommandRequiresFreshWorkerAndToken(t *testing.T) {
	service, s, m, _ := setup(t, fixture.Source(`if command.fail then cache=77;error("failed after staging") end;assert(cache==nil)`), nil)
	c := fixture.Command("failure")
	c.Input.Table["fail"] = checkpoint.Bool(true)
	old := s.Token()
	pid := s.PID()
	r, e := service.Execute(context.Background(), old, c)
	assertRollback(t, m, r, e)
	if e = s.Reconstruct(context.Background(), vm.State{Version: 1, Value: fixture.State()}, nil); e != nil {
		t.Fatal(e)
	}
	if s.PID() == pid {
		t.Fatal("failed worker reused")
	}
	c = fixture.Command("fresh")
	r, e = service.Execute(context.Background(), old, c)
	assertRollback(t, m, r, e)
	r, e = service.Execute(context.Background(), s.Token(), c)
	if e != nil || r.Version != 2 {
		t.Fatal("fresh reconstruction did not recover", r, e)
	}
}
func TestActiveCallbackWallDeadlineRollsBack(t *testing.T) {
	service, s, m, _ := setup(t, fixture.Source(""), func(o *host.Options) {
		o.Fault = func(ctx context.Context, p string) error {
			if p == "callback:host.log.write" {
				<-ctx.Done()
				return ctx.Err()
			}
			return nil
		}
	})
	started := time.Now()
	r, e := service.Execute(context.Background(), s.Token(), fixture.Command("deadline"))
	assertRollback(t, m, r, e)
	if time.Since(started) > 2*time.Second {
		t.Fatal("wall budget not enforced")
	}
	assertPoisoned(t, s)
}
func TestLifecycleCallbackUsesSameAtomicWorkspace(t *testing.T) {
	source := strings.Replace(fixture.Source(""), "return M", "M.on_session_start=M.execute_command;return M", 1)
	service, s, m, _ := setup(t, source, nil)
	c := fixture.Command("lifecycle")
	c.Callback = "on_session_start"
	r, e := service.Execute(context.Background(), s.Token(), c)
	if e != nil || r.Version != 2 || len(m.commits) != 1 || len(m.commits[0].Tasks) != 2 || r.Inputs.Callback != "on_session_start" {
		t.Fatal("lifecycle bypassed command transaction", r, e)
	}
}

func TestSchemaAuthorityUsesExactArtifactAndBoundedLocalValidator(t *testing.T) {
	pkg, e := fixture.Package("", fixture.Source(""), nil)
	if e != nil {
		t.Fatal(e)
	}
	file, _ := pkg.Entry("schemas/state.schema.json")
	for _, c := range []struct{ name, path, digest string }{{"missing", "schemas/absent.schema.json", checkpoint.Hash(file.Bytes())}, {"digest", "schemas/state.schema.json", "sha256:" + strings.Repeat("0", 64)}} {
		t.Run(c.name, func(t *testing.T) {
			if _, e := host.BindSchema(pkg, c.path, c.digest, fixture.State()); e == nil {
				t.Fatal("unbound schema accepted")
			}
		})
	}
	for _, c := range []struct{ name, schema string }{{"remote-reference", `{"$schema":"https://json-schema.org/draft/2020-12/schema","$ref":"https://example.invalid/schema"}`}, {"recursive-reference", `{"$schema":"https://json-schema.org/draft/2020-12/schema","$ref":"#"}`}, {"duplicate-json-key", `{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","type":"string"}`}} {
		t.Run(c.name, func(t *testing.T) {
			p, e := fixture.Package("", fixture.Source(""), map[string][]byte{"schemas/state.schema.json": []byte(c.schema)})
			if e != nil {
				t.Fatal(e)
			}
			f, _ := p.Entry("schemas/state.schema.json")
			if _, e := host.BindSchema(p, "schemas/state.schema.json", checkpoint.Hash(f.Bytes()), fixture.State()); e == nil {
				t.Fatal("unsafe/unbounded schema accepted")
			}
		})
	}
}
