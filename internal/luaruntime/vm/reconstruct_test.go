// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package vm

import (
	"bytes"
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"reflect"
	"strings"
	"testing"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
)

const vmCanonicalLock = `{"schema_version":1,"root":"publisher/rules","packages":[{"package_id":"publisher/rules","version":"1.2.3","content_hash":"sha256:` + packageHash + `","features":[],"dependencies":[]}]}`

func binding(sessionID string) checkpoint.Binding {
	config := ProductionConfig(sessionID)
	return checkpoint.Binding{
		SessionID:      sessionID,
		StateVersion:   12,
		PackageHashes:  []checkpoint.PackageHash{{PackageID: "publisher/rules", SHA256: packageHash}},
		DependencyLock: vmCanonicalLock,
		LuaProfile:     config.ProfileID,
		RuntimeVersion: config.RuntimeVersion,
	}
}

func reconstructionInput(authoritative any) ReconstructionInput {
	return ReconstructionInput{
		Authoritative: authoritative,
		RuntimeProgram: ReconstructionProgram{
			Name: "runtime:publisher/rules",
			Source: `runtime_program_loaded = true
old_global_was_absent = hidden_vm_only == nil
old_coroutine_was_absent = old_coroutine == nil
__trpg_authoritative_state = {authoritative_marker = -1}
__trpg_checkpoint_state = {restored_marker = -1}
math.randomseed(445566)
recovery_random_marker = math.random(1, 1000000000)`,
		},
		RestoreProgram: ReconstructionProgram{
			Name: "restore:publisher/rules",
			Source: fmt.Sprintf(`local authoritative = %s
local explicit = %s
if type(authoritative) ~= "table" or type(explicit) ~= "table" then
  error("recovery inputs must be tables")
end
authoritative_marker = authoritative.authoritative_marker
restored_marker = explicit.restored_marker
restored_sum = authoritative_marker + restored_marker`, profile.RecoveryAuthoritativeGlobal, profile.RecoveryCheckpointGlobal),
		},
	}
}

func reconstructedValue(t *testing.T, runtime *VM, source string) checkpoint.Value {
	t.Helper()
	value, err := runtime.Eval(context.Background(), "observe-recovery", []byte(source))
	if err != nil {
		t.Fatalf("observe reconstructed VM: %v", err)
	}
	return value
}

func TestCheckpointReconstructionRestoresObservableStateAndIsIndependent(t *testing.T) {
	ctx := context.Background()
	original := mustVM(t, "session-recovery")
	if _, err := original.Eval(ctx, "pollution", []byte(`hidden_vm_only = 999
old_coroutine = coroutine.create(function() coroutine.yield(1) end)
coroutine.resume(old_coroutine)
math.randomseed(123)
math.random()
return hidden_vm_only`)); err != nil {
		t.Fatal(err)
	}
	if err := original.LoadModule(ctx, "old-module", []byte(`return {old = true}`)); err != nil {
		t.Fatal(err)
	}
	oldHandle, err := original.IssueCapability(ctx, "state.read")
	if err != nil {
		t.Fatal(err)
	}
	checkpointState := map[string]any{"restored_marker": int64(42)}
	encoded, err := original.CreateCheckpoint(ctx, binding("session-recovery"), checkpointState)
	if err != nil {
		t.Fatal(err)
	}
	input := reconstructionInput(map[string]any{"authoritative_marker": int64(8)})

	first, err := Reconstruct(ctx, ProductionConfig("session-recovery"), input, encoded, binding("session-recovery"))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = first.Destroy(ctx) })
	second, err := Reconstruct(ctx, ProductionConfig("session-recovery"), input, encoded, binding("session-recovery"))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = second.Destroy(ctx) })

	for _, replacement := range []*VM{first, second} {
		if got := valueInt(t, reconstructedValue(t, replacement, "return restored_marker")); got != 42 {
			t.Fatalf("restored marker = %d, want 42", got)
		}
		if got := valueInt(t, reconstructedValue(t, replacement, "return authoritative_marker")); got != 8 {
			t.Fatalf("authoritative marker = %d, want 8", got)
		}
		if !valueBool(t, reconstructedValue(t, replacement, "return runtime_program_loaded and old_global_was_absent and old_coroutine_was_absent")) {
			t.Fatal("fresh runtime inherited an old global or coroutine")
		}
		if loaded, err := replacement.HasModule(ctx, "old-module"); err != nil || loaded {
			t.Fatalf("fresh runtime inherited module cache: loaded=%v err=%v", loaded, err)
		}
		if err := replacement.CheckCapability(ctx, oldHandle, "state.read"); !errors.Is(err, ErrForeignCapability) {
			t.Fatalf("fresh runtime accepted old capability: %v", err)
		}
	}
	firstRandom := valueInt(t, reconstructedValue(t, first, "return recovery_random_marker"))
	secondRandom := valueInt(t, reconstructedValue(t, second, "return recovery_random_marker"))
	if firstRandom != secondRandom {
		t.Fatalf("deterministic runtime programs produced random markers %d and %d", firstRandom, secondRandom)
	}

	firstState, err := first.Recovery(ctx)
	if err != nil {
		t.Fatal(err)
	}
	secondState, err := second.Recovery(ctx)
	if err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(firstState, secondState) {
		t.Fatalf("reconstruction differs: first=%#v second=%#v", firstState, secondState)
	}
	reencoded, err := first.CreateCheckpoint(ctx, binding("session-recovery"), firstState.Checkpoint)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(encoded, reencoded) {
		t.Fatal("reconstructed checkpoint did not encode deterministically")
	}

	if err := original.Destroy(ctx); err != nil {
		t.Fatal(err)
	}
	afterDestroy, err := Reconstruct(ctx, ProductionConfig("session-recovery"), input, encoded, binding("session-recovery"))
	if err != nil {
		t.Fatalf("destroyed old VM affected reconstruction: %v", err)
	}
	t.Cleanup(func() { _ = afterDestroy.Destroy(ctx) })
	if got := valueInt(t, reconstructedValue(t, afterDestroy, "return restored_sum")); got != 50 {
		t.Fatalf("post-destroy restored sum = %d", got)
	}

	poisoned := mustVM(t, "session-poisoned-old")
	if _, err := poisoned.Eval(ctx, "poison", []byte(`error("poison")`)); err == nil {
		t.Fatal("poisoning source succeeded")
	}
	poisonedEncoded, err := checkpoint.Marshal(binding("session-poisoned-old"), checkpointState)
	if err != nil {
		t.Fatal(err)
	}
	fromPoisoned, err := Reconstruct(ctx, ProductionConfig("session-poisoned-old"), reconstructionInput(map[string]any{"authoritative_marker": int64(8)}), poisonedEncoded, binding("session-poisoned-old"))
	if err != nil {
		t.Fatalf("poisoned old VM affected reconstruction: %v", err)
	}
	t.Cleanup(func() { _ = fromPoisoned.Destroy(ctx) })
	if got := valueInt(t, reconstructedValue(t, fromPoisoned, "return restored_sum")); got != 50 {
		t.Fatalf("poison-independent restored sum = %d", got)
	}
}

func TestAuthoritativeAndCheckpointInputsIndependentlyAffectRecovery(t *testing.T) {
	ctx := context.Background()
	makeCheckpoint := func(marker int64) []byte {
		encoded, err := checkpoint.Marshal(binding("session-participation"), map[string]any{"restored_marker": marker})
		if err != nil {
			t.Fatal(err)
		}
		return encoded
	}
	reconstruct := func(authoritative, restored int64) *VM {
		replacement, err := Reconstruct(
			ctx,
			ProductionConfig("session-participation"),
			reconstructionInput(map[string]any{"authoritative_marker": authoritative}),
			makeCheckpoint(restored),
			binding("session-participation"),
		)
		if err != nil {
			t.Fatal(err)
		}
		t.Cleanup(func() { _ = replacement.Destroy(ctx) })
		return replacement
	}
	base := valueInt(t, reconstructedValue(t, reconstruct(7, 42), "return authoritative_marker * 1000 + restored_marker"))
	authoritativeChanged := valueInt(t, reconstructedValue(t, reconstruct(8, 42), "return authoritative_marker * 1000 + restored_marker"))
	checkpointChanged := valueInt(t, reconstructedValue(t, reconstruct(7, 99), "return authoritative_marker * 1000 + restored_marker"))
	if base != 7042 || authoritativeChanged != 8042 || checkpointChanged != 7099 {
		t.Fatalf("recovery participation = %d, %d, %d", base, authoritativeChanged, checkpointChanged)
	}
}

func TestReconstructionRejectsEveryCompatibilityMutation(t *testing.T) {
	ctx := context.Background()
	encoded, err := checkpoint.Marshal(binding("session-matrix"), map[string]any{"restored_marker": int64(42)})
	if err != nil {
		t.Fatal(err)
	}
	tests := []struct {
		name     string
		config   Config
		expected checkpoint.Binding
		encoded  []byte
	}{
		{name: "wrong SessionID", config: ProductionConfig("session-other"), expected: binding("session-other"), encoded: encoded},
		{name: "wrong state version", config: ProductionConfig("session-matrix"), expected: func() checkpoint.Binding { value := binding("session-matrix"); value.StateVersion++; return value }(), encoded: encoded},
		{name: "wrong package hashes", config: ProductionConfig("session-matrix"), expected: func() checkpoint.Binding {
			value := binding("session-matrix")
			value.PackageHashes[0].SHA256 = strings.Repeat("b", 64)
			return value
		}(), encoded: encoded},
		{name: "wrong exact lock", config: ProductionConfig("session-matrix"), expected: func() checkpoint.Binding {
			value := binding("session-matrix")
			value.DependencyLock = strings.Replace(value.DependencyLock, `"version":"1.2.3"`, `"version":"1.2.4"`, 1)
			return value
		}(), encoded: encoded},
		{name: "wrong Lua profile", config: ProductionConfig("session-matrix"), expected: func() checkpoint.Binding {
			value := binding("session-matrix")
			value.LuaProfile = "platform-lua-5.5-p2"
			return value
		}(), encoded: encoded},
		{name: "wrong runtime", config: ProductionConfig("session-matrix"), expected: func() checkpoint.Binding {
			value := binding("session-matrix")
			value.RuntimeVersion = "latest"
			return value
		}(), encoded: encoded},
		{name: "corrupt checkpoint", config: ProductionConfig("session-matrix"), expected: binding("session-matrix"), encoded: func() []byte { value := append([]byte(nil), encoded...); value[len(value)/2] ^= 1; return value }()},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			replacement, err := Reconstruct(ctx, tt.config, reconstructionInput(map[string]any{"authoritative_marker": int64(8)}), tt.encoded, tt.expected)
			if replacement != nil {
				_ = replacement.Destroy(ctx)
				t.Fatal("incompatible reconstruction returned a VM")
			}
			if err == nil {
				t.Fatal("incompatible reconstruction succeeded")
			}
		})
	}
}

type rawCheckpointPayload struct {
	FormatVersion int                `json:"format_version"`
	Binding       checkpoint.Binding `json:"binding"`
	State         checkpoint.Value   `json:"state"`
}

type rawCheckpointDocument struct {
	Payload       rawCheckpointPayload `json:"payload"`
	PayloadSHA256 string               `json:"payload_sha256"`
}

func invalidValueCheckpoint(t *testing.T, sessionID string) []byte {
	t.Helper()
	payload := rawCheckpointPayload{
		FormatVersion: checkpoint.FormatVersion,
		Binding:       binding(sessionID),
		State:         checkpoint.Value{Type: checkpoint.ValueType("function")},
	}
	payloadBytes, err := json.Marshal(payload)
	if err != nil {
		t.Fatal(err)
	}
	digest := sha256.Sum256(payloadBytes)
	encoded, err := json.Marshal(rawCheckpointDocument{Payload: payload, PayloadSHA256: hex.EncodeToString(digest[:])})
	if err != nil {
		t.Fatal(err)
	}
	return encoded
}

func TestReconstructionRejectsInvalidCheckpointValueAndAuthoritativeState(t *testing.T) {
	ctx := context.Background()
	input := reconstructionInput(map[string]any{"authoritative_marker": int64(8)})
	if replacement, err := Reconstruct(ctx, ProductionConfig("session-invalid-value"), input, invalidValueCheckpoint(t, "session-invalid-value"), binding("session-invalid-value")); replacement != nil || !errors.Is(err, checkpoint.ErrCorrupt) {
		t.Fatalf("invalid checkpoint value: replacement=%p error=%v", replacement, err)
	}
	encoded, err := checkpoint.Marshal(binding("session-invalid-authoritative"), map[string]any{"restored_marker": int64(42)})
	if err != nil {
		t.Fatal(err)
	}
	input = reconstructionInput(func() {})
	if replacement, err := Reconstruct(ctx, ProductionConfig("session-invalid-authoritative"), input, encoded, binding("session-invalid-authoritative")); replacement != nil || !errors.Is(err, checkpoint.ErrUnsupportedValue) {
		t.Fatalf("invalid authoritative state: replacement=%p error=%v", replacement, err)
	}
}

func TestReconstructionProgramFailuresFailClosedAndDiscardPartialVM(t *testing.T) {
	ctx := context.Background()
	encoded, err := checkpoint.Marshal(binding("session-program-failure"), map[string]any{"restored_marker": int64(42)})
	if err != nil {
		t.Fatal(err)
	}
	tests := []struct {
		name          string
		mutate        func(*ReconstructionInput)
		wantAllocated bool
	}{
		{name: "invalid runtime name", mutate: func(input *ReconstructionInput) { input.RuntimeProgram.Name = "" }},
		{name: "empty runtime source", mutate: func(input *ReconstructionInput) { input.RuntimeProgram.Source = "" }},
		{name: "runtime invalid UTF-8", mutate: func(input *ReconstructionInput) { input.RuntimeProgram.Source = string([]byte{0xff}) }},
		{name: "runtime bytecode", mutate: func(input *ReconstructionInput) { input.RuntimeProgram.Source = "\x1bLua\x54" }},
		{name: "runtime source oversized", mutate: func(input *ReconstructionInput) {
			input.RuntimeProgram.Source = strings.Repeat("x", MaxReconstructionSourceBytes+1)
		}},
		{name: "invalid restore name", mutate: func(input *ReconstructionInput) { input.RestoreProgram.Name = "bad\x00name" }},
		{name: "empty restore source", mutate: func(input *ReconstructionInput) { input.RestoreProgram.Source = "" }},
		{name: "runtime syntax error", wantAllocated: true, mutate: func(input *ReconstructionInput) { input.RuntimeProgram.Source = "local =" }},
		{name: "restore syntax error", wantAllocated: true, mutate: func(input *ReconstructionInput) { input.RestoreProgram.Source = "local =" }},
		{name: "runtime execution error", wantAllocated: true, mutate: func(input *ReconstructionInput) {
			input.RuntimeProgram.Source = `partial_coroutine = coroutine.create(function() coroutine.yield(1) end)
coroutine.resume(partial_coroutine)
error("runtime failure")`
		}},
		{name: "restore execution error", wantAllocated: true, mutate: func(input *ReconstructionInput) { input.RestoreProgram.Source = `error("restore failure")` }},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			input := reconstructionInput(map[string]any{"authoritative_marker": int64(8)})
			tt.mutate(&input)
			var allocated *VM
			factory := func(config Config) (*VM, error) {
				created, err := New(config)
				allocated = created
				return created, err
			}
			replacement, err := reconstruct(ctx, ProductionConfig("session-program-failure"), input, encoded, binding("session-program-failure"), factory)
			if replacement != nil || err == nil {
				if replacement != nil {
					_ = replacement.Destroy(ctx)
				}
				t.Fatalf("program failure returned replacement=%p error=%v", replacement, err)
			}
			if tt.wantAllocated {
				if allocated == nil || allocated.state != stateDestroyed || allocated.runtime != nil {
					t.Fatalf("partial VM was not discarded: %#v", allocated)
				}
				if _, evalErr := allocated.Eval(ctx, "discarded", []byte("return true")); !errors.Is(evalErr, ErrDestroyed) {
					t.Fatalf("discarded VM Eval error = %v", evalErr)
				}
			} else if allocated != nil {
				t.Fatal("invalid program allocated a VM")
			}
		})
	}
}

func TestCrossVMCheckpointContaminationIsRejected(t *testing.T) {
	ctx := context.Background()
	a := mustVM(t, "session-a")
	encoded, err := a.CreateCheckpoint(ctx, binding("session-a"), map[string]any{"restored_marker": int64(1)})
	if err != nil {
		t.Fatal(err)
	}
	input := reconstructionInput(map[string]any{"authoritative_marker": int64(1)})
	if _, err := Reconstruct(ctx, ProductionConfig("session-b"), input, encoded, binding("session-b")); !errors.Is(err, checkpoint.ErrIncompatibleBinding) {
		t.Fatalf("cross-VM reconstruction error = %v", err)
	}
	if _, err := a.CreateCheckpoint(ctx, binding("session-b"), map[string]any{"turn": 1}); !errors.Is(err, ErrBindingMismatch) {
		t.Fatalf("cross-VM creation error = %v", err)
	}
}
