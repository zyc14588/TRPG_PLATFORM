// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package vm

import (
	"context"
	"crypto/subtle"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/ipc"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/capability"
)

// HostOptions is a trusted Go configuration, never a manifest extension or Lua
// input. Every package receives its own declaration/trust/context intersection.
type HostOptions struct {
	Trust         map[string]capability.TrustLevel
	Policy        capability.TrustPolicy
	Execution     capability.GrantSet
	CallbackLimit int
}
type ModuleIdentity struct {
	PackageID    string
	ContentHash  string
	Trust        capability.TrustLevel
	Capabilities capability.GrantSet
}

func (s *Session) ModuleIdentity(module string) (ModuleIdentity, bool) {
	s.mu.Lock()
	defer s.mu.Unlock()
	x, ok := s.modules[module]
	return x, ok
}
func (s *Session) SessionID() string { s.mu.Lock(); defer s.mu.Unlock(); return s.binding.SessionID }
func (s *Session) GraphHash() string {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.binding.DependencyLock
}

// PackageHashes returns an owned copy of the entire authenticated graph,
// including packages without Lua. Callback authority remains in ModuleBindings.
func (s *Session) PackageHashes() map[string]string {
	s.mu.Lock()
	defer s.mu.Unlock()
	out := make(map[string]string, len(s.binding.PackageHashes))
	for id, hash := range s.binding.PackageHashes {
		out[id] = hash
	}
	return out
}
func (s *Session) StateVersion() uint64 { s.mu.Lock(); defer s.mu.Unlock(); return s.state.Version }

// Invoke only enters functions retained from the verified immutable graph.
// Token and SID are checked in the parent and never serialized into the child.
func (s *Session) Invoke(ctx context.Context, token Token, sid, name string, args []checkpoint.Value, handler profile.HostHandler) (profile.Result, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.invoke(ctx, token, sid, name, args, handler)
}
func (s *Session) invoke(ctx context.Context, token Token, sid, name string, args []checkpoint.Value, handler profile.HostHandler) (profile.Result, error) {
	if err := s.check(token); err != nil {
		return profile.Result{}, s.deniedHost(err)
	}
	if s.config.Host == nil || sid != s.binding.SessionID || handler == nil {
		return profile.Result{}, s.deniedHost(profile.Fail(profile.ErrCapability))
	}
	if name != "command" && (!profile.IsStandardCallback(name) || name == "validate_command" || name == "execute_command") {
		return profile.Result{}, s.deniedHost(profile.Fail(profile.ErrCapability))
	}
	guarded := func(callctx context.Context, c profile.HostCall) (checkpoint.Value, error) {
		m, ok := s.modules[c.Module]
		if !ok {
			return checkpoint.Value{}, profile.Fail(profile.ErrCapability)
		}
		cap, err := capability.ParseName(c.Capability)
		if err != nil || !m.Capabilities.Contains(cap) || !profile.KnownHostOperation(c.Capability, c.Operation) || c.Line < 1 || (c.Phase != "validate" && c.Phase != "execute" && c.Phase != "read") {
			return checkpoint.Value{}, profile.Fail(profile.ErrCapability)
		}
		return handler(callctx, c)
	}
	response, err := s.client.CallWithHost(ctx, ipc.Request{Operation: "host-invoke", Callback: name, Arguments: args}, guarded)
	if auditErr := s.record("host-"+name, err); auditErr != nil {
		err = auditErr
	}
	if err != nil {
		s.poisoned = true
		s.client.Kill()
		return profile.Result{}, err
	}
	return response.Result, nil
}
func (s *Session) deniedHost(err error) error {
	if e := s.record("host-denied", err); e != nil {
		s.poisoned = true
		if s.client != nil {
			s.client.Kill()
		}
		return e
	}
	return err
}

// Poison is called for Go validation, database and audit failures after Lua
// returns. Reconstruction from authoritative state is required before reuse.
func (s *Session) Poison() {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.poisoned = true
	if s.client != nil {
		s.client.Kill()
	}
}
func (s *Session) HostEnabled() bool { s.mu.Lock(); defer s.mu.Unlock(); return s.config.Host != nil }

func (s *Session) ModuleBindings() map[string]ModuleIdentity {
	s.mu.Lock()
	defer s.mu.Unlock()
	out := map[string]ModuleIdentity{}
	for k, v := range s.modules {
		out[k] = v
	}
	return out
}
func (s *Session) Limits() profile.Limits { s.mu.Lock(); defer s.mu.Unlock(); return s.config.Limits }
func (s *Session) AcceptCommittedState(version uint64, value checkpoint.Value) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	if version <= s.state.Version {
		return checkpoint.ErrRejected
	}
	b := s.binding
	b.StateVersion = version
	c, err := checkpoint.Seal(b, value)
	if err != nil {
		return err
	}
	s.binding = c.Binding
	s.state = State{Version: version, Value: c.State}
	return nil
}

// CaptureHost holds the VM generation/state binding across invocation and seal.
func (s *Session) CaptureHost(ctx context.Context, token Token, sid string, handler profile.HostHandler) (checkpoint.Checkpoint, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	result, err := s.invoke(ctx, token, sid, "create_checkpoint", nil, handler)
	if err != nil {
		return checkpoint.Checkpoint{}, err
	}
	if len(result.Values) != 1 {
		s.poisoned = true
		s.client.Kill()
		return checkpoint.Checkpoint{}, checkpoint.ErrRejected
	}
	return checkpoint.Seal(s.binding, result.Values[0])
}

func (s *Session) AuthorizeToken(token Token, sid string) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.destroyed || sid != s.binding.SessionID || subtle.ConstantTimeCompare(token.secret[:], s.token.secret[:]) != 1 {
		return s.deniedHost(profile.Fail(profile.ErrCapability))
	}
	return nil
}
