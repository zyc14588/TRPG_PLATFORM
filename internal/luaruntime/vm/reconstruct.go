// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package vm

import (
	"context"
	"fmt"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
)

func bindingMatchesConfig(binding checkpoint.Binding, config Config) bool {
	return binding.SessionID == config.SessionID &&
		binding.LuaProfile == config.ProfileID &&
		binding.RuntimeVersion == config.RuntimeVersion
}

// CreateCheckpoint serializes only the explicit value supplied by the caller.
// Globals, coroutine stacks, functions and VM memory are never inspected.
func (v *VM) CreateCheckpoint(ctx context.Context, binding checkpoint.Binding, explicitState any) ([]byte, error) {
	if err := v.acquire(ctx); err != nil {
		return nil, err
	}
	defer v.release()
	if !bindingMatchesConfig(binding, v.config) {
		return nil, ErrBindingMismatch
	}
	return checkpoint.Marshal(binding, explicitState)
}

// Reconstruct validates all explicit inputs before allocating the replacement
// VM. The result depends only on authoritativeState and the compatible
// checkpoint; it does not reference an old VM or any process-local cache.
func Reconstruct(ctx context.Context, config Config, authoritativeState any, encoded []byte, expected checkpoint.Binding) (*VM, error) {
	if ctx == nil {
		ctx = context.Background()
	}
	if err := ctx.Err(); err != nil {
		return nil, err
	}
	if _, err := validateConfig(config); err != nil {
		return nil, err
	}
	if !bindingMatchesConfig(expected, config) {
		return nil, ErrBindingMismatch
	}
	authoritative, err := checkpoint.FromGo(authoritativeState)
	if err != nil {
		return nil, fmt.Errorf("authoritative state: %w", err)
	}
	authoritative, err = checkpoint.Normalize(authoritative)
	if err != nil {
		return nil, fmt.Errorf("authoritative state: %w", err)
	}
	decoded, err := checkpoint.Unmarshal(encoded, expected)
	if err != nil {
		return nil, err
	}
	replacement, err := New(config)
	if err != nil {
		return nil, err
	}
	replacement.recovery = &RecoveryState{Authoritative: authoritative, Checkpoint: decoded.State}
	return replacement, nil
}

// Recovery returns defensive canonical copies of the explicit inputs used to
// reconstruct this VM.
func (v *VM) Recovery(ctx context.Context) (RecoveryState, error) {
	if err := v.acquire(ctx); err != nil {
		return RecoveryState{}, err
	}
	defer v.release()
	if v.recovery == nil {
		return RecoveryState{}, nil
	}
	authoritative, err := checkpoint.Normalize(v.recovery.Authoritative)
	if err != nil {
		return RecoveryState{}, err
	}
	restored, err := checkpoint.Normalize(v.recovery.Checkpoint)
	if err != nil {
		return RecoveryState{}, err
	}
	return RecoveryState{Authoritative: authoritative, Checkpoint: restored}, nil
}
