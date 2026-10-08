// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package gateway

import (
	"context"
	"fmt"
	"io"
	"strconv"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/model"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	packagemodel "github.com/zyc14588/TRPG_PLATFORM/internal/package/model"
	store "github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
	"github.com/zyc14588/TRPG_PLATFORM/internal/task"
)

// TaskStorage adds metadata from the CURRENT authenticated SQL lease to the
// private Execute input. Package payloads cannot replace workspace, room,
// Session, source principal, version or configuration with their own fields.
// SaveResult and Post always use the original canonical repository job.
type TaskStorage struct{ data **taskStorageData }
type taskStorageData struct{ base task.Storage }

func (TaskStorage) Format(f fmt.State, _ rune) {
	_, _ = io.WriteString(f, "<canonical model task storage adapter>")
}
func (TaskStorage) MarshalJSON() ([]byte, error) { return nil, task.ErrDenied }
func (s *TaskStorage) state() *taskStorageData {
	if s == nil || s.data == nil {
		return nil
	}
	return *s.data
}
func BindTasks(base task.Storage) (*TaskStorage, error) {
	if base == nil {
		return nil, task.ErrInvalid
	}
	d := &taskStorageData{base: base}
	return &TaskStorage{data: &d}, nil
}
func (s *TaskStorage) Claim(ctx context.Context, w task.Worker) (task.Job, error) {
	if s.state() == nil {
		return task.Job{}, task.ErrDenied
	}
	j, e := s.state().base.Claim(ctx, w)
	if e != nil {
		return task.Job{}, task.SafeError(e)
	}
	current, e := s.state().base.Current(ctx, w, j)
	if e != nil {
		return task.Job{}, task.SafeError(e)
	}
	v, e := current.StorageValue()
	if e != nil || w.Check(ctx, v.Scope.WorkspaceID) != nil {
		return task.Job{}, task.ErrDenied
	}
	p, e := v.Payload.StorageValue()
	if e != nil {
		return task.Job{}, task.ErrDenied
	}
	meta := map[string]checkpoint.Value{"workspace": checkpoint.Text(v.Scope.WorkspaceID), "room": checkpoint.Text(v.Scope.RoomID), "game": checkpoint.Text(v.Scope.GameID), "session": checkpoint.Text(v.Binding.Session), "graph": checkpoint.Text(v.Binding.GraphHash), "task": checkpoint.Text(v.TaskID), "package": checkpoint.Text(v.PackageID), "configuration": checkpoint.Text(v.ConfigurationID), "configuration_hash": checkpoint.Text(v.ConfigurationHash), "principal": checkpoint.Text(v.OriginPrincipal), "version": checkpoint.Int(int64(v.OriginVersion))}
	v.Payload, e = task.NewValue(checkpoint.Object(map[string]checkpoint.Value{"server": checkpoint.Object(meta), "input": p}))
	if e != nil {
		return task.Job{}, task.ErrDenied
	}
	return task.NewJob(v)
}
func (s *TaskStorage) SaveResult(ctx context.Context, w task.Worker, j task.Job, result task.Value, inputs task.Inputs) (task.Job, error) {
	current, e := s.Current(ctx, w, j)
	if e != nil {
		return task.Job{}, e
	}
	return s.state().base.SaveResult(ctx, w, current, result, inputs)
}
func (s *TaskStorage) Current(ctx context.Context, w task.Worker, j task.Job) (task.Job, error) {
	if s.state() == nil {
		return task.Job{}, task.ErrDenied
	}
	return s.state().base.Current(ctx, w, j)
}
func (s *TaskStorage) Complete(ctx context.Context, w task.Worker, j task.Job, r data.Receipt) error {
	if s.state() == nil {
		return task.ErrDenied
	}
	return s.state().base.Complete(ctx, w, j, r)
}
func (s *TaskStorage) Retry(ctx context.Context, w task.Worker, j task.Job) error {
	if s.state() == nil {
		return task.ErrDenied
	}
	return s.state().base.Retry(ctx, w, j)
}
func (s *TaskStorage) Cancel(ctx context.Context, w task.Worker, b data.Binding, id string) error {
	if s.state() == nil {
		return task.ErrDenied
	}
	return s.state().base.Cancel(ctx, w, b, id)
}
func (s *TaskStorage) Status(ctx context.Context, w task.Worker, b data.Binding, id string) (string, error) {
	if s.state() == nil {
		return "", task.ErrDenied
	}
	return s.state().base.Status(ctx, w, b, id)
}

type CallerSource func(context.Context, core.Scope, string) (model.Caller, error)
type PolicyOptions struct {
	GraphHash, ConfigurationHash, PackageID string
	Caller                                  CallerSource
	ValidateInput                           func(checkpoint.Value) error
}

func decodeInput(v checkpoint.Value) (RequestData, checkpoint.Value, error) {
	if v.Kind != "table" || len(v.Table) != 2 || v.Table["server"].Kind != "table" || v.Table["input"].Kind != "table" {
		return RequestData{}, checkpoint.Value{}, task.ErrInvalid
	}
	m := v.Table["server"].Table
	input := v.Table["input"]
	if len(m) != 11 || len(input.Table) != 3 {
		return RequestData{}, checkpoint.Value{}, task.ErrInvalid
	}
	get := func(key string) (string, error) {
		x := m[key]
		if x.Kind != "string" || x.String == "" {
			return "", task.ErrInvalid
		}
		return x.String, nil
	}
	values := map[string]string{}
	for _, key := range []string{"workspace", "room", "game", "session", "graph", "task", "package", "configuration", "configuration_hash", "principal"} {
		s, e := get(key)
		if e != nil {
			return RequestData{}, checkpoint.Value{}, e
		}
		values[key] = s
	}
	n := m["version"]
	version, e := strconv.ParseUint(n.Number, 10, 64)
	if e != nil || n.Kind != "integer" {
		return RequestData{}, checkpoint.Value{}, task.ErrInvalid
	}
	r := RequestData{Scope: core.Scope{WorkspaceID: values["workspace"], RoomID: values["room"], GameID: values["game"]}, Binding: data.Binding{Workspace: values["workspace"], Session: values["session"], GraphHash: values["graph"]}, TaskID: values["task"], PackageID: values["package"], ConfigurationID: values["configuration"], ConfigurationHash: values["configuration_hash"], OriginPrincipal: values["principal"], OriginVersion: version}
	for key, dest := range map[string]*string{"seat_id": &r.SeatID, "selection": &r.Selection, "mode": &r.Mode} {
		x := input.Table[key]
		if x.Kind != "string" {
			return RequestData{}, checkpoint.Value{}, task.ErrInvalid
		}
		*dest = x.String
	}
	if _, e := packagemodel.ParsePackageID(r.PackageID); e != nil || !requestValid(r) {
		return RequestData{}, checkpoint.Value{}, task.ErrInvalid
	}
	return r, input, nil
}
func ResultValue(output Output) (task.Value, error) {
	v := output.StorageValue()
	m := map[string]checkpoint.Value{"mode": checkpoint.Text(v.Mode), "status": checkpoint.Text(v.Status)}
	if v.Status == "complete" {
		if v.Mode == "proposal" && v.Action != nil {
			m["action"] = checkpoint.Object(map[string]checkpoint.Value{"type": checkpoint.Text(v.Action.Type), "expected_state_version": checkpoint.Int(int64(v.Action.ExpectedVersion)), "payload": v.Action.Payload})
		} else if v.Mode == "narrative" && v.Narrative != "" {
			m["narrative"] = checkpoint.Text(v.Narrative)
		} else {
			return task.Value{}, task.ErrInvalid
		}
	} else if v.Status != "paused" {
		return task.Value{}, task.ErrInvalid
	}
	return task.NewValue(checkpoint.Object(m))
}
func ValidateResult(v checkpoint.Value) error {
	if v.Kind != "table" || len(v.Table) < 2 || len(v.Table) > 3 {
		return task.ErrInvalid
	}
	mode, status := v.Table["mode"], v.Table["status"]
	if mode.Kind != "string" || (mode.String != "proposal" && mode.String != "narrative") || status.Kind != "string" {
		return task.ErrInvalid
	}
	if status.String == "paused" {
		if len(v.Table) == 2 {
			return nil
		}
		return task.ErrInvalid
	}
	if status.String != "complete" || len(v.Table) != 3 {
		return task.ErrInvalid
	}
	if mode.String == "narrative" {
		x := v.Table["narrative"]
		if x.Kind == "string" && len(x.String) > 0 && len(x.String) <= 16<<10 {
			return nil
		}
		return task.ErrInvalid
	}
	x := v.Table["action"]
	if x.Kind != "table" || len(x.Table) != 3 || x.Table["type"].Kind != "string" || !store.ValidID(x.Table["type"].String) || x.Table["expected_state_version"].Kind != "integer" || checkpoint.Validate(x.Table["payload"]) != nil {
		return task.ErrInvalid
	}
	n, e := strconv.ParseUint(x.Table["expected_state_version"].Number, 10, 64)
	if e != nil || n == 0 || n >= 1<<53 {
		return task.ErrInvalid
	}
	return nil
}
func (s *Service) Policy(o PolicyOptions) (task.Policy, error) {
	if s.state() == nil || !checkpoint.IsDigest(o.GraphHash) || !checkpoint.IsDigest(o.ConfigurationHash) || o.Caller == nil || o.ValidateInput == nil {
		return task.Policy{}, task.ErrInvalid
	}
	if _, e := packagemodel.ParsePackageID(o.PackageID); e != nil {
		return task.Policy{}, task.ErrInvalid
	}
	check := func(v checkpoint.Value) error {
		r, input, e := decodeInput(v)
		if e != nil || r.Binding.GraphHash != o.GraphHash || r.ConfigurationHash != o.ConfigurationHash || r.PackageID != o.PackageID {
			return task.ErrDenied
		}
		if o.ValidateInput(input) != nil {
			return task.ErrInvalid
		}
		return nil
	}
	return task.Policy{GraphHash: o.GraphHash, ConfigurationHash: o.ConfigurationHash, PackageID: o.PackageID, ValidateInput: check, ValidateResult: ValidateResult, Execute: func(ctx context.Context, value task.Value) (task.Value, error) {
		v, e := value.StorageValue()
		if e != nil || check(v) != nil {
			return task.Value{}, task.ErrDenied
		}
		r, _, _ := decodeInput(v)
		caller, e := o.Caller(ctx, r.Scope, r.OriginPrincipal)
		if e != nil {
			return task.Value{}, task.ErrDenied
		}
		output, e := s.Execute(ctx, caller, auth.RoomSecret(r))
		if e != nil {
			return task.Value{}, task.SafeError(e)
		}
		return ResultValue(output)
	}, Inputs: func(ctx context.Context, j task.Job) (task.Inputs, error) {
		if ctx == nil || ctx.Err() != nil {
			return task.Inputs{}, task.ErrDenied
		}
		if _, e := j.StorageValue(); e != nil {
			return task.Inputs{}, task.ErrDenied
		}
		return task.NewInputs(time.Now().UnixMilli(), nil)
	}}, nil
}

var _ task.Storage = (*TaskStorage)(nil)
