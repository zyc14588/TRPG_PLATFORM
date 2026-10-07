// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// Package actor serializes every command and management read for an active
// Session. A single bounded registry is owned by the platformd composition.
package actor

import (
	"context"
	"errors"
	"sync"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/outbox"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/command"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/realtime"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
)

var ErrBackpressure = errors.New("SESSION_MAILBOX_BACKPRESSURE")
var ErrClosed = errors.New("SESSION_ACTOR_CLOSED")
var ErrPanic = errors.New("SESSION_ACTOR_PANIC")
var ErrEnded = errors.New("SESSION_ENDED")

type Engine interface {
	Execute(context.Context, command.Identity, command.Envelope) (data.Receipt, error)
	Project(context.Context, command.Identity) (checkpoint.Value, error)
	Checkpoint(context.Context) error
	Version() uint64
	Cursor() uint64
	Close() error
}
type Backend interface {
	Open(context.Context, data.Binding) (Engine, error)
	Resolve(context.Context, command.Identity, command.Envelope) (data.Receipt, bool, error)
	Journal(context.Context, data.Binding, uint64, int) (data.JournalPage, error)
}
type Options struct {
	Authority            *command.Authority
	Hub                  *realtime.Hub
	Backend              Backend
	MaxSessions, Mailbox int
	Idle, CommandTimeout time.Duration
}
type Registry struct {
	mu      sync.Mutex
	options Options
	ctx     context.Context
	cancel  context.CancelFunc
	entries map[string]*entry
	closed  bool
}
type entry struct {
	owner           *Registry
	binding         data.Binding
	queue           chan request
	done            chan struct{}
	engine          Engine
	version, cursor uint64
}
type request struct {
	ctx        context.Context
	identity   command.Identity
	envelope   command.Envelope
	kind       string
	after      uint64
	connection *realtime.Connection
	reply      chan response
}
type response struct {
	receipt data.Receipt
	point   RecoveryPoint
	frame   realtime.Frame
	err     error
}

func New(ctx context.Context, o Options) (*Registry, error) {
	if ctx == nil || o.Authority == nil || o.Hub == nil || o.Backend == nil || o.MaxSessions < 1 || o.MaxSessions > 256 || o.Mailbox < 1 || o.Mailbox > 128 || o.Idle < time.Millisecond || o.Idle > time.Hour || o.CommandTimeout < time.Millisecond || o.CommandTimeout > 30*time.Second {
		return nil, ErrClosed
	}
	owned, cancel := context.WithCancel(ctx)
	return &Registry{options: o, ctx: owned, cancel: cancel, entries: map[string]*entry{}}, nil
}
func key(b data.Binding) string { return b.Workspace + "/" + b.Session }
func (r *Registry) dispatch(q request) (response, error) {
	if q.ctx == nil || r.options.Authority.VerifyContext(q.ctx, q.identity) != nil {
		return response{}, command.ErrDenied
	}
	if err := q.ctx.Err(); err != nil {
		return response{}, err
	}
	b := q.identity.Binding()
	r.mu.Lock()
	if r.closed || r.ctx.Err() != nil {
		r.mu.Unlock()
		return response{}, ErrClosed
	}
	e := r.entries[key(b)]
	if e == nil {
		if len(r.entries) >= r.options.MaxSessions {
			r.mu.Unlock()
			return response{}, ErrBackpressure
		}
		e = &entry{owner: r, binding: b, queue: make(chan request, r.options.Mailbox), done: make(chan struct{})}
		r.entries[key(b)] = e
		go e.run()
	} else if e.binding != b {
		r.mu.Unlock()
		return response{}, command.ErrDenied
	}
	q.reply = make(chan response, 1)
	select {
	case e.queue <- q:
		r.mu.Unlock()
	default:
		r.mu.Unlock()
		return response{}, ErrBackpressure
	}
	select {
	case <-q.ctx.Done():
		return response{}, q.ctx.Err()
	case <-r.ctx.Done():
		return response{}, ErrClosed
	case answer := <-q.reply:
		return answer, answer.err
	case <-e.done:
		select {
		case answer := <-q.reply:
			return answer, answer.err
		default:
			return response{}, ErrClosed
		}
	}
}
func (r *Registry) Submit(ctx context.Context, i command.Identity, e command.Envelope) (data.Receipt, error) {
	owned, err := r.options.Authority.ValidateContext(ctx, i, e)
	if err != nil {
		return data.Receipt{}, err
	}
	answer, err := r.dispatch(request{ctx: ctx, identity: i, envelope: owned, kind: "command"})
	return answer.receipt, err
}
func (r *Registry) Reconnect(ctx context.Context, i command.Identity, after uint64, c *realtime.Connection) (realtime.Frame, error) {
	if c != nil && (c.Identity().Binding() != i.Binding() || c.Identity().Principal() != i.Principal() || c.Identity().Seat() != i.Seat()) {
		return realtime.Frame{}, command.ErrDenied
	}
	answer, err := r.dispatch(request{ctx: ctx, identity: i, kind: "reconnect", after: after, connection: c})
	return answer.frame, err
}
func (r *Registry) Sleep(ctx context.Context, i command.Identity) error {
	_, err := r.dispatch(request{ctx: ctx, identity: i, kind: "sleep"})
	return err
}
func (r *Registry) Close() error {
	r.mu.Lock()
	r.closed = true
	r.cancel()
	entries := []*entry{}
	for _, e := range r.entries {
		entries = append(entries, e)
	}
	r.mu.Unlock()
	for _, e := range entries {
		<-e.done
	}
	r.options.Hub.Close()
	return nil
}
func (e *entry) closeEngine() (err error) {
	defer func() {
		if recover() != nil {
			err = ErrPanic
		}
	}()
	if e.engine == nil {
		return nil
	}
	engine := e.engine
	e.engine = nil
	return engine.Close()
}
func (e *entry) idleSleep(ctx context.Context) (err error) {
	defer func() {
		if recover() != nil {
			_ = e.closeEngine()
			err = ErrPanic
		}
	}()
	if e.engine == nil {
		return nil
	}
	err = e.engine.Checkpoint(ctx)
	return errors.Join(err, e.closeEngine())
}
func (e *entry) open(ctx context.Context) error {
	if e.engine != nil {
		return nil
	}
	engine, err := e.owner.options.Backend.Open(ctx, e.binding)
	if err != nil {
		return err
	}
	if engine == nil {
		return ErrClosed
	}
	e.engine = engine
	e.version = engine.Version()
	e.cursor = engine.Cursor()
	return nil
}
func (e *entry) safely(q request) (answer response, end bool) {
	defer func() {
		if recover() != nil {
			_ = e.closeEngine()
			answer = response{err: ErrPanic}
			end = false
		}
	}()
	r := e.owner
	ctx, cancel := context.WithTimeout(q.ctx, r.options.CommandTimeout)
	stop := context.AfterFunc(r.ctx, cancel)
	defer cancel()
	defer stop()
	if err := r.options.Authority.VerifyContext(ctx, q.identity); err != nil {
		return response{err: err}, false
	}
	if err := ctx.Err(); err != nil {
		return response{err: err}, false
	}
	if q.kind == "command" {
		if _, err := r.options.Authority.ValidateContext(ctx, q.identity, q.envelope); err != nil {
			return response{err: err}, false
		}
		saved, found, err := r.options.Backend.Resolve(ctx, q.identity, q.envelope)
		if err != nil {
			return response{err: err}, false
		}
		if found {
			return response{receipt: saved}, false
		}
	}
	if err := e.open(ctx); err != nil {
		return response{err: err}, false
	}
	switch q.kind {
	case "command":
		receipt, err := e.engine.Execute(ctx, q.identity, q.envelope)
		if err != nil {
			_ = e.closeEngine()
			return response{err: err}, false
		}
		if receipt.Replayed {
			return response{receipt: receipt}, false
		}
		n, err := outbox.FromCommitted(receipt)
		if err != nil {
			_ = e.closeEngine()
			return response{err: data.ErrUnknownCommit}, false
		}
		if n.Binding != e.binding || receipt.Header.Principal != q.identity.Principal() || receipt.Header.CommandID != q.envelope.CommandID || receipt.Header.ExpectedVersion != q.envelope.ExpectedStateVersion {
			_ = e.closeEngine()
			return response{err: data.ErrUnknownCommit}, false
		}
		// Engine.Execute returns only after the authoritative SQL commit. All
		// memory advancement and publication occur below that boundary.
		e.version = receipt.Version
		e.cursor = receipt.Cursor
		if err = r.options.Hub.Publish(ctx, n, e.engine.Project); err != nil {
			_ = e.closeEngine()
		}
		end = q.envelope.Type == "end"
		if end {
			_ = e.closeEngine()
			r.options.Hub.CloseSession(e.binding)
		}
		return response{receipt: receipt}, end
	case "reconnect":
		page, err := r.options.Backend.Journal(ctx, e.binding, q.after, 128)
		if err != nil {
			return response{err: err}, false
		}
		if page.Ended {
			return response{err: ErrEnded}, false
		}
		view, err := e.engine.Project(ctx, q.identity)
		if err != nil {
			_ = e.closeEngine()
			return response{err: err}, false
		}
		policy, err := r.options.Authority.PolicyContext(ctx, q.identity)
		if err != nil {
			return response{err: err}, false
		}
		frame := realtime.Frame{Kind: "reconnect", Session: e.binding.Session, Version: page.Version, Cursor: page.Cursor, View: realtime.Filter(view, policy.ViewFields), Events: realtime.FilterEvents(page.Events, policy)}
		if q.connection != nil {
			if err = r.options.Hub.EnqueueContext(ctx, q.connection, frame); err != nil {
				return response{err: err}, false
			}
		}
		return response{frame: frame}, false
	case "recovery-point":
		if err := r.options.Authority.CheckRecoveryPoint(ctx, q.identity); err != nil {
			return response{err: err}, false
		}
		point := RecoveryPoint{Version: e.engine.Version(), Cursor: e.engine.Cursor()}
		err := e.engine.Checkpoint(ctx)
		closeErr := e.closeEngine()
		return response{point: point, err: errors.Join(err, closeErr)}, false
	case "sleep":
		err := e.engine.Checkpoint(ctx)
		closeErr := e.closeEngine()
		return response{err: errors.Join(err, closeErr)}, false
	default:
		return response{err: ErrClosed}, false
	}
}
func (e *entry) run() {
	r := e.owner
	timer := time.NewTimer(r.options.Idle)
	defer timer.Stop()
	defer func() {
		_ = e.closeEngine()
		r.mu.Lock()
		if r.entries[key(e.binding)] == e {
			delete(r.entries, key(e.binding))
		}
		r.mu.Unlock()
		close(e.done)
	}()
	for {
		select {
		case <-r.ctx.Done():
			return
		case <-timer.C:
			if e.engine != nil {
				ctx, cancel := context.WithTimeout(r.ctx, r.options.CommandTimeout)
				_ = e.idleSleep(ctx)
				cancel()
			}

			// Reclaim idle registry capacity only while no accepted request is
			// queued. Dispatch uses the same lock, so a new writer cannot open
			// until this runtime has been closed and removed.
			r.mu.Lock()
			retire := len(e.queue) == 0 && r.entries[key(e.binding)] == e
			if retire {
				delete(r.entries, key(e.binding))
			}
			r.mu.Unlock()
			if retire {
				return
			}
			timer.Reset(r.options.Idle)
		case q := <-e.queue:
			if !timer.Stop() {
				select {
				case <-timer.C:
				default:
				}
			}
			answer, end := e.safely(q)
			if end {
				r.mu.Lock()
				if r.entries[key(e.binding)] == e {
					delete(r.entries, key(e.binding))
				}
				r.mu.Unlock()
			}
			q.reply <- answer
			if end {
				return
			}
			timer.Reset(r.options.Idle)
		}
	}
}
