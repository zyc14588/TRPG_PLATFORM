// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package task

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"io"
	"math"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/model"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/command"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
)

const (
	Queued      = "queued"
	Running     = "running"
	Ready       = "ready"
	Delivering  = "delivering"
	Done        = "done"
	Failed      = "failed"
	Cancelled   = "cancelled"
	Expired     = "expired"
	Stale       = "stale"
	MaxBody     = 512 << 10
	MaxAttempts = 4
)

type Continuation struct {
	ID    string `json:"id"`
	Value Value  `json:"-"`
}
type JobData struct {
	Scope             core.Scope     `json:"scope"`
	Binding           data.Binding   `json:"binding"`
	TaskID            string         `json:"task_id"`
	OutboxID          string         `json:"outbox_id"`
	SourceCommand     string         `json:"source_command"`
	PackageID         string         `json:"package_id"`
	ConfigurationID   string         `json:"configuration_id"`
	ConfigurationHash string         `json:"configuration_hash"`
	SourceHash        string         `json:"source_hash"`
	OriginVersion     uint64         `json:"origin_version"`
	OriginPrincipal   string         `json:"origin_principal"`
	Status            string         `json:"status"`
	Attempt           int            `json:"attempt"`
	DeliveryAttempt   int            `json:"delivery_attempt"`
	Next              int            `json:"next"`
	LeaseOwner        string         `json:"lease_owner"`
	LeaseUntil        time.Time      `json:"lease_until"`
	Expires           time.Time      `json:"expires"`
	ResultWorker      string         `json:"result_worker,omitempty"`
	Payload           Value          `json:"-"`
	Continuations     []Continuation `json:"-"`
	Result            Value          `json:"-"`
	Inputs            Inputs         `json:"-"`
	Token             Token          `json:"-"`
}

func (JobData) Format(f fmt.State, _ rune) {
	_, _ = io.WriteString(f, "<private operational task data>")
}
func (JobData) MarshalJSON() ([]byte, error) { return nil, ErrDenied }

type Job struct{ data **jobData }
type jobData struct{ value JobData }

func (Job) Format(f fmt.State, _ rune)   { _, _ = io.WriteString(f, "<tenant-bound external task>") }
func (Job) MarshalJSON() ([]byte, error) { return nil, ErrDenied }
func (j Job) state() *jobData {
	if j.data == nil {
		return nil
	}
	return *j.data
}

func validJob(v JobData) bool {
	if !store.ValidID(v.Scope.WorkspaceID) || !store.ValidID(v.Scope.RoomID) || !store.ValidID(v.Scope.GameID) || v.Scope.WorkspaceID != v.Binding.Workspace || !store.ValidID(v.Binding.Session) || !checkpoint.IsDigest(v.Binding.GraphHash) || !store.ValidID(v.TaskID) || !store.ValidID(v.OutboxID) || !store.ValidID(v.SourceCommand) || !store.ValidID(v.ConfigurationID) || !checkpoint.IsDigest(v.ConfigurationHash) || !checkpoint.IsDigest(v.SourceHash) || v.OriginVersion < 1 || v.OriginVersion >= math.MaxInt64-32 || !store.ValidID(v.OriginPrincipal) || v.Payload.state() == nil || len(v.Continuations) > 32 || v.Next < 0 || v.Next > len(v.Continuations) || v.Attempt < 0 || v.Attempt > MaxAttempts || v.DeliveryAttempt < 0 || v.DeliveryAttempt > MaxAttempts || v.Expires.IsZero() {
		return false
	}
	if _, e := model.ParsePackageID(v.PackageID); e != nil {
		return false
	}
	seen := map[string]bool{}
	size := v.Payload.Bytes()
	for _, c := range v.Continuations {
		if !store.ValidID(c.ID) || seen[c.ID] || c.Value.state() == nil {
			return false
		}
		seen[c.ID] = true
		size += c.Value.Bytes()
	}
	if size > 256<<10 {
		return false
	}
	switch v.Status {
	case Queued, Running, Ready, Delivering, Done, Failed, Cancelled, Expired, Stale:
	default:
		return false
	}
	if v.Status == Running || v.Status == Delivering {
		if !store.ValidID(v.LeaseOwner) || v.LeaseUntil.IsZero() {
			return false
		}
	}
	if v.Status == Ready || v.Status == Delivering || v.Status == Done {
		if v.Result.state() == nil || v.Inputs.state() == nil || !store.ValidID(v.ResultWorker) {
			return false
		}
	}
	return true
}
func NewJob(v JobData) (Job, error) {
	if !validJob(v) {
		return Job{}, ErrInvalid
	}
	v.Continuations = append([]Continuation(nil), v.Continuations...)
	d := &jobData{value: v}
	return Job{data: &d}, nil
}
func (j Job) StorageValue() (JobData, error) {
	if j.state() == nil {
		return JobData{}, ErrDenied
	}
	v := j.state().value
	v.Continuations = append([]Continuation(nil), v.Continuations...)
	return v, nil
}
func (j Job) Status() string {
	if j.state() == nil {
		return ""
	}
	return j.state().value.Status
}
func (j Job) Payload() (Value, error) {
	if j.state() == nil {
		return Value{}, ErrDenied
	}
	return j.state().value.Payload, nil
}
func (j Job) ExpectedVersion() uint64 {
	if j.state() == nil {
		return 0
	}
	v := j.state().value
	return v.OriginVersion + uint64(v.Next)
}

// The command identity and input are stable across leases and worker restarts.
// Raw lease/worker credentials never appear in the command or replay history.
func (j Job) Envelope() (command.Envelope, error) {
	v, e := j.StorageValue()
	if e != nil || v.Next >= len(v.Continuations) || v.Result.state() == nil {
		return command.Envelope{}, ErrDenied
	}
	c := v.Continuations[v.Next]
	result, e := v.Result.StorageValue()
	if e != nil {
		return command.Envelope{}, e
	}
	saved, e := c.Value.StorageValue()
	if e != nil {
		return command.Envelope{}, e
	}
	h := sha256.Sum256([]byte(v.Binding.Workspace + "\x00" + v.Binding.Session + "\x00" + v.Binding.GraphHash + "\x00" + v.TaskID + "\x00" + c.ID))
	id := hex.EncodeToString(h[:])
	return command.Envelope{CommandID: "task-resume-" + id, SessionID: v.Binding.Session, ExpectedStateVersion: j.ExpectedVersion(), SeatID: "task-system", Type: "resume-continuation", CorrelationID: "task-correlation-" + id, Payload: checkpoint.Object(map[string]checkpoint.Value{"task_id": checkpoint.Text(v.TaskID), "continuation_id": checkpoint.Text(c.ID), "package_id": checkpoint.Text(v.PackageID), "continuation": saved, "result": result})}, nil
}

type storedContinuation struct {
	ID    string           `json:"id"`
	Value checkpoint.Value `json:"value"`
}
type jobMetadata JobData
type storedJob struct {
	Metadata      jobMetadata          `json:"metadata"`
	Payload       checkpoint.Value     `json:"payload"`
	Continuations []storedContinuation `json:"continuations"`
	Result        *checkpoint.Value    `json:"result,omitempty"`
	Time          *int64               `json:"time,omitempty"`
	Random        []int64              `json:"random,omitempty"`
}

// EncodeForStorage/DecodeStored are protected repository boundaries. Bytes are
// never an ordinary JSON response; no live token is in this representation.
func EncodeForStorage(j Job) ([]byte, error) {
	v, e := j.StorageValue()
	if e != nil {
		return nil, e
	}
	p, e := v.Payload.StorageValue()
	if e != nil {
		return nil, e
	}
	s := storedJob{Metadata: jobMetadata(v), Payload: p, Continuations: []storedContinuation{}}
	for _, c := range v.Continuations {
		x, e := c.Value.StorageValue()
		if e != nil {
			return nil, e
		}
		s.Continuations = append(s.Continuations, storedContinuation{ID: c.ID, Value: x})
	}
	if v.Result.state() != nil {
		x, e := v.Result.StorageValue()
		if e != nil {
			return nil, e
		}
		s.Result = &x
		at, r, e := v.Inputs.StorageValue()
		if e != nil {
			return nil, e
		}
		s.Time = &at
		s.Random = r
	}
	raw, e := json.Marshal(s)
	if e != nil || len(raw) > MaxBody {
		return nil, ErrInvalid
	}
	return raw, nil
}
func DecodeStored(raw []byte) (Job, error) {
	var s storedJob
	if checkpoint.StrictDecode(raw, &s, MaxBody) != nil {
		return Job{}, ErrInvalid
	}
	v := JobData(s.Metadata)
	var e error
	v.Payload, e = NewValue(s.Payload)
	if e != nil {
		return Job{}, e
	}
	v.Continuations = []Continuation{}
	for _, c := range s.Continuations {
		p, e := NewValue(c.Value)
		if e != nil {
			return Job{}, e
		}
		v.Continuations = append(v.Continuations, Continuation{ID: c.ID, Value: p})
	}
	if s.Result != nil {
		v.Result, e = NewValue(*s.Result)
		if e != nil || s.Time == nil {
			return Job{}, ErrInvalid
		}
		v.Inputs, e = NewInputs(*s.Time, s.Random)
		if e != nil {
			return Job{}, e
		}
	}
	return NewJob(v)
}
