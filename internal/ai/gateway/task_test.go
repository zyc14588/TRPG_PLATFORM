// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package gateway

import (
	"context"
	"testing"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
	"github.com/zyc14588/TRPG_PLATFORM/internal/task"
)

type storedJob struct {
	job          task.Job
	savedPayload string
}

func scopeFixture() core.Scope {
	return core.Scope{WorkspaceID: "owned-workspace", RoomID: "owned-room", GameID: "owned-game"}
}
func (s *storedJob) Claim(context.Context, task.Worker) (task.Job, error) { return s.job, nil }
func (s *storedJob) Current(context.Context, task.Worker, task.Job) (task.Job, error) {
	if s.job.Status() != task.Delivering {
		return task.Job{}, task.ErrDenied
	}
	return s.job, nil
}
func (s *storedJob) SaveResult(_ context.Context, _ task.Worker, j task.Job, _ task.Value, _ task.Inputs) (task.Job, error) {
	v, _ := j.StorageValue()
	s.savedPayload = v.Payload.Digest()
	return j, nil
}
func (s *storedJob) Complete(context.Context, task.Worker, task.Job, data.Receipt) error { return nil }
func (s *storedJob) Retry(context.Context, task.Worker, task.Job) error                  { return nil }
func (s *storedJob) Cancel(context.Context, task.Worker, data.Binding, string) error     { return nil }
func (s *storedJob) Status(context.Context, task.Worker, data.Binding, string) (string, error) {
	return task.Running, nil
}
func TestExecuteMetadataComesFromCanonicalLeaseAndSaveUsesOriginalJob(t *testing.T) {
	credential, e := task.NewCredential(make([]byte, 32))
	if e != nil {
		t.Fatal("fixture worker credential rejected")
	}
	authority, e := task.NewAuthority([]task.WorkerGrant{{ID: "owned-worker", Credential: credential, Workspaces: []string{"owned-workspace"}, Expires: time.Now().Add(time.Hour)}})
	if e != nil {
		t.Fatal("fixture worker authority rejected")
	}
	worker, e := authority.Authenticate(context.Background(), credential)
	if e != nil {
		t.Fatal("fixture worker denied")
	}
	input, e := task.NewValue(checkpoint.Object(map[string]checkpoint.Value{"seat_id": checkpoint.Text("ai"), "selection": checkpoint.Text("selected"), "mode": checkpoint.Text("proposal")}))
	if e != nil {
		t.Fatal("fixture input rejected")
	}
	token, e := task.NewToken()
	if e != nil {
		t.Fatal("fixture token rejected")
	}
	job, e := task.NewJob(task.JobData{Scope: scopeFixture(), Binding: data.Binding{Workspace: "owned-workspace", Session: "owned-session", GraphHash: checkpoint.Hash([]byte("graph"))}, TaskID: "owned-task", OutboxID: "owned-outbox", SourceCommand: "owned-command", PackageID: "example.test/host", ConfigurationID: "owned-configuration", ConfigurationHash: checkpoint.Hash([]byte("configuration")), SourceHash: checkpoint.Hash([]byte("source")), OriginVersion: 2, OriginPrincipal: "owned-participant", Status: task.Running, Attempt: 1, LeaseOwner: "owned-worker", LeaseUntil: time.Now().Add(time.Minute), Expires: time.Now().Add(time.Hour), Payload: input, Token: token})
	if e != nil {
		t.Fatal("fixture job rejected")
	}
	base := &storedJob{job: job}
	adapter, e := BindTasks(base)
	if e != nil {
		t.Fatal("task adapter rejected")
	}
	claimed, e := adapter.Claim(context.Background(), worker)
	if e != nil {
		t.Fatal("canonical claim rejected")
	}
	v, _ := claimed.StorageValue()
	raw, _ := v.Payload.StorageValue()
	r, _, e := decodeInput(raw)
	if e != nil || r.Scope != scopeFixture() || r.TaskID != "owned-task" || r.OriginVersion != 2 {
		t.Fatal("canonical metadata lost")
	}
	inputs, _ := task.NewInputs(1, nil)
	if _, e := adapter.SaveResult(context.Background(), worker, claimed, input, inputs); e != nil || base.savedPayload != input.Digest() {
		t.Fatal("augmented execution payload replaced durable source")
	}
	raw.Table["server"].Table["workspace"] = checkpoint.Text("foreign")
	fresh, _ := claimed.Payload()
	freshRaw, _ := fresh.StorageValue()
	if freshRaw.Table["server"].Table["workspace"].String != "owned-workspace" {
		t.Fatal("metadata handle shared caller map")
	}
	v.Payload, _ = task.NewValue(raw)
	tampered, _ := task.NewJob(v)
	if _, e := adapter.SaveResult(context.Background(), worker, tampered, input, inputs); e != task.ErrDenied {
		t.Fatal("changed execution metadata reached canonical save")
	}
}
func TestModelResultRejectsFakeSuccessAndUnknownAuthorityFields(t *testing.T) {
	for _, v := range []checkpoint.Value{checkpoint.Object(map[string]checkpoint.Value{"mode": checkpoint.Text("proposal"), "status": checkpoint.Text("complete")}), checkpoint.Object(map[string]checkpoint.Value{"mode": checkpoint.Text("proposal"), "status": checkpoint.Text("paused"), "event": checkpoint.Text("forged")}), checkpoint.Object(map[string]checkpoint.Value{"mode": checkpoint.Text("narrative"), "status": checkpoint.Text("complete"), "narrative": checkpoint.Text("")})} {
		if ValidateResult(v) == nil {
			t.Fatal("invalid success result accepted")
		}
	}
	if ValidateResult(checkpoint.Object(map[string]checkpoint.Value{"mode": checkpoint.Text("proposal"), "status": checkpoint.Text("paused")})) != nil {
		t.Fatal("honest pause rejected")
	}
	value, e := ResultValue(auth.RoomSecret(OutputData{Mode: "narrative", Status: "paused", Narrative: "committed filtered result"}))
	if e != nil {
		t.Fatal("narrative template rejected")
	}
	raw, e := value.StorageValue()
	if e != nil || ValidateResult(raw) != nil || raw.Table["narrative"].String != "committed filtered result" {
		t.Fatal("durable result discarded deterministic template")
	}
	raw.Table["mode"] = checkpoint.Text("proposal")
	if ValidateResult(raw) == nil {
		t.Fatal("proposal pause admitted narrative authority fields")
	}
}
