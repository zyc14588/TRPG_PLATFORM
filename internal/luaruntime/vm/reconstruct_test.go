// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package vm

import (
	"bytes"
	"context"
	"errors"
	"reflect"
	"testing"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
)

func binding(sessionID string) checkpoint.Binding {
	config := ProductionConfig(sessionID)
	return checkpoint.Binding{
		SessionID:      sessionID,
		StateVersion:   12,
		PackageHashes:  []checkpoint.PackageHash{{PackageID: "pkg.rules", SHA256: packageHash}},
		DependencyLock: `{"packages":[{"id":"pkg.rules","version":"1.2.3","sha256":"` + packageHash + `"}]}`,
		LuaProfile:     config.ProfileID,
		RuntimeVersion: config.RuntimeVersion,
	}
}

func TestCheckpointAndReconstructionAreExplicitAndDeterministic(t *testing.T) {
	ctx := context.Background()
	original := mustVM(t, "session-recovery")
	if _, err := original.Eval(ctx, "pollution", []byte("hidden_vm_only = 999; return hidden_vm_only")); err != nil {
		t.Fatal(err)
	}
	checkpointState := map[string]any{"cache": map[string]any{"round": int64(4)}, "enabled": true}
	encoded, err := original.CreateCheckpoint(ctx, binding("session-recovery"), checkpointState)
	if err != nil {
		t.Fatal(err)
	}
	authoritative := map[string]any{"turn": int64(22), "players": []any{"a", "b"}}
	first, err := Reconstruct(ctx, ProductionConfig("session-recovery"), authoritative, encoded, binding("session-recovery"))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = first.Destroy(ctx) })
	second, err := Reconstruct(ctx, ProductionConfig("session-recovery"), authoritative, encoded, binding("session-recovery"))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = second.Destroy(ctx) })
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
	clean, err := first.Eval(ctx, "no-hidden-memory", []byte("return hidden_vm_only == nil"))
	if err != nil || !valueBool(t, clean) {
		t.Fatalf("reconstruction used hidden VM state: value=%#v err=%v", clean, err)
	}

	reencoded, err := first.CreateCheckpoint(ctx, binding("session-recovery"), firstState.Checkpoint)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(encoded, reencoded) {
		t.Fatal("reconstructed checkpoint did not encode deterministically")
	}
}

func TestCrossVMCheckpointContaminationIsRejected(t *testing.T) {
	ctx := context.Background()
	a := mustVM(t, "session-a")
	encoded, err := a.CreateCheckpoint(ctx, binding("session-a"), map[string]any{"turn": 1})
	if err != nil {
		t.Fatal(err)
	}
	if _, err := Reconstruct(ctx, ProductionConfig("session-b"), map[string]any{"turn": 1}, encoded, binding("session-b")); !errors.Is(err, checkpoint.ErrIncompatibleBinding) {
		t.Fatalf("cross-VM reconstruction error = %v", err)
	}
	if _, err := a.CreateCheckpoint(ctx, binding("session-b"), map[string]any{"turn": 1}); !errors.Is(err, ErrBindingMismatch) {
		t.Fatalf("cross-VM creation error = %v", err)
	}
}

func TestReconstructionRejectsUnsupportedAuthoritativeState(t *testing.T) {
	ctx := context.Background()
	v := mustVM(t, "session-reject")
	encoded, err := v.CreateCheckpoint(ctx, binding("session-reject"), map[string]any{"turn": 1})
	if err != nil {
		t.Fatal(err)
	}
	if _, err := Reconstruct(ctx, ProductionConfig("session-reject"), func() {}, encoded, binding("session-reject")); !errors.Is(err, checkpoint.ErrUnsupportedValue) {
		t.Fatalf("unsupported authoritative state error = %v", err)
	}
}
