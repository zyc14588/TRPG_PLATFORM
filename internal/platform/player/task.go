// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package player

import (
	"context"
	"fmt"
	"io"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	packagemodel "github.com/zyc14588/TRPG_PLATFORM/internal/package/model"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
	"github.com/zyc14588/TRPG_PLATFORM/internal/task"
)

type TaskStorage struct{ data **taskStorageData }
type taskStorageData struct {
	base    task.Storage
	control *Control
}

func (TaskStorage) Format(f fmt.State, _ rune) {
	_, _ = io.WriteString(f, "<player-controlled task storage>")
}
func (TaskStorage) MarshalJSON() ([]byte, error) { return nil, task.ErrDenied }
func (s *TaskStorage) state() *taskStorageData {
	if s == nil || s.data == nil {
		return nil
	}
	return *s.data
}

type taskSubject struct {
	scope                                                    core.Scope
	binding                                                  data.Binding
	configuration, configurationHash, pkg, taskID, principal string
	version                                                  uint64
}

func taskMetadata(v task.JobData) checkpoint.Value {
	return checkpoint.Object(map[string]checkpoint.Value{"workspace": checkpoint.Text(v.Scope.WorkspaceID), "room": checkpoint.Text(v.Scope.RoomID), "game": checkpoint.Text(v.Scope.GameID), "session": checkpoint.Text(v.Binding.Session), "graph": checkpoint.Text(v.Binding.GraphHash), "configuration": checkpoint.Text(v.ConfigurationID), "configuration_hash": checkpoint.Text(v.ConfigurationHash), "package": checkpoint.Text(v.PackageID), "task": checkpoint.Text(v.TaskID), "principal": checkpoint.Text(v.OriginPrincipal), "version": checkpoint.Text(decimal(v.OriginVersion))})
}
func taskInput(v checkpoint.Value) (taskSubject, checkpoint.Value, error) {
	if checkpoint.Validate(v) != nil || v.Kind != "table" || len(v.Table) != 2 {
		return taskSubject{}, checkpoint.Value{}, task.ErrInvalid
	}
	meta := v.Table["player_server"]
	input, ok := v.Table["input"]
	if !ok || meta.Kind != "table" || len(meta.Table) != 11 {
		return taskSubject{}, checkpoint.Value{}, task.ErrInvalid
	}
	r := taskSubject{}
	for key, dest := range map[string]*string{"workspace": &r.scope.WorkspaceID, "room": &r.scope.RoomID, "game": &r.scope.GameID, "session": &r.binding.Session, "graph": &r.binding.GraphHash, "configuration": &r.configuration, "configuration_hash": &r.configurationHash, "package": &r.pkg, "task": &r.taskID, "principal": &r.principal} {
		x := meta.Table[key]
		if x.Kind != "string" {
			return taskSubject{}, checkpoint.Value{}, task.ErrInvalid
		}
		*dest = x.String
	}
	x := meta.Table["version"]
	if x.Kind != "string" {
		return taskSubject{}, checkpoint.Value{}, task.ErrInvalid
	}
	var e error
	r.version, e = counter(x.String)
	r.binding.Workspace = r.scope.WorkspaceID
	if e != nil || r.version < 1 || !store.ValidID(r.scope.WorkspaceID) || !store.ValidID(r.scope.RoomID) || !store.ValidID(r.scope.GameID) || !store.ValidID(r.binding.Session) || !checkpoint.IsDigest(r.binding.GraphHash) || !checkpoint.IsDigest(r.configurationHash) || !store.ValidID(r.configuration) || !store.ValidID(r.taskID) || !store.ValidID(r.principal) {
		return taskSubject{}, checkpoint.Value{}, task.ErrInvalid
	}
	if _, e = packagemodel.ParsePackageID(r.pkg); e != nil {
		return taskSubject{}, checkpoint.Value{}, task.ErrInvalid
	}
	return r, input, nil
}
func stripJob(j task.Job) (task.Job, error) {
	v, e := j.StorageValue()
	if e != nil {
		return task.Job{}, task.ErrDenied
	}
	raw, e := v.Payload.StorageValue()
	if e != nil {
		return task.Job{}, task.ErrDenied
	}
	meta, input, e := taskInput(raw)
	if e != nil || meta.scope != v.Scope || meta.binding != v.Binding || meta.configuration != v.ConfigurationID || meta.configurationHash != v.ConfigurationHash || meta.pkg != v.PackageID || meta.taskID != v.TaskID || meta.principal != v.OriginPrincipal || meta.version != v.OriginVersion {
		return task.Job{}, task.ErrDenied
	}
	v.Payload, e = task.NewValue(input)
	if e != nil {
		return task.Job{}, e
	}
	return task.NewJob(v)
}

// BindTasks wraps every external callback, including AI proposal/narrative
// pipelines, before dispatch. The scope comes from the actual durable job.
func BindTasks(base task.Storage, control *Control, policies []task.Policy) (*TaskStorage, []task.Policy, error) {
	if base == nil || control.state() == nil || len(policies) < 1 || len(policies) > 128 {
		return nil, nil, task.ErrInvalid
	}
	out := make([]task.Policy, 0, len(policies))
	for _, p := range policies {
		if p.ValidateInput == nil || p.ValidateResult == nil || p.Execute == nil || p.Inputs == nil {
			return nil, nil, task.ErrInvalid
		}
		original := p
		check := func(v checkpoint.Value) (taskSubject, checkpoint.Value, error) {
			meta, input, e := taskInput(v)
			if e != nil || meta.binding.GraphHash != original.GraphHash || meta.configurationHash != original.ConfigurationHash || meta.pkg != original.PackageID {
				return taskSubject{}, checkpoint.Value{}, task.ErrDenied
			}
			if original.ValidateInput(input) != nil {
				return taskSubject{}, checkpoint.Value{}, task.ErrInvalid
			}
			return meta, input, nil
		}
		p.ValidateInput = func(v checkpoint.Value) error { _, _, e := check(v); return e }
		p.Execute = func(ctx context.Context, value task.Value) (task.Value, error) {
			raw, e := value.StorageValue()
			if e != nil {
				return task.Value{}, task.ErrDenied
			}
			meta, input, e := check(raw)
			if e != nil {
				return task.Value{}, e
			}
			owned, release, e := control.BeginMutation(ctx, meta.scope, meta.binding)
			if e != nil {
				return task.Value{}, task.ErrDenied
			}
			defer release()
			inner, e := task.NewValue(input)
			if e != nil {
				return task.Value{}, e
			}
			result, e := original.Execute(owned, inner)
			if e != nil || owned.Err() != nil {
				return task.Value{}, task.ErrDenied
			}
			return result, nil
		}
		p.Inputs = func(ctx context.Context, j task.Job) (task.Inputs, error) {
			inner, e := stripJob(j)
			if e != nil {
				return task.Inputs{}, e
			}
			return original.Inputs(ctx, inner)
		}
		out = append(out, p)
	}
	d := &taskStorageData{base, control}
	return &TaskStorage{data: &d}, out, nil
}
func (s *TaskStorage) Claim(ctx context.Context, w task.Worker) (task.Job, error) {
	if s.state() == nil {
		return task.Job{}, task.ErrDenied
	}
	j, e := s.state().base.Claim(ctx, w)
	if e != nil {
		return task.Job{}, task.SafeError(e)
	}
	v, e := j.StorageValue()
	if e != nil {
		return task.Job{}, task.ErrDenied
	}
	input, e := v.Payload.StorageValue()
	if e != nil {
		return task.Job{}, task.ErrDenied
	}
	v.Payload, e = task.NewValue(checkpoint.Object(map[string]checkpoint.Value{"player_server": taskMetadata(v), "input": input}))
	if e != nil {
		return task.Job{}, e
	}
	return task.NewJob(v)
}
func (s *TaskStorage) SaveResult(c context.Context, w task.Worker, j task.Job, v task.Value, i task.Inputs) (task.Job, error) {
	inner, e := stripJob(j)
	if e != nil {
		return task.Job{}, e
	}
	return s.state().base.SaveResult(c, w, inner, v, i)
}
func (s *TaskStorage) Current(c context.Context, w task.Worker, j task.Job) (task.Job, error) {
	return s.state().base.Current(c, w, j)
}
func (s *TaskStorage) Complete(c context.Context, w task.Worker, j task.Job, r data.Receipt) error {
	return s.state().base.Complete(c, w, j, r)
}
func (s *TaskStorage) Retry(c context.Context, w task.Worker, j task.Job) error {
	v, e := j.StorageValue()
	if e != nil {
		return task.ErrDenied
	}
	if s.state().control.Check(c, v.Scope, v.Binding) != nil {
		return task.ErrDenied
	}
	raw, e := v.Payload.StorageValue()
	if e != nil {
		return task.ErrDenied
	}
	if raw.Kind == "table" {
		if _, ok := raw.Table["player_server"]; ok {
			j, e = stripJob(j)
			if e != nil {
				return e
			}
		}
	}
	return s.state().base.Retry(c, w, j)
}
func (s *TaskStorage) Cancel(c context.Context, w task.Worker, b data.Binding, id string) error {
	return s.state().base.Cancel(c, w, b, id)
}
func (s *TaskStorage) Status(c context.Context, w task.Worker, b data.Binding, id string) (string, error) {
	return s.state().base.Status(c, w, b, id)
}

var _ task.Storage = (*TaskStorage)(nil)
