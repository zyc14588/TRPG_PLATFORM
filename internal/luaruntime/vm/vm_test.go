// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package vm

import (
	"context"
	"errors"
	"reflect"
	"sync"
	"testing"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
)

const packageHash = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"

func mustVM(t *testing.T, sessionID string) *VM {
	t.Helper()
	result, err := New(ProductionConfig(sessionID))
	if err != nil {
		t.Fatalf("New(%s): %v", sessionID, err)
	}
	t.Cleanup(func() {
		if err := result.Destroy(context.Background()); err != nil && !errors.Is(err, ErrDestroyed) {
			t.Errorf("Destroy(%s): %v", sessionID, err)
		}
	})
	return result
}

func valueBool(t *testing.T, value checkpoint.Value) bool {
	t.Helper()
	if value.Type != checkpoint.TypeBool {
		t.Fatalf("value type = %s, want boolean", value.Type)
	}
	return value.Boolean
}

func valueInt(t *testing.T, value checkpoint.Value) int64 {
	t.Helper()
	if value.Type != checkpoint.TypeInt {
		t.Fatalf("value type = %s, want integer", value.Type)
	}
	return value.Integer
}

func TestVMsDoNotShareGlobalsModulesCoroutinesRandomOrCapabilities(t *testing.T) {
	ctx := context.Background()
	a := mustVM(t, "session-a")
	b := mustVM(t, "session-b")

	if _, err := a.Eval(ctx, "set-global", []byte("session_marker = 91; return session_marker")); err != nil {
		t.Fatal(err)
	}
	missing, err := b.Eval(ctx, "get-global", []byte("return session_marker == nil"))
	if err != nil || !valueBool(t, missing) {
		t.Fatalf("B observed A global: value=%#v err=%v", missing, err)
	}

	if err := a.LoadModule(ctx, "counter", []byte("return { owner = 'a', count = 1 }")); err != nil {
		t.Fatal(err)
	}
	if loaded, err := b.HasModule(ctx, "counter"); err != nil || loaded {
		t.Fatalf("B observed A module: loaded=%v err=%v", loaded, err)
	}
	if err := b.LoadModule(ctx, "counter", []byte("return { owner = 'b', count = 1 }")); err != nil {
		t.Fatal(err)
	}
	aModule, err := a.ModuleValue(ctx, "counter")
	if err != nil {
		t.Fatal(err)
	}
	bModule, err := b.ModuleValue(ctx, "counter")
	if err != nil {
		t.Fatal(err)
	}
	if reflect.DeepEqual(aModule, bModule) {
		t.Fatalf("module states unexpectedly equal: %#v", aModule)
	}

	coType, err := a.Eval(ctx, "coroutine-a", []byte("co = coroutine.create(function() coroutine.yield(1) end); return type(co)"))
	if err != nil || coType.Type != checkpoint.TypeString || coType.String != "thread" {
		t.Fatalf("create A coroutine: value=%#v err=%v", coType, err)
	}
	missing, err = b.Eval(ctx, "coroutine-b", []byte("return co == nil"))
	if err != nil || !valueBool(t, missing) {
		t.Fatalf("B observed A coroutine: value=%#v err=%v", missing, err)
	}

	aFirst, err := a.Eval(ctx, "seed-a", []byte("math.randomseed(445566); return math.random(1, 1000000000)"))
	if err != nil {
		t.Fatal(err)
	}
	if _, err := a.Eval(ctx, "advance-a", []byte("return math.random(1, 1000000000)")); err != nil {
		t.Fatal(err)
	}
	bFirst, err := b.Eval(ctx, "seed-b", []byte("math.randomseed(445566); return math.random(1, 1000000000)"))
	if err != nil {
		t.Fatal(err)
	}
	if valueInt(t, aFirst) != valueInt(t, bFirst) {
		t.Fatalf("A progression changed B first draw: A=%#v B=%#v", aFirst, bFirst)
	}

	handle, err := a.IssueCapability(ctx, "state.read")
	if err != nil {
		t.Fatal(err)
	}
	if err := a.CheckCapability(ctx, handle, "state.read"); err != nil {
		t.Fatalf("A rejected its handle: %v", err)
	}
	if err := b.CheckCapability(ctx, handle, "state.read"); !errors.Is(err, ErrForeignCapability) {
		t.Fatalf("B capability error = %v", err)
	}
	if _, err := a.CreateCheckpoint(ctx, binding("session-a"), handle); !errors.Is(err, checkpoint.ErrUnsupportedValue) {
		t.Fatalf("capability checkpoint error = %v", err)
	}
}

func TestDestroyIsIsolatedAndRecreateIsClean(t *testing.T) {
	ctx := context.Background()
	a, err := New(ProductionConfig("session-a"))
	if err != nil {
		t.Fatal(err)
	}
	b := mustVM(t, "session-b")
	if _, err := a.Eval(ctx, "pollute", []byte("pollution = 123; return pollution")); err != nil {
		t.Fatal(err)
	}
	oldHandle, err := a.IssueCapability(ctx, "state.read")
	if err != nil {
		t.Fatal(err)
	}
	if err := a.Destroy(ctx); err != nil {
		t.Fatal(err)
	}
	if _, err := a.Eval(ctx, "after-destroy", []byte("return 1")); !errors.Is(err, ErrDestroyed) {
		t.Fatalf("destroyed Eval error = %v", err)
	}
	if _, err := b.Eval(ctx, "survivor", []byte("return 7")); err != nil {
		t.Fatalf("destroy A affected B: %v", err)
	}

	recreated := mustVM(t, "session-a")
	clean, err := recreated.Eval(ctx, "clean", []byte("return pollution == nil"))
	if err != nil || !valueBool(t, clean) {
		t.Fatalf("recreated A inherited global: value=%#v err=%v", clean, err)
	}
	if err := recreated.CheckCapability(ctx, oldHandle, "state.read"); !errors.Is(err, ErrForeignCapability) {
		t.Fatalf("recreated A accepted stale handle: %v", err)
	}
}

func TestDestroyDuringUseFailsBusy(t *testing.T) {
	v := mustVM(t, "session-busy")
	ctx := context.Background()
	entered := make(chan struct{})
	release := make(chan struct{})
	done := make(chan error, 1)
	go func() {
		if err := v.acquire(ctx); err != nil {
			done <- err
			return
		}
		close(entered)
		<-release
		v.release()
		done <- nil
	}()
	<-entered
	if err := v.Destroy(ctx); !errors.Is(err, ErrBusy) {
		t.Fatalf("Destroy during use error = %v", err)
	}
	close(release)
	if err := <-done; err != nil {
		t.Fatal(err)
	}
}

func TestRuntimeFailurePoisonsVMUntilReconstruction(t *testing.T) {
	ctx := context.Background()
	v := mustVM(t, "session-poison")
	if _, err := v.Eval(ctx, "fault", []byte("partial = 1; error('fault')")); err == nil {
		t.Fatal("faulting script succeeded")
	}
	if _, err := v.Eval(ctx, "must-reject", []byte("return partial")); !errors.Is(err, ErrPoisoned) {
		t.Fatalf("poisoned VM error = %v", err)
	}
	if err := v.Destroy(ctx); err != nil {
		t.Fatalf("destroy poisoned VM: %v", err)
	}
}

func TestConcurrentSessionIsolation(t *testing.T) {
	ctx := context.Background()
	const sessions = 12
	vms := make([]*VM, sessions)
	for i := range vms {
		vms[i] = mustVM(t, "race-session-"+string(rune('a'+i)))
	}
	var wg sync.WaitGroup
	errs := make(chan error, sessions)
	for i, runtime := range vms {
		i, runtime := i, runtime
		wg.Add(1)
		go func() {
			defer wg.Done()
			for n := 0; n < 50; n++ {
				value, err := runtime.Eval(ctx, "race", []byte("counter = (counter or 0) + 1; return counter"))
				if err != nil {
					errs <- err
					return
				}
				if value.Type != checkpoint.TypeInt || value.Integer != int64(n+1) {
					errs <- errors.New("cross-Session counter contamination")
					return
				}
			}
			_ = i
		}()
	}
	wg.Wait()
	close(errs)
	for err := range errs {
		t.Fatal(err)
	}
}
