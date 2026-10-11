// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// Package vm owns one isolated runner process per long-lived Session VM. It has
// no Session command processing, database access, or Host Callback implementation.
package vm

import (
	"context"
	"crypto/rand"
	"crypto/subtle"
	"errors"
	"fmt"
	"sync"
	"time"

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
type Token struct{ capability **tokenCapability }

// Double indirection keeps fmt's invalid-verb fallback from dereferencing the
// private backing object. The nonce is immutable after publication; no public
// field or accessor exposes it, and no unsafe pointer or global registry is used.
type tokenCapability struct{ secret [32]byte }

func newExecutionToken() (Token, error) {
	data := &tokenCapability{}
	if _, err := rand.Read(data.secret[:]); err != nil {
		return Token{}, err
	}
	return Token{capability: &data}, nil
}

func (token Token) nonce() [32]byte {
	if token.capability == nil || *token.capability == nil {
		return [32]byte{}
	}
	return (**token.capability).secret
}

func (token Token) matches(other Token) bool {
	if token.capability == nil || other.capability == nil ||
		*token.capability == nil || *other.capability == nil {
		return false
	}
	one, two := token.nonce(), other.nonce()
	return subtle.ConstantTimeCompare(one[:], two[:]) == 1
}

func (Token) MarshalJSON() ([]byte, error) { return nil, profile.Fail(profile.ErrValue) }
func (Token) String() string               { return "<execution-token>" }
func (Token) GoString() string             { return "<execution-token>" }

// Format closes numeric and nested diagnostics as well as String/GoString.
// Ignore the requested width and verb so diagnostics stay bounded and opaque.
func (Token) Format(out fmt.State, _ rune) {
	_, _ = out.Write([]byte("<execution-token>"))
}

type AuditSink func(profile.Audit) error

type Options struct {
	SessionID    string
	Runner       string
	Launcher     ipc.Launcher
	Package      *archive.Package
	Dependencies []*archive.Package
	State        State
	Limits       profile.Limits
	Audit        AuditSink
	Fallbacks    map[string]FallbackProof
	Host         *HostOptions
}
type Session struct {
	sessionDiagnostics
	runtime **sessionRuntime
}

// The pointer target is itself a pointer, so even fmt's error fallback cannot
// expand the backing runtime. Public handle copies share this immutable pointer
// and the same lock; they never copy private state, credentials, or a mutex.
type sessionRuntime struct {
	mu         sync.Mutex
	client     ipc.Runner
	launcher   ipc.Launcher
	cleanupErr error
	reaped     bool
	runner     string
	config     profile.Config
	entry      []byte
	binding    checkpoint.Binding
	state      State
	token      Token
	audit      AuditSink
	poisoned   bool
	destroyed  bool
	sequence   uint64
	modules    map[string]ModuleIdentity
}

// runtimeState is private to this package. The factory sets runtime once and
// keeps its pointee for the full handle lifetime, including VM reconstruction.
func (s *Session) runtimeState() *sessionRuntime { return *s.runtime }

// sessionDiagnostics has no runtime fields. Promoting its value method protects
// both Session and *Session diagnostics.
type sessionDiagnostics struct{}

func (sessionDiagnostics) Format(out fmt.State, _ rune) {
	_, _ = out.Write([]byte("<session-vm:redacted>"))
}

func New(ctx context.Context, options Options) (*Session, error) {
	if options.Audit == nil {
		return nil, profile.Fail(profile.ErrConfiguration)
	}
	data := &sessionRuntime{runner: options.Runner, launcher: options.Launcher, audit: options.Audit}
	s := &Session{runtime: &data}
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
	s.runtimeState().config, s.runtimeState().entry, s.runtimeState().binding, s.runtimeState().state = config, entry, sealed.Binding, State{Version: options.State.Version, Value: sealed.State}
	s.runtimeState().modules, err = hostModuleIdentities(options)
	if err != nil {
		return reject(err)
	}
	s.runtimeState().token, err = newExecutionToken()
	if err != nil {
		return reject(err)
	}
	s.runtimeState().client, err = s.start(ctx, sealed.State, checkpoint.Value{Kind: "nil"})
	if err != nil {
		return nil, err
	}
	return s, nil
}

func (s *Session) start(ctx context.Context, state, saved checkpoint.Value) (ipc.Runner, error) {
	c, err := ipc.Launch(ctx, s.runtimeState().launcher, s.runtimeState().runner, s.runtimeState().config)
	if err != nil {
		if auditErr := s.record("runner-start", err); auditErr != nil {
			return nil, auditErr
		}
		return nil, err
	}
	if _, err = c.Call(ctx, ipc.Request{Operation: "state", State: &state, Saved: &saved}); err == nil {
		if s.runtimeState().config.Host != nil {
			_, err = c.Call(ctx, ipc.Request{Operation: "host-load"})
		} else {
			_, err = c.Call(ctx, ipc.Request{Operation: "execute", Source: s.runtimeState().entry})
		}
	}
	if auditErr := s.record("runner-start", err); auditErr != nil {
		err = auditErr
	}
	if err != nil {
		return nil, errors.Join(err, stop(c))
	}
	return c, nil
}
func (s *Session) record(kind string, err error) error {
	s.runtimeState().sequence++
	return s.runtimeState().audit(profile.Audit{Level: "AUDIT-0", Sequence: s.runtimeState().sequence, Kind: kind, Outcome: profile.Code(err)})
}
func (s *Session) Token() Token {
	s.runtimeState().mu.Lock()
	defer s.runtimeState().mu.Unlock()
	return s.runtimeState().token
}
func (s *Session) PID() int {
	s.runtimeState().mu.Lock()
	defer s.runtimeState().mu.Unlock()
	if s.runtimeState().client == nil {
		return 0
	}
	return s.runtimeState().client.PID()
}
func (s *Session) check(token Token) error {
	if s.runtimeState().destroyed {
		return profile.Fail(profile.ErrDestroyed)
	}
	if !token.matches(s.runtimeState().token) {
		return profile.Fail(profile.ErrCapability)
	}
	if s.runtimeState().poisoned {
		return profile.Fail(profile.ErrPoisoned)
	}
	return nil
}
func (s *Session) Execute(ctx context.Context, token Token, source []byte) (profile.Result, error) {
	s.runtimeState().mu.Lock()
	defer s.runtimeState().mu.Unlock()
	return s.execute(ctx, token, source)
}
func (s *Session) execute(ctx context.Context, token Token, source []byte) (profile.Result, error) {
	if err := s.check(token); err != nil {
		if auditErr := s.record("execution-denied", err); auditErr != nil {
			s.runtimeState().poisoned = true
			s.stopRunner()
			return profile.Result{}, errors.Join(auditErr, s.runtimeState().cleanupErr)
		}
		return profile.Result{}, err
	}
	var response ipc.Response
	err := profile.ValidateSource(source)
	if s.runtimeState().config.Host != nil {
		err = profile.Fail(profile.ErrCapability)
	}
	if err == nil {
		response, err = s.runtimeState().client.Call(ctx, ipc.Request{Operation: "execute", Source: append([]byte(nil), source...)})
	}
	if err != nil {
		s.runtimeState().poisoned = true
	}
	if auditErr := s.record("execution", err); auditErr != nil {
		err = auditErr
		s.runtimeState().poisoned = true
		s.stopRunner()
	}
	return response.Result, errors.Join(err, s.runtimeState().cleanupErr)
}

func (s *Session) Capture(ctx context.Context, token Token, source []byte) (checkpoint.Checkpoint, error) {
	s.runtimeState().mu.Lock()
	defer s.runtimeState().mu.Unlock()
	return s.capture(ctx, token, source)
}
func (s *Session) capture(ctx context.Context, token Token, source []byte) (checkpoint.Checkpoint, error) {
	result, err := s.execute(ctx, token, source)
	if err != nil {
		return checkpoint.Checkpoint{}, err
	}
	if len(result.Values) != 1 {
		s.runtimeState().poisoned = true
		err = profile.Fail(profile.ErrValue)
		if auditErr := s.record("checkpoint", err); auditErr != nil {
			s.stopRunner()
			err = auditErr
		}
		return checkpoint.Checkpoint{}, err
	}
	c, err := checkpoint.Seal(s.runtimeState().binding, result.Values[0])
	if err != nil {
		s.runtimeState().poisoned = true
	}
	if auditErr := s.record("checkpoint", err); auditErr != nil {
		err = auditErr
		s.runtimeState().poisoned = true
		s.stopRunner()
	}
	return c, errors.Join(err, s.runtimeState().cleanupErr)
}

// Reconstruct receives authoritative state, never a serialized VM. A supplied
// checkpoint must match the exact new state version and original package graph.
func (s *Session) Reconstruct(ctx context.Context, state State, saved *checkpoint.Checkpoint) error {
	s.runtimeState().mu.Lock()
	defer s.runtimeState().mu.Unlock()
	if s.runtimeState().destroyed {
		return profile.Fail(profile.ErrDestroyed)
	}
	return s.reconstruct(ctx, state, saved)
}
func (s *Session) reconstruct(ctx context.Context, state State, saved *checkpoint.Checkpoint) (resultErr error) {
	if s.runtimeState().cleanupErr != nil {
		return s.runtimeState().cleanupErr
	}
	defer func() {
		if err := s.record("reconstruction", resultErr); err != nil {
			s.runtimeState().poisoned = true
			s.stopRunner()
			resultErr = err
		}
	}()
	if state.Version < s.runtimeState().state.Version {
		return checkpoint.ErrRejected
	}
	b := s.runtimeState().binding
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
	token, err := newExecutionToken()
	if err != nil {
		return err
	}
	replacement, err := s.start(ctx, authoritative.State, value)
	if err != nil {
		return err
	}
	if e := s.stopRunner(); e != nil {
		s.runtimeState().poisoned = true
		return errors.Join(e, stop(replacement))
	}
	s.runtimeState().reaped = false
	s.runtimeState().client = replacement
	s.runtimeState().binding = authoritative.Binding
	s.runtimeState().state = State{Version: state.Version, Value: authoritative.State}
	s.runtimeState().token = token
	s.runtimeState().poisoned = false
	return nil
}

// RebuildForMemoryPressure first obtains a current checkpoint. A failed capture
// cannot replace the running VM or turn stale cache data into authoritative facts.
func (s *Session) RebuildForMemoryPressure(ctx context.Context, token Token, source []byte) (checkpoint.Checkpoint, error) {
	s.runtimeState().mu.Lock()
	defer s.runtimeState().mu.Unlock()
	saved, err := s.capture(ctx, token, source)
	if err != nil {
		return saved, err
	}
	return saved, s.reconstruct(ctx, s.runtimeState().state, &saved)
}
func (s *Session) Destroy() error {
	s.runtimeState().mu.Lock()
	defer s.runtimeState().mu.Unlock()
	if s.runtimeState().destroyed {
		return s.runtimeState().cleanupErr
	}
	s.runtimeState().destroyed = true
	s.runtimeState().token = Token{}
	if s.runtimeState().client != nil {
		s.stopRunner()
	}
	return errors.Join(s.runtimeState().cleanupErr, s.record("destroy", s.runtimeState().cleanupErr))
}

// IsBoundaryFailure distinguishes runner/environment loss from a script finding.
func IsBoundaryFailure(err error) bool {
	return errors.Is(err, ipc.ErrUnknownExit) || errors.Is(err, ipc.ErrRunner) || errors.Is(err, ipc.ErrProtocol) || errors.Is(err, context.DeadlineExceeded) || errors.Is(err, context.Canceled)
}

// stop requires acknowledgement from the actual process parent. A canceled
// command gets a separate bounded cleanup lifetime, never a fabricated Wait.
func stop(r ipc.Runner) error {
	if r == nil {
		return nil
	}
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()
	exit, err := r.StopAndWait(ctx)
	if err != nil || !exit.Reaped || exit.PID != r.PID() || exit.PID <= 0 {
		return errors.Join(ipc.ErrUnknownExit, err)
	}
	return nil
}
func (s *Session) stopRunner() error {
	err := stop(s.runtimeState().client)
	if err != nil {
		s.runtimeState().cleanupErr = errors.Join(s.runtimeState().cleanupErr, err)
		s.runtimeState().poisoned = true
	} else if s.runtimeState().cleanupErr == nil {
		s.runtimeState().reaped = true
	}
	return s.runtimeState().cleanupErr
}

// Reaped is independent of Destroyed: a failed remote close remains unknown.
func (s *Session) Reaped() bool {
	s.runtimeState().mu.Lock()
	defer s.runtimeState().mu.Unlock()
	return s.runtimeState().reaped && s.runtimeState().cleanupErr == nil
}
