// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package actor

import (
	"context"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/command"
	"sync/atomic"
	"testing"
	"time"
)

func TestRecoveryPointRechecksNativeManagementWhenQueued(t *testing.T) {
	rig := newRig(t, 2, time.Hour)
	var allowed atomic.Bool
	allowed.Store(true)
	basis := rig.identities["one-gm"]
	i, e := rig.authority.IssueNative(context.Background(), func(context.Context) (command.NativeSeat, error) {
		return command.NativeSeat{Binding: basis.Binding(), Principal: basis.Principal(), Seat: basis.Seat(), RecoveryPoint: allowed.Load(), Commands: map[string]func(checkpoint.Value) error{"block": func(checkpoint.Value) error { return nil }}}, nil
	})
	if e != nil {
		t.Fatal("issue native management identity")
	}
	done := make(chan error, 1)
	go func() {
		_, e := rig.registry.Submit(context.Background(), i, envelope("one", "block", "block", 1))
		done <- e
	}()
	<-rig.store.entered
	pointDone := make(chan error, 1)
	go func() { _, e := rig.registry.CreateRecoveryPoint(context.Background(), i); pointDone <- e }()
	waitQueue(t, rig.registry, i, 1)
	allowed.Store(false)
	close(rig.store.release)
	if e = <-done; e != nil {
		t.Fatal("bounded command failed")
	}
	if e = <-pointDone; e != command.ErrDenied {
		t.Fatal("queued removed management permission survived")
	}
	rig.store.mu.Lock()
	n := rig.store.session(basis.Binding()).checkpoints
	rig.store.mu.Unlock()
	if n != 0 {
		t.Fatal("unauthorized checkpoint written")
	}
	allowed.Store(true)
	point, e := rig.registry.CreateRecoveryPoint(context.Background(), i)
	if e != nil || point.Version != 2 || point.Cursor != 1 {
		t.Fatal("serialized recovery metadata mismatch")
	}
	rig.store.mu.Lock()
	n = rig.store.session(basis.Binding()).checkpoints
	active := rig.store.session(basis.Binding()).active
	rig.store.mu.Unlock()
	if n != 1 || active != 0 {
		t.Fatal("checkpoint did not close its runtime")
	}
	if _, e = rig.registry.CreateRecoveryPoint(context.Background(), basis); e != command.ErrDenied {
		t.Fatal("fixture identity silently acquired native management")
	}
}
