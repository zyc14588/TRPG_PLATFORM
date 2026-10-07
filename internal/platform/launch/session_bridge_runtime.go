// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package launch

import (
	"context"
	"crypto/rand"
	"encoding/hex"
	"errors"
	"sync"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/hostapi"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/install"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/actor"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/command"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/persistence"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/recovery"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
)

// This composition retains the accepted installed package, controller,
// immutable-history recovery and duplicate resolver. Only the native command's
// inputs are selected by its current trusted package policy rather than the M1
// fixture's fixed operator inputs. Host methods and transaction rules are intact.
type sessionRuntime struct {
	factory   *install.SessionFactory
	repo      persistence.Repository
	request   install.SessionRequest
	binding   data.Binding
	authority *command.Authority
	recovery  *recovery.Service
	duplicate *persistence.Installed
}
type sessionEngine struct {
	owner           *sessionRuntime
	session         *install.InstalledSession
	version, cursor uint64
	closeOnce       sync.Once
	closeErr        error
}

func newSessionRuntime(factory *install.SessionFactory, repo persistence.Repository, q install.SessionRequest, b data.Binding, a *command.Authority) (*sessionRuntime, error) {
	if factory == nil || repo == nil || a == nil || !validBinding(b) || q.Workspace != b.Workspace || q.Session != b.Session {
		return nil, command.ErrDenied
	}
	rebuild, e := recovery.New(repo)
	if e != nil {
		return nil, e
	}
	duplicate, e := persistence.New(persistence.Options{Factory: factory, Repository: repo, Request: func(wanted data.Binding) (install.SessionRequest, error) {
		if wanted != b {
			return install.SessionRequest{}, command.ErrDenied
		}
		return q, nil
	}})
	if e != nil {
		return nil, e
	}
	return &sessionRuntime{factory: factory, repo: repo, request: q, binding: b, authority: a, recovery: rebuild, duplicate: duplicate}, nil
}
func (p *sessionRuntime) Resolve(ctx context.Context, i command.Identity, e command.Envelope) (data.Receipt, bool, error) {
	return p.duplicate.Resolve(ctx, i, e)
}
func (p *sessionRuntime) Journal(ctx context.Context, b data.Binding, after uint64, limit int) (data.JournalPage, error) {
	if b != p.binding {
		return data.JournalPage{}, command.ErrDenied
	}
	return p.repo.ReadJournal(ctx, b, after, limit)
}
func (p *sessionRuntime) Open(ctx context.Context, b data.Binding) (result actor.Engine, err error) {
	if b != p.binding {
		return nil, command.ErrDenied
	}
	page, err := p.repo.ReadJournal(ctx, b, 0, 1)
	if err != nil {
		return nil, err
	}
	if page.Ended {
		return nil, actor.ErrEnded
	}
	if _, err = p.repo.ReadCreation(ctx, b); err != nil {
		return nil, err
	}
	s, err := p.factory.Recover(ctx, p.request, p.recovery.Build)
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
	return &sessionEngine{owner: p, session: s, version: page.Version, cursor: page.Cursor}, nil
}
func (e *sessionEngine) Version() uint64 { return e.version }
func (e *sessionEngine) Cursor() uint64  { return e.cursor }
func (e *sessionEngine) Execute(ctx context.Context, i command.Identity, c command.Envelope) (data.Receipt, error) {
	if i.Binding() != e.session.Binding {
		return data.Receipt{}, command.ErrDenied
	}
	inputs, err := e.owner.authority.InputsContext(ctx, i, c)
	if err != nil {
		return data.Receipt{}, err
	}
	callback := "command"
	if c.Type == "end" {
		callback = "on_session_end"
	}
	r, err := e.session.Commands.Execute(ctx, e.session.VM.Token(), hostapi.Command{Callback: callback, ID: c.CommandID, Principal: i.Principal(), ExpectedVersion: c.ExpectedStateVersion, Input: persistence.Input(c), Time: inputs.Time, Random: inputs.Random, Envelope: &data.EnvelopeMetadata{Seat: c.SeatID, Type: c.Type, Correlation: c.CorrelationID}})
	if err == nil && !r.Replayed {
		e.version = r.Version
		e.cursor = r.Cursor
	}
	return r, err
}
func (e *sessionEngine) read(ctx context.Context, principal, callback string, input checkpoint.Value) (data.Receipt, error) {
	var nonce [16]byte
	if _, err := rand.Read(nonce[:]); err != nil {
		return data.Receipt{}, command.ErrDenied
	}
	id := "read-" + hex.EncodeToString(nonce[:])
	clear(nonce[:])
	return e.session.Commands.Read(ctx, e.session.VM.Token(), hostapi.Command{Callback: callback, ID: id, Principal: principal, ExpectedVersion: e.version, Input: input, Time: time.Now().UnixMilli()})
}
func (e *sessionEngine) Project(ctx context.Context, i command.Identity) (checkpoint.Value, error) {
	if i.Binding() != e.session.Binding {
		return checkpoint.Value{}, command.ErrDenied
	}
	r, err := e.read(ctx, i.Principal(), "project_view", checkpoint.Object(map[string]checkpoint.Value{"seat_id": checkpoint.Text(i.Seat())}))
	return r.Result, err
}
func (e *sessionEngine) Checkpoint(ctx context.Context) error {
	r, err := e.read(ctx, "platform", "create_checkpoint", checkpoint.Object(map[string]checkpoint.Value{}))
	if err != nil {
		return err
	}
	return e.owner.recovery.Save(ctx, e.session.Recovery, r.Result, e.version, e.cursor)
}
func (e *sessionEngine) Close() error {
	e.closeOnce.Do(func() {
		defer func() { e.closeErr = errors.Join(e.closeErr, e.session.Close()) }()
		ctx, cancel := context.WithTimeout(context.Background(), time.Duration(e.session.VM.Limits().WallMillis)*time.Millisecond)
		defer cancel()
		_, e.closeErr = e.read(ctx, "platform", "cleanup", checkpoint.Object(map[string]checkpoint.Value{}))
	})
	return e.closeErr
}
