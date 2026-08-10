// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package vm

import (
	"context"
	"errors"
	"fmt"
	"strings"
	"unicode/utf8"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
)

const MaxReconstructionSourceBytes = 1 << 20

var ErrInvalidReconstructionProgram = errors.New("invalid Lua reconstruction program")

// ReconstructionProgram is one explicit deterministic source input. Runtime
// source is loaded first; restore source then applies the two standard recovery
// globals to package-defined observable runtime state.
type ReconstructionProgram struct {
	Name   string
	Source string
}

// ReconstructionInput is the complete non-checkpoint input to reconstruction.
// It intentionally contains no VM pointer, package store, filesystem path,
// database handle, network client, or implicit source-discovery mechanism.
type ReconstructionInput struct {
	Authoritative  any
	RuntimeProgram ReconstructionProgram
	RestoreProgram ReconstructionProgram
}

// ValidateReconstructionProgram enforces the source-only bounded input contract
// before a replacement VM is allocated.
func ValidateReconstructionProgram(program ReconstructionProgram) error {
	if strings.TrimSpace(program.Name) == "" || len(program.Name) > 512 || !utf8.ValidString(program.Name) || strings.ContainsRune(program.Name, '\x00') {
		return fmt.Errorf("%w: invalid chunk name", ErrInvalidReconstructionProgram)
	}
	if len(program.Source) == 0 || len(program.Source) > MaxReconstructionSourceBytes {
		return fmt.Errorf("%w: source size", ErrInvalidReconstructionProgram)
	}
	if err := profile.ValidateSource([]byte(program.Source)); err != nil {
		return fmt.Errorf("%w: %w", ErrInvalidReconstructionProgram, err)
	}
	return nil
}

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

// Reconstruct creates a fresh VM, deterministically loads explicit runtime and
// restore source, and applies authoritative plus checkpoint state through the
// standard recovery globals. No old VM or process-local cache participates.
func Reconstruct(ctx context.Context, config Config, input ReconstructionInput, encoded []byte, expected checkpoint.Binding) (*VM, error) {
	return reconstruct(ctx, config, input, encoded, expected, New)
}

type reconstructionFactory func(Config) (*VM, error)

func reconstruct(ctx context.Context, config Config, input ReconstructionInput, encoded []byte, expected checkpoint.Binding, factory reconstructionFactory) (*VM, error) {
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
	if err := ValidateReconstructionProgram(input.RuntimeProgram); err != nil {
		return nil, fmt.Errorf("runtime program: %w", err)
	}
	if err := ValidateReconstructionProgram(input.RestoreProgram); err != nil {
		return nil, fmt.Errorf("restore program: %w", err)
	}
	authoritative, err := checkpoint.FromGo(input.Authoritative)
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
	replacement, err := factory(config)
	if err != nil {
		return nil, err
	}
	keep := false
	defer func() {
		if !keep {
			_ = replacement.Destroy(context.Background())
		}
	}()
	if _, err := replacement.runtime.Execute(input.RuntimeProgram.Name, []byte(input.RuntimeProgram.Source)); err != nil {
		return nil, fmt.Errorf("execute runtime program: %w", err)
	}
	if err := ctx.Err(); err != nil {
		return nil, err
	}
	// Install the canonical pair after package/runtime initialization so source
	// cannot pre-populate or tamper with the standard recovery inputs.
	if err := replacement.runtime.InstallRecoveryInputs(authoritative, decoded.State); err != nil {
		return nil, err
	}
	if _, err := replacement.runtime.Execute(input.RestoreProgram.Name, []byte(input.RestoreProgram.Source)); err != nil {
		return nil, fmt.Errorf("execute restore program: %w", err)
	}
	if err := ctx.Err(); err != nil {
		return nil, err
	}
	replacement.recovery = &RecoveryState{Authoritative: authoritative, Checkpoint: decoded.State}
	keep = true
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
