// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// Package vm owns one isolated runner process per long-lived Session VM. It has
// no Session command processing, database access, or Host Callback implementation.
package vm

import (
	"context"
	"crypto/rand"
	"crypto/subtle"
	"errors"
	"sync"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/ipc"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/archive"
)

type State struct {
	Version uint64
	Value   checkpoint.Value
}

// Token is an opaque execution capability bound to a single VM generation. It
// never enters Lua, IPC payloads, checkpoints or ordinary log output.
type Token struct{ secret [32]byte }

func (Token) MarshalJSON() ([]byte, error) { return nil, profile.Fail(profile.ErrValue) }
func (Token) String() string               { return "<execution-token>" }
func (Token) GoString() string             { return "<execution-token>" }

type AuditSink func(profile.Audit) error

type Options struct {
	SessionID    string
	Runner       string
	Package      *archive.Package
	Dependencies []*archive.Package
	State        State
	Limits       profile.Limits
	Audit        AuditSink
	Fallbacks    map[string]FallbackProof
}
type Session struct {
	mu        sync.Mutex
	client    *ipc.Client
	runner    string
	config    profile.Config
	entry     []byte
	binding   checkpoint.Binding
	state     State
	token     Token
	audit     AuditSink
	poisoned  bool
	destroyed bool
	sequence  uint64
}

func New(ctx context.Context, options Options) (*Session, error) {
	if options.Audit == nil {
		return nil, profile.Fail(profile.ErrConfiguration)
	}
	s := &Session{runner: options.Runner, audit: options.Audit}
	reject := func(err error) (*Session, error) {
		if auditErr := s.record("initialization-denied", err); auditErr != nil {
			return nil, auditErr
		}
		return nil, err
	}
	binding, config, entry, err := adapt(options)
	if err != nil {
		return reject(err)
	}
	sealed, err := checkpoint.Seal(binding, options.State.Value)
	if err != nil {
		return reject(err)
	}
	s.config, s.entry, s.binding, s.state = config, entry, sealed.Binding, State{Version: options.State.Version, Value: sealed.State}
	if _, err := rand.Read(s.token.secret[:]); err != nil {
		return reject(err)
	}
	s.client, err = s.start(ctx, sealed.State, checkpoint.Value{Kind: "nil"})
	if err != nil {
		return nil, err
	}
	return s, nil
}

func (s *Session) start(ctx context.Context, state, saved checkpoint.Value) (*ipc.Client, error) {
	c, err := ipc.Start(ctx, s.runner, s.config)
	if err != nil {
		if auditErr := s.record("runner-start", err); auditErr != nil {
			return nil, auditErr
		}
		return nil, err
	}
	if _, err = c.Call(ctx, ipc.Request{Operation: "state", State: &state, Saved: &saved}); err == nil {
		_, err = c.Call(ctx, ipc.Request{Operation: "execute", Source: s.entry})
	}
	if auditErr := s.record("runner-start", err); auditErr != nil {
		err = auditErr
	}
	if err != nil {
		c.Kill()
		return nil, err
	}
	return c, nil
}
func (s *Session) record(kind string, err error) error {
	s.sequence++
	return s.audit(profile.Audit{Level: "AUDIT-0", Sequence: s.sequence, Kind: kind, Outcome: profile.Code(err)})
}
func (s *Session) Token() Token { s.mu.Lock(); defer s.mu.Unlock(); return s.token }
func (s *Session) PID() int {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.client == nil {
		return 0
	}
	return s.client.PID()
}
func (s *Session) check(token Token) error {
	if s.destroyed {
		return profile.Fail(profile.ErrDestroyed)
	}
	if subtle.ConstantTimeCompare(token.secret[:], s.token.secret[:]) != 1 {
		return profile.Fail(profile.ErrCapability)
	}
	if s.poisoned {
		return profile.Fail(profile.ErrPoisoned)
	}
	return nil
}
func (s *Session) Execute(ctx context.Context, token Token, source []byte) (profile.Result, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.execute(ctx, token, source)
}
func (s *Session) execute(ctx context.Context, token Token, source []byte) (profile.Result, error) {
	if err := s.check(token); err != nil {
		if auditErr := s.record("execution-denied", err); auditErr != nil {
			s.poisoned = true
			s.client.Kill()
			return profile.Result{}, auditErr
		}
		return profile.Result{}, err
	}
	var response ipc.Response
	err := profile.ValidateSource(source)
	if err == nil {
		response, err = s.client.Call(ctx, ipc.Request{Operation: "execute", Source: append([]byte(nil), source...)})
	}
	if err != nil {
		s.poisoned = true
	}
	if auditErr := s.record("execution", err); auditErr != nil {
		err = auditErr
		s.poisoned = true
		s.client.Kill()
	}
	return response.Result, err
}

func (s *Session) Capture(ctx context.Context, token Token, source []byte) (checkpoint.Checkpoint, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.capture(ctx, token, source)
}
func (s *Session) capture(ctx context.Context, token Token, source []byte) (checkpoint.Checkpoint, error) {
	result, err := s.execute(ctx, token, source)
	if err != nil {
		return checkpoint.Checkpoint{}, err
	}
	if len(result.Values) != 1 {
		s.poisoned = true
		err = profile.Fail(profile.ErrValue)
		if auditErr := s.record("checkpoint", err); auditErr != nil {
			s.client.Kill()
			err = auditErr
		}
		return checkpoint.Checkpoint{}, err
	}
	c, err := checkpoint.Seal(s.binding, result.Values[0])
	if err != nil {
		s.poisoned = true
	}
	if auditErr := s.record("checkpoint", err); auditErr != nil {
		err = auditErr
		s.poisoned = true
		s.client.Kill()
	}
	return c, err
}

// Reconstruct receives authoritative state, never a serialized VM. A supplied
// checkpoint must match the exact new state version and original package graph.
func (s *Session) Reconstruct(ctx context.Context, state State, saved *checkpoint.Checkpoint) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.destroyed {
		return profile.Fail(profile.ErrDestroyed)
	}
	return s.reconstruct(ctx, state, saved)
}
func (s *Session) reconstruct(ctx context.Context, state State, saved *checkpoint.Checkpoint) (resultErr error) {
	defer func() {
		if err := s.record("reconstruction", resultErr); err != nil {
			s.poisoned = true
			s.client.Kill()
			resultErr = err
		}
	}()
	if state.Version < s.state.Version {
		return checkpoint.ErrRejected
	}
	b := s.binding
	b.StateVersion = state.Version
	authoritative, err := checkpoint.Seal(b, state.Value)
	if err != nil {
		return err
	}
	value := checkpoint.Value{Kind: "nil"}
	if saved != nil {
		raw, err := checkpoint.Encode(*saved)
		if err != nil {
			return err
		}
		c, err := checkpoint.Decode(raw, b)
		if err != nil {
			return err
		}
		value = c.State
	}
	var token Token
	if _, err := rand.Read(token.secret[:]); err != nil {
		return err
	}
	replacement, err := s.start(ctx, authoritative.State, value)
	if err != nil {
		return err
	}
	s.client.Kill()
	s.client = replacement
	s.binding = authoritative.Binding
	s.state = State{Version: state.Version, Value: authoritative.State}
	s.token = token
	s.poisoned = false
	return nil
}

// RebuildForMemoryPressure first obtains a current checkpoint. A failed capture
// cannot replace the running VM or turn stale cache data into authoritative facts.
func (s *Session) RebuildForMemoryPressure(ctx context.Context, token Token, source []byte) (checkpoint.Checkpoint, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	saved, err := s.capture(ctx, token, source)
	if err != nil {
		return saved, err
	}
	return saved, s.reconstruct(ctx, s.state, &saved)
}
func (s *Session) Destroy() error {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.destroyed {
		return nil
	}
	s.destroyed = true
	s.token = Token{}
	if s.client != nil {
		s.client.Kill()
	}
	return s.record("destroy", nil)
}

// IsBoundaryFailure distinguishes runner/environment loss from a script finding.
func IsBoundaryFailure(err error) bool {
	return errors.Is(err, ipc.ErrRunner) || errors.Is(err, ipc.ErrProtocol) || errors.Is(err, context.DeadlineExceeded) || errors.Is(err, context.Canceled)
}
