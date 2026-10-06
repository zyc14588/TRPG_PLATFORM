// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// Package persistence composes SessionActor with verified installed packages
// and trusted storage interfaces. It imports no SQL driver or database handle.
package persistence

import (
	"context"
	"crypto/rand"
	"encoding/hex"
	"encoding/json"
	"errors"
	"sync"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/hostapi"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/install"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/actor"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/command"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/recovery"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
)

type Repository interface {
	recovery.Repository
	ReadJournal(context.Context, data.Binding, uint64, int) (data.JournalPage, error)
	ReadCreation(context.Context, data.Binding) (data.Creation, error)
	LookupCommand(context.Context, data.Binding, string, string) (data.Receipt, error)
	SaveCheckpoint(context.Context, data.CheckpointCache) error
}
type Options struct {
	Factory    *install.SessionFactory
	Repository Repository
	Request    func(data.Binding) (install.SessionRequest, error)
	// Inputs are operator-selected deterministic fixture inputs, persisted with
	// the command. They do not trigger an external model or network request.
	Time        int64
	Random      []int64
	ToolResults []checkpoint.Value
}
type Installed struct {
	options  Options
	recovery *recovery.Service
}
type engine struct {
	owner           *Installed
	session         *install.InstalledSession
	version, cursor uint64
	closeOnce       sync.Once
	closeErr        error
}

func New(o Options) (*Installed, error) {
	if o.Factory == nil || o.Repository == nil || o.Request == nil || o.Time < 0 || len(o.Random) > 256 || len(o.ToolResults) > 32 {
		return nil, command.ErrDenied
	}
	o.Random = append([]int64(nil), o.Random...)
	for _, v := range o.ToolResults {
		if checkpoint.Validate(v) != nil {
			return nil, command.ErrDenied
		}
	}
	raw, _ := json.Marshal(o.ToolResults)
	if err := json.Unmarshal(raw, &o.ToolResults); err != nil {
		return nil, err
	}
	rebuild, err := recovery.New(o.Repository)
	if err != nil {
		return nil, err
	}
	return &Installed{options: o, recovery: rebuild}, nil
}
func Input(e command.Envelope) checkpoint.Value {
	return checkpoint.Object(map[string]checkpoint.Value{"type": checkpoint.Text(e.Type), "seat_id": checkpoint.Text(e.SeatID), "correlation_id": checkpoint.Text(e.CorrelationID), "payload": e.Payload})
}
func (p *Installed) Resolve(ctx context.Context, i command.Identity, e command.Envelope) (data.Receipt, bool, error) {
	r, err := p.options.Repository.LookupCommand(ctx, i.Binding(), i.Principal(), e.CommandID)
	if errors.Is(err, data.ErrNotFound) {
		return data.Receipt{}, false, nil
	}
	if err != nil {
		return data.Receipt{}, false, err
	}
	meta := data.EnvelopeMetadata{Seat: e.SeatID, Type: e.Type, Correlation: e.CorrelationID}
	callback := "command"
	if e.Type == "end" {
		callback = "on_session_end"
	}
	expected, _ := json.Marshal(Input(e))
	saved, _ := json.Marshal(r.Inputs.Command)
	if r.Header.ExpectedVersion != e.ExpectedStateVersion || r.Header.ReadOnly || r.Inputs.Envelope == nil || *r.Inputs.Envelope != meta || r.Inputs.Callback != callback || string(expected) != string(saved) {
		return data.Receipt{}, false, data.ErrConflict
	}
	r.Replayed = true
	return r, true, nil
}
func (p *Installed) Open(ctx context.Context, b data.Binding) (result actor.Engine, err error) {
	page, err := p.options.Repository.ReadJournal(ctx, b, 0, 1)
	if err != nil {
		return nil, err
	}
	if page.Ended {
		return nil, actor.ErrEnded
	}
	if _, err = p.options.Repository.ReadCreation(ctx, b); err != nil {
		return nil, err
	}
	r, err := p.options.Request(b)
	if err != nil || r.Workspace != b.Workspace || r.Session != b.Session {
		return nil, command.ErrDenied
	}
	s, err := p.options.Factory.Recover(ctx, r, p.recovery.Build)
	if err != nil {
		return nil, err
	}
	defer func() {
		if recover() != nil {
			_ = s.Close()
			result = nil
			err = actor.ErrPanic
		}
	}()
	if s.Binding != b || s.VM.StateVersion() != page.Version {
		_ = s.Close()
		return nil, data.ErrConflict
	}
	return &engine{owner: p, session: s, version: page.Version, cursor: page.Cursor}, nil
}
func (p *Installed) Journal(ctx context.Context, b data.Binding, after uint64, limit int) (data.JournalPage, error) {
	return p.options.Repository.ReadJournal(ctx, b, after, limit)
}
func (e *engine) Version() uint64 { return e.version }
func (e *engine) Cursor() uint64  { return e.cursor }
func (e *engine) Execute(ctx context.Context, i command.Identity, c command.Envelope) (data.Receipt, error) {
	if i.Binding() != e.session.Binding {
		return data.Receipt{}, command.ErrDenied
	}
	callback := "command"
	if c.Type == "end" {
		callback = "on_session_end"
	}
	r, err := e.session.Commands.Execute(ctx, e.session.VM.Token(), hostapi.Command{Callback: callback, ID: c.CommandID, Principal: i.Principal(), ExpectedVersion: c.ExpectedStateVersion, Input: Input(c), Time: e.owner.options.Time, Random: append([]int64(nil), e.owner.options.Random...), ToolResults: e.owner.options.ToolResults, Envelope: &data.EnvelopeMetadata{Seat: c.SeatID, Type: c.Type, Correlation: c.CorrelationID}})
	if err == nil && !r.Replayed {
		e.version = r.Version
		e.cursor = r.Cursor
	}
	return r, err
}
func readID() (string, error) {
	var b [16]byte
	if _, err := rand.Read(b[:]); err != nil {
		return "", err
	}
	return "read-" + hex.EncodeToString(b[:]), nil
}
func (e *engine) read(ctx context.Context, principal, callback string, input checkpoint.Value) (data.Receipt, error) {
	id, err := readID()
	if err != nil {
		return data.Receipt{}, err
	}
	return e.session.Commands.Read(ctx, e.session.VM.Token(), hostapi.Command{Callback: callback, ID: id, Principal: principal, ExpectedVersion: e.version, Input: input, Time: e.owner.options.Time})
}
func (e *engine) Project(ctx context.Context, i command.Identity) (checkpoint.Value, error) {
	if i.Binding() != e.session.Binding {
		return checkpoint.Value{}, command.ErrDenied
	}
	r, err := e.read(ctx, i.Principal(), "project_view", checkpoint.Object(map[string]checkpoint.Value{"seat_id": checkpoint.Text(i.Seat())}))
	return r.Result, err
}
func (e *engine) Checkpoint(ctx context.Context) error {
	r, err := e.read(ctx, "platform", "create_checkpoint", checkpoint.Object(map[string]checkpoint.Value{}))
	if err != nil {
		return err
	}
	return e.owner.recovery.Save(ctx, e.session.Recovery, r.Result, e.version, e.cursor)
}
func (e *engine) Close() error {
	e.closeOnce.Do(func() {
		defer func() { e.closeErr = errors.Join(e.closeErr, e.session.Close()) }()
		ctx, cancel := context.WithTimeout(context.Background(), time.Duration(e.session.VM.Limits().WallMillis)*time.Millisecond)
		defer cancel()
		_, e.closeErr = e.read(ctx, "platform", "cleanup", checkpoint.Object(map[string]checkpoint.Value{}))
	})
	return e.closeErr
}
