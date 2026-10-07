//go:build linux && integration

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package continuation_test

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"net/url"
	"os"
	"os/exec"
	"strings"
	"sync"
	"sync/atomic"
	"testing"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	platformsession "github.com/zyc14588/TRPG_PLATFORM/internal/platform/session"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/postgres"
	"github.com/zyc14588/TRPG_PLATFORM/internal/task"
)

type taskFixture struct {
	*nativeFixture
	tasks           *postgres.PlatformTaskStorage
	worker          task.Worker
	workerAuthority *task.Authority
	completions     *platformsession.Continuations
	policies        []task.Policy
	connection      *platformsession.Connection
	external        atomic.Int64
	mu              sync.Mutex
	faultPoint      string
}

func taskNeed(t *testing.T, e error) {
	t.Helper()
	if e != nil {
		t.Fatalf("task operation failed: %v", task.SafeError(e))
	}
}
func newTaskFixture(t *testing.T, lease, lifetime time.Duration) *taskFixture {
	t.Helper()
	n := newNativeFixture(t, false)
	f := &taskFixture{nativeFixture: n}
	f.connection = n.connect(t, n.owner, 0)
	credential, e := task.NewCredential(bytes.Repeat([]byte{77}, 32))
	taskNeed(t, e)
	f.workerAuthority, e = task.NewAuthority([]task.WorkerGrant{{ID: "owned-worker", Credential: credential, Workspaces: []string{f.w}, Expires: time.Now().Add(time.Hour)}})
	taskNeed(t, e)
	f.worker, e = f.workerAuthority.Authenticate(f.ctx, credential)
	taskNeed(t, e)
	f.tasks, e = postgres.NewPlatformTaskStorage(postgres.PlatformTaskOptions{Repository: f.r, Lease: lease, Lifetime: lifetime, Fault: func(_ context.Context, point string) error {
		f.mu.Lock()
		defer f.mu.Unlock()
		if point == f.faultPoint {
			f.faultPoint = ""
			return errors.New("synthetic private provider diagnostic")
		}
		return nil
	}})
	taskNeed(t, e)
	taskNeed(t, f.tasks.Bootstrap(f.ctx))
	b := f.launched.StorageValue()
	f.policies = []task.Policy{{GraphHash: b.Binding.GraphHash, ConfigurationHash: b.ConfigurationHash, PackageID: PackageID, ValidateInput: f.inputSchema.Validate, ValidateResult: f.resultSchema.Validate, Execute: func(ctx context.Context, _ task.Value) (task.Value, error) {
		f.external.Add(1)
		// An actual separate SQL transaction proves the authoritative row is
		// unlocked while the external handler runs. No Lua/game Tx is waiting.
		if e := externalUnlocked(ctx, b.Binding); e != nil {
			return task.Value{}, e
		}
		return task.NewValue(checkpoint.Int(5))
	}, Inputs: func(ctx context.Context, _ task.Job) (task.Inputs, error) {
		if ctx.Err() != nil {
			return task.Inputs{}, task.ErrUnavailable
		}
		return task.NewInputs(time.Now().UnixMilli(), nil)
	}}}
	f.composeTasks(t)
	return f
}

func externalUnlocked(ctx context.Context, b data.Binding) error {
	u, e := url.Parse(dsn)
	if e != nil || !ownedDatabase(u) || !store.ValidID(b.Workspace) || !store.ValidID(b.Session) {
		return task.ErrDenied
	}
	statement := `BEGIN; SET LOCAL lock_timeout='100ms'; SELECT 1 FROM host_command.sessions WHERE workspace='` + b.Workspace + `' AND session='` + b.Session + `' FOR UPDATE NOWAIT; ROLLBACK;`
	raw, e := exec.CommandContext(ctx, "docker", "exec", os.Getenv("M2B006_CONTAINER_ID"), "psql", "-U", "m2b006", "-d", "m2_b006_fixture", "-v", "ON_ERROR_STOP=1", "-Atq", "-c", statement).Output()
	if e != nil || strings.TrimSpace(string(raw)) != "1" {
		return task.ErrBusy
	}
	return nil
}
func (f *taskFixture) composeTasks(t *testing.T) {
	t.Helper()
	var e error
	f.completions, e = platformsession.NewContinuations(platformsession.ContinuationOptions{Launch: f.service, Storage: f.tasks, Policies: f.policies})
	taskNeed(t, e)
}
func (f *taskFixture) runtime(t *testing.T) *task.Runtime {
	t.Helper()
	r, e := task.NewWorker(task.WorkerOptions{Identity: f.worker, Storage: f.tasks, Policies: f.policies, Post: f.completions.Post, MaxActive: 2, ExternalTimeout: 2 * time.Second, PostTimeout: 3 * time.Second, Poll: 50 * time.Millisecond})
	taskNeed(t, e)
	return r
}
func (f *taskFixture) source(t *testing.T, id string, version uint64) {
	t.Helper()
	_, e := f.connection.Submit(f.ctx, f.envelope(id, "gm", "increment", version))
	need(t, e)
}
func (f *taskFixture) claim(t *testing.T) task.Job {
	t.Helper()
	j, e := f.tasks.Claim(f.ctx, f.worker)
	taskNeed(t, e)
	return j
}
func (f *taskFixture) save(t *testing.T, j task.Job) task.Job {
	t.Helper()
	v, e := task.NewValue(checkpoint.Int(5))
	taskNeed(t, e)
	inputs, e := task.NewInputs(time.Now().UnixMilli(), []int64{7})
	taskNeed(t, e)
	saved, e := f.tasks.SaveResult(f.ctx, f.worker, j, v, inputs)
	taskNeed(t, e)
	return saved
}
func (f *taskFixture) version(t *testing.T) string {
	t.Helper()
	return f.sql(t, `SELECT version FROM host_command.sessions WHERE workspace='`+f.w+`'`)
}
func (f *taskFixture) setFault(p string) { f.mu.Lock(); f.faultPoint = p; f.mu.Unlock() }

func TestCommittedTaskReturnsThroughStandardActorWithOrderedFanout(t *testing.T) {
	f := newTaskFixture(t, 15*time.Second, time.Minute)
	f.source(t, "create", 1)
	r := f.runtime(t)
	for n := 0; n < 2; n++ {
		report, e := r.RunOnce(f.ctx)
		taskNeed(t, e)
		if report.Applied != 1 || report.Executed != 1-n {
			t.Fatal("task phase accounting")
		}
	}
	if f.external.Load() != 1 || f.version(t) != "4" {
		t.Fatal("fanout repeated external operation or lost authority version")
	}
	if f.sql(t, `SELECT count(*) FROM host_command.requests WHERE workspace='`+f.w+`' AND principal='task-system'`) != "2" || f.sql(t, `SELECT count(*) FROM platform_task.jobs WHERE workspace='`+f.w+`' AND status='done'`) != "1" {
		t.Fatal("standard completion ledger or task ack absent")
	}
	if _, e := r.RunOnce(f.ctx); e != task.ErrNotFound {
		t.Fatal("completed task dispatched again")
	}
	frame, e := f.connection.Reconnect(f.ctx, 0)
	need(t, e)
	v := frame.StorageValue()
	if v.View.Table["counter"].Number != "12" {
		t.Fatal("same Actor did not apply both continuations")
	}
}
func TestGameFailureAfterTaskCreationRollsBackEveryIntentBeforeDispatch(t *testing.T) {
	f := newTaskFixture(t, 15*time.Second, time.Minute)
	_, e := f.connection.Submit(f.ctx, f.envelope("rollback", "gm", "fail", 1))
	if e == nil {
		t.Fatal("synthetic post-intent Lua failure accepted")
	}
	if f.version(t) != "1" || f.sql(t, `SELECT (SELECT count(*) FROM host_command.tasks WHERE workspace='`+f.w+`')+(SELECT count(*) FROM host_command.continuations WHERE workspace='`+f.w+`')+(SELECT count(*) FROM host_command.outbox WHERE workspace='`+f.w+`')`) != "0" {
		t.Fatal("authority rollback left intents")
	}
	if _, e = f.runtime(t).RunOnce(f.ctx); e != task.ErrNotFound || f.external.Load() != 0 {
		t.Fatal("uncommitted task dispatched")
	}
	f.source(t, "committed", 1)
	if _, e = f.runtime(t).RunOnce(f.ctx); e != nil {
		t.Fatal("committed task unavailable")
	}
}
func TestDuplicateCompletionBeforeAckReturnsOriginalWithoutNewEffect(t *testing.T) {
	f := newTaskFixture(t, 15*time.Second, time.Minute)
	f.source(t, "create", 1)
	j := f.save(t, f.claim(t))
	first, e := f.completions.Post(f.ctx, f.worker, j)
	taskNeed(t, e)
	again, e := f.completions.Post(f.ctx, f.worker, j)
	taskNeed(t, e)
	if !again.Replayed || first.Header != again.Header || first.Version != again.Version || first.Cursor != again.Cursor || f.version(t) != "3" {
		t.Fatal("duplicate changed immutable receipt or authority")
	}
	taskNeed(t, f.tasks.Complete(f.ctx, f.worker, j, again))
	if e = f.tasks.Complete(f.ctx, f.worker, j, again); e != task.ErrDenied {
		t.Fatal("consumed lease remained valid")
	}
	if f.version(t) != "3" {
		t.Fatal("duplicate ack wrote game state")
	}
}
func TestTenantGraphTokenAndCallerFieldsCannotReplaceDurableLease(t *testing.T) {
	f := newTaskFixture(t, 15*time.Second, time.Minute)
	f.source(t, "create", 1)
	j := f.save(t, f.claim(t))
	original, _ := j.StorageValue()
	for _, name := range []string{"tenant", "graph", "token", "payload", "result", "version"} {
		t.Run(name, func(t *testing.T) {
			v := original
			switch name {
			case "tenant":
				v.Scope.WorkspaceID = "other"
				v.Binding.Workspace = "other"
			case "graph":
				v.Binding.GraphHash = "sha256:" + strings.Repeat("a", 64)
			case "token":
				v.Token, _ = task.NewToken()
			case "payload":
				v.Payload, _ = task.NewValue(checkpoint.Text("private-fake-task"))
			case "result":
				v.Result, _ = task.NewValue(checkpoint.Int(99))
			case "version":
				v.OriginVersion = 99
			}
			forged, e := task.NewJob(v)
			taskNeed(t, e)
			current, e := f.tasks.Current(f.ctx, f.worker, forged)
			if name == "payload" || name == "result" || name == "version" {
				if e != nil {
					t.Fatal("canonical lease unavailable")
				}
				owned, _ := current.StorageValue()
				if owned.Payload.Digest() != original.Payload.Digest() || owned.Result.Digest() != original.Result.Digest() || owned.OriginVersion != original.OriginVersion {
					t.Fatal("caller replaced durable authority input")
				}
			} else if e == nil {
				t.Fatal("foreign lease accepted")
			}
		})
	}
	if f.version(t) != "2" {
		t.Fatal("lease check wrote authority")
	}
}
func TestStaleStateVersionRejectsPreviouslySavedCompletion(t *testing.T) {
	f := newTaskFixture(t, 15*time.Second, time.Minute)
	f.source(t, "first", 1)
	j := f.save(t, f.claim(t))
	f.source(t, "second", 2)
	if _, e := f.tasks.Current(f.ctx, f.worker, j); e != task.ErrStale {
		t.Fatal("stale lease accepted")
	}
	if _, e := f.completions.Post(f.ctx, f.worker, j); e != task.ErrStale {
		t.Fatal("stale completion applied")
	}
	if f.version(t) != "3" {
		t.Fatal("stale completion changed version")
	}
}
func TestCancelledTaskAndDeliveringCancellationHaveDefinedAuthorityBoundary(t *testing.T) {
	for _, name := range []string{"running", "delivering"} {
		t.Run(name, func(t *testing.T) {
			f := newTaskFixture(t, 15*time.Second, time.Minute)
			f.source(t, "create", 1)
			j := f.claim(t)
			if name == "delivering" {
				j = f.save(t, j)
			}
			v, _ := j.StorageValue()
			e := f.tasks.Cancel(f.ctx, f.worker, v.Binding, v.TaskID)
			if name == "delivering" {
				if e != task.ErrBusy {
					t.Fatal("delivering cancellation raced commit")
				}
				r, e := f.completions.Post(f.ctx, f.worker, j)
				taskNeed(t, e)
				taskNeed(t, f.tasks.Complete(f.ctx, f.worker, j, r))
			} else {
				taskNeed(t, e)
				if _, e = f.tasks.SaveResult(f.ctx, f.worker, j, v.Payload, mustInputs(t)); e != task.ErrDenied {
					t.Fatal("cancelled task saved result")
				}
				status, e := f.tasks.Status(f.ctx, f.worker, v.Binding, v.TaskID)
				taskNeed(t, e)
				if status != task.Cancelled || f.version(t) != "2" {
					t.Fatal("cancellation changed authority")
				}
			}
		})
	}
}
func mustInputs(t *testing.T) task.Inputs {
	t.Helper()
	i, e := task.NewInputs(1000, nil)
	taskNeed(t, e)
	return i
}
func TestExpiredTaskAndRevokedWorkerCannotComplete(t *testing.T) {
	for _, name := range []string{"expired", "revoked"} {
		t.Run(name, func(t *testing.T) {
			lease, lifetime := 15*time.Second, time.Minute
			if name == "expired" {
				lease, lifetime = 100*time.Millisecond, 200*time.Millisecond
			}
			f := newTaskFixture(t, lease, lifetime)
			f.source(t, "create", 1)
			j := f.save(t, f.claim(t))
			if name == "expired" {
				time.Sleep(220 * time.Millisecond)
			} else {
				taskNeed(t, f.workerAuthority.Revoke("owned-worker"))
			}
			if _, e := f.completions.Post(f.ctx, f.worker, j); e == nil {
				t.Fatal("expired or revoked completion accepted")
			}
			if f.version(t) != "2" {
				t.Fatal("expired or revoked task changed state")
			}
		})
	}
}
func TestClaimAndResultCommitCrashesRecoverWithoutRepeatingSavedExternalResult(t *testing.T) {
	for _, point := range []string{"task-before-claim-commit", "task-after-claim-commit", "task-before-result-commit", "task-after-result-commit"} {
		t.Run(point, func(t *testing.T) {
			f := newTaskFixture(t, time.Second, time.Minute)
			f.source(t, "create", 1)
			f.setFault(point)
			if _, e := f.runtime(t).RunOnce(f.ctx); e != task.ErrUnavailable {
				t.Fatal("crash injection was not observed")
			}
			before := f.external.Load()
			time.Sleep(1100 * time.Millisecond)
			f.connection.Close()
			f.recomposeLaunch(t, f.configs)
			f.composeTasks(t)
			report, e := f.runtime(t).RunOnce(f.ctx)
			taskNeed(t, e)
			if report.Applied != 1 || f.version(t) != "3" {
				t.Fatal("crashed task not recovered")
			}
			if point == "task-after-result-commit" {
				if report.Executed != 0 || f.external.Load() != before {
					t.Fatal("saved external result repeated on restart")
				}
			} else if report.Executed != 1 {
				t.Fatal("uncommitted result treated as saved")
			}
		})
	}
}
func TestAuthorityCommitBeforeAckCrashReplaysOriginalAfterColdRestart(t *testing.T) {
	f := newTaskFixture(t, 15*time.Second, time.Minute)
	f.source(t, "create", 1)
	j := f.save(t, f.claim(t))
	original, e := f.completions.Post(f.ctx, f.worker, j)
	taskNeed(t, e)
	f.setFault("task-before-ack-commit")
	if e = f.tasks.Complete(f.ctx, f.worker, j, original); e != task.ErrUnavailable {
		t.Fatal("ack crash not observed")
	}
	taskNeed(t, f.tasks.Retry(f.ctx, f.worker, j))
	f.connection.Close()
	f.recomposeLaunch(t, f.configs)
	f.composeTasks(t)
	current := f.claim(t)
	replayed, e := f.completions.Post(f.ctx, f.worker, current)
	taskNeed(t, e)
	if !replayed.Replayed || replayed.Header != original.Header || replayed.Version != original.Version || replayed.Cursor != original.Cursor || f.version(t) != "3" {
		t.Fatal("cold duplicate wrote a second effect")
	}
	taskNeed(t, f.tasks.Complete(f.ctx, f.worker, current, replayed))
	if f.external.Load() != 0 {
		t.Fatal("cold duplicate called external provider")
	}
}
func TestAckCommittedThenDeliveryFailureDoesNotRepeatThatContinuation(t *testing.T) {
	f := newTaskFixture(t, 15*time.Second, time.Minute)
	f.source(t, "create", 1)
	j := f.save(t, f.claim(t))
	original, e := f.completions.Post(f.ctx, f.worker, j)
	taskNeed(t, e)
	f.setFault("task-after-ack-commit")
	if e = f.tasks.Complete(f.ctx, f.worker, j, original); e != task.ErrUnavailable {
		t.Fatal("unknown ack crash not observed")
	}
	next := f.claim(t)
	v, _ := next.StorageValue()
	if v.Next != 1 || next.ExpectedVersion() != 3 {
		t.Fatal("committed ack repeated old continuation")
	}
	report, e := f.runtime(t).RunOnce(f.ctx)
	if e != task.ErrNotFound || report.Applied != 0 {
		t.Fatal("active lease was stolen")
	}
	receipt, e := f.completions.Post(f.ctx, f.worker, next)
	taskNeed(t, e)
	taskNeed(t, f.tasks.Complete(f.ctx, f.worker, next, receipt))
	if f.version(t) != "4" {
		t.Fatal("second continuation not applied")
	}
}
func TestBadProviderResultIsRejectedBeforeAuthorityAndRetriesAreBounded(t *testing.T) {
	f := newTaskFixture(t, 15*time.Second, time.Minute)
	f.source(t, "create", 1)
	f.policies[0].Execute = func(context.Context, task.Value) (task.Value, error) {
		f.external.Add(1)
		return task.NewValue(checkpoint.Text(PrivateValue))
	}
	runtime := f.runtime(t)
	for n := 0; n < task.MaxAttempts; n++ {
		report, e := runtime.RunOnce(f.ctx)
		if e != task.ErrInvalid || strings.Contains(report.Code, PrivateValue) {
			t.Fatal("invalid provider result escaped validation")
		}
	}
	if _, e := runtime.RunOnce(f.ctx); e != task.ErrNotFound || f.external.Load() != task.MaxAttempts || f.version(t) != "2" {
		t.Fatal("retry bound or authoritative result validation failed")
	}
}
func TestProtectedOperationalHandlesAndErrorsNeverLeakFixturePrivateState(t *testing.T) {
	f := newTaskFixture(t, 15*time.Second, time.Minute)
	f.source(t, "create", 1)
	j := f.save(t, f.claim(t))
	v, _ := j.StorageValue()
	for _, x := range []any{f.tasks, f.workerAuthority, f.worker, f.completions, j, v, v.Payload, v.Result, v.Inputs, v.Token} {
		for _, verb := range []string{"%v", "%+v", "%#v", "%d", "%f", "%w", "%*v"} {
			text := fmt.Sprintf(verb, x)
			if strings.Contains(text, PrivateValue) || strings.Contains(text, "synthetic private provider diagnostic") {
				t.Fatal("private operational log leak")
			}
		}
		if raw, e := json.Marshal(x); e == nil || bytes.Contains(raw, []byte(PrivateValue)) {
			t.Fatal("private operational JSON export")
		}
	}
}
