// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package task

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"reflect"
	"strings"
	"sync"
	"sync/atomic"
	"testing"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
)

func workerFixture(t *testing.T) (*Authority, Worker) {
	t.Helper()
	c, e := NewCredential(bytes.Repeat([]byte{9}, 32))
	if e != nil {
		t.Fatal("credential")
	}
	a, e := NewAuthority([]WorkerGrant{{ID: "worker", Credential: c, Workspaces: []string{"space"}, Expires: time.Now().Add(time.Hour)}})
	if e != nil {
		t.Fatal("authority")
	}
	w, e := a.Authenticate(context.Background(), c)
	if e != nil {
		t.Fatal("authenticate")
	}
	return a, w
}
func jobFixture(t *testing.T, status string) Job {
	t.Helper()
	p, e := NewValue(checkpoint.Object(map[string]checkpoint.Value{"value": checkpoint.Int(2)}))
	if e != nil {
		t.Fatal("payload")
	}
	c, e := NewValue(checkpoint.Text("private-task-marker"))
	if e != nil {
		t.Fatal("continuation")
	}
	token, e := NewToken()
	if e != nil {
		t.Fatal("token")
	}
	v := JobData{Scope: core.Scope{WorkspaceID: "space", RoomID: "room", GameID: "game"}, Binding: data.Binding{Workspace: "space", Session: "game", GraphHash: "sha256:" + strings.Repeat("a", 64)}, TaskID: "task", OutboxID: "outbox", SourceCommand: "source", PackageID: "example.test/task", ConfigurationID: "config", ConfigurationHash: "sha256:" + strings.Repeat("b", 64), SourceHash: "sha256:" + strings.Repeat("c", 64), OriginVersion: 2, OriginPrincipal: "person", Status: status, Attempt: 1, LeaseOwner: "worker", LeaseUntil: time.Now().Add(time.Minute), Expires: time.Now().Add(time.Hour), Payload: p, Continuations: []Continuation{{ID: "continuation", Value: c}}, Token: token}
	if status == Delivering || status == Ready || status == Done {
		v.Result, _ = NewValue(checkpoint.Int(5))
		v.Inputs, _ = NewInputs(1000, []int64{7})
		v.ResultWorker = "worker"
	}
	j, e := NewJob(v)
	if e != nil {
		t.Fatal("job")
	}
	return j
}
func TestWorkerCredentialsScopesExpiryRevocationAndOwnership(t *testing.T) {
	a, w := workerFixture(t)
	ctx := context.Background()
	if w.Check(ctx, "space") != nil || w.Check(ctx, "other") != ErrDenied || (Worker{}).Check(ctx, "space") != ErrDenied {
		t.Fatal("scope")
	}
	spaces, e := w.Workspaces(ctx)
	if e != nil {
		t.Fatal("workspaces")
	}
	spaces[0] = "other"
	if w.Check(ctx, "space") != nil {
		t.Fatal("scope alias")
	}
	if e = a.Revoke("worker"); e != nil || w.Check(ctx, "space") != ErrDenied {
		t.Fatal("revocation")
	}
	c, _ := NewCredential(bytes.Repeat([]byte{1}, 32))
	for _, g := range []WorkerGrant{{ID: "worker", Credential: c, Workspaces: []string{"space"}, Expires: time.Now().Add(-time.Second)}, {ID: "worker", Credential: c, Workspaces: []string{"space", "space"}, Expires: time.Now().Add(time.Hour)}} {
		if _, e = NewAuthority([]WorkerGrant{g}); e != ErrInvalid {
			t.Fatal("invalid grant")
		}
	}
}
func TestProtectedTaskHandlesNeverExportAndStorageCopiesAreOwned(t *testing.T) {
	a, w := workerFixture(t)
	j := jobFixture(t, Delivering)
	v, _ := j.StorageValue()
	c, _ := NewCredential(bytes.Repeat([]byte{7}, 32))
	values := []any{a, w, j, v, v.Payload, v.Result, v.Inputs, v.Token, c}
	for _, value := range values {
		for _, verb := range []string{"%v", "%+v", "%#v", "%d", "%f", "%w", "%*v", "%.*v"} {
			for _, x := range []any{value, reflect.ValueOf(value)} {
				if strings.Contains(fmt.Sprintf(verb, x), "private-task-marker") {
					t.Fatal("private formatting leak")
				}
			}
		}
		if _, e := json.Marshal(value); e == nil {
			t.Fatal("private JSON permitted")
		}
	}
	x, _ := v.Continuations[0].Value.StorageValue()
	x.String = "changed"
	v.Continuations[0].ID = "changed"
	raw, e := EncodeForStorage(j)
	if e != nil {
		t.Fatal("encode")
	}
	copy, e := DecodeStored(raw)
	if e != nil {
		t.Fatal("decode")
	}
	owned, _ := copy.StorageValue()
	if owned.Continuations[0].ID != "continuation" || owned.Continuations[0].Value.Digest() != j.state().value.Continuations[0].Value.Digest() || owned.Token.Digest() != "" {
		t.Fatal("storage alias or live token persisted")
	}
	env, e := j.Envelope()
	if e != nil || env.ExpectedStateVersion != 2 || env.Type != "resume-continuation" {
		t.Fatal("envelope")
	}
	env.Payload.Table["result"] = checkpoint.Int(100)
	again, _ := j.Envelope()
	if again.Payload.Table["result"].Number != "5" {
		t.Fatal("envelope alias")
	}
}

type memoryTasks struct {
	mu                           sync.Mutex
	j                            Job
	claimed                      bool
	executed, completed, retries int
}

func (m *memoryTasks) Claim(ctx context.Context, w Worker) (Job, error) {
	m.mu.Lock()
	defer m.mu.Unlock()
	if m.claimed {
		return Job{}, ErrNotFound
	}
	m.claimed = true
	return m.j, nil
}
func (m *memoryTasks) SaveResult(ctx context.Context, w Worker, j Job, v Value, i Inputs) (Job, error) {
	m.mu.Lock()
	defer m.mu.Unlock()
	x, _ := j.StorageValue()
	if w.Check(ctx, x.Binding.Workspace) != nil {
		return Job{}, ErrDenied
	}
	x.Result = v
	x.Inputs = i
	x.ResultWorker = w.ID()
	x.Status = Delivering
	m.j, _ = NewJob(x)
	m.executed++
	return m.j, nil
}
func (m *memoryTasks) Current(ctx context.Context, w Worker, j Job) (Job, error) {
	m.mu.Lock()
	defer m.mu.Unlock()
	v, _ := m.j.StorageValue()
	if w.Check(ctx, v.Binding.Workspace) != nil {
		return Job{}, ErrDenied
	}
	return m.j, nil
}
func (m *memoryTasks) Complete(ctx context.Context, w Worker, j Job, _ data.Receipt) error {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.completed++
	return nil
}
func (m *memoryTasks) Retry(context.Context, Worker, Job) error {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.retries++
	m.claimed = false
	return nil
}
func (m *memoryTasks) Cancel(context.Context, Worker, data.Binding, string) error { return ErrDenied }
func (m *memoryTasks) Status(context.Context, Worker, data.Binding, string) (string, error) {
	return "", ErrDenied
}
func runtimeFixture(t *testing.T, w Worker, m *memoryTasks, f func(context.Context, Value) (Value, error)) *Runtime {
	t.Helper()
	v, _ := m.j.StorageValue()
	r, e := NewWorker(WorkerOptions{Identity: w, Storage: m, Policies: []Policy{{GraphHash: v.Binding.GraphHash, ConfigurationHash: v.ConfigurationHash, PackageID: v.PackageID, ValidateInput: func(v checkpoint.Value) error {
		if v.Kind != "table" {
			return ErrInvalid
		}
		return nil
	}, ValidateResult: func(v checkpoint.Value) error {
		if v.Kind != "integer" {
			return ErrInvalid
		}
		return nil
	}, Execute: f, Inputs: func(context.Context, Job) (Inputs, error) { return NewInputs(1000, []int64{7}) }}}, Post: func(context.Context, Worker, Job) (data.Receipt, error) { return data.Receipt{}, nil }, MaxActive: 1, ExternalTimeout: 20 * time.Millisecond, PostTimeout: time.Second, Poll: 50 * time.Millisecond})
	if e != nil {
		t.Fatal("runtime")
	}
	return r
}
func TestWorkerSavedResultDoesNotRepeatExternalOperation(t *testing.T) {
	_, w := workerFixture(t)
	m := &memoryTasks{j: jobFixture(t, Delivering)}
	var calls atomic.Int64
	r := runtimeFixture(t, w, m, func(context.Context, Value) (Value, error) { calls.Add(1); return NewValue(checkpoint.Int(5)) })
	report, e := r.RunOnce(context.Background())
	if e != nil || report.Applied != 1 || report.Executed != 0 || calls.Load() != 0 || m.completed != 1 {
		t.Fatal("saved result repeated external operation")
	}
}
func TestWorkerTimeoutKeepsBoundUntilIgnoredCancellationReallyEnds(t *testing.T) {
	_, w := workerFixture(t)
	m := &memoryTasks{j: jobFixture(t, Running)}
	entered, release, finished := make(chan struct{}), make(chan struct{}), make(chan struct{})
	r := runtimeFixture(t, w, m, func(context.Context, Value) (Value, error) {
		close(entered)
		<-release
		close(finished)
		return NewValue(checkpoint.Int(5))
	})
	_, e := r.RunOnce(context.Background())
	if !errors.Is(e, context.DeadlineExceeded) {
		t.Fatal("deadline")
	}
	<-entered
	for n := 0; n < 8; n++ {
		if _, e = r.RunOnce(context.Background()); e != ErrBusy {
			t.Fatal("unbounded abandoned callback")
		}
	}
	close(release)
	<-finished
	deadline := time.Now().Add(time.Second)
	for len(r.state().slots) != 0 && time.Now().Before(deadline) {
		time.Sleep(time.Millisecond)
	}
	if len(r.state().slots) != 0 || m.executed != 0 || m.completed != 0 {
		t.Fatal("late callback applied or slot lost")
	}
}
func TestWorkerProviderPanicInvalidResultAndRevocationStayCanonical(t *testing.T) {
	for _, name := range []string{"panic", "private-error", "invalid-result", "revoke"} {
		t.Run(name, func(t *testing.T) {
			a, w := workerFixture(t)
			m := &memoryTasks{j: jobFixture(t, Running)}
			r := runtimeFixture(t, w, m, func(context.Context, Value) (Value, error) {
				switch name {
				case "panic":
					panic("private-task-marker")
				case "private-error":
					return Value{}, errors.New("private-task-marker")
				case "invalid-result":
					return NewValue(checkpoint.Text("private-task-marker"))
				default:
					_ = a.Revoke("worker")
					return NewValue(checkpoint.Int(5))
				}
			})
			report, e := r.RunOnce(context.Background())
			if e == nil || strings.Contains(e.Error(), "private-task-marker") || strings.Contains(report.Code, "private-task-marker") || m.executed != 0 || m.completed != 0 {
				t.Fatal("invalid or revoked result applied")
			}
		})
	}
}
