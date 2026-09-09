// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package vm

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"math"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"testing"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/ipc"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
)

func assertRestoreValue(t *testing.T, got, want checkpoint.Value) {
	t.Helper()
	g, err := json.Marshal(got)
	if err != nil {
		t.Fatal(err)
	}
	w, err := json.Marshal(want)
	if err != nil {
		t.Fatal(err)
	}
	if string(g) != string(w) {
		t.Errorf("checkpoint value/type/shape changed: got %s; want %s", g, w)
	}
}

func readRestoreValue(t *testing.T, s *Session, want checkpoint.Value) {
	t.Helper()
	r, err := s.Execute(context.Background(), s.Token(), []byte(`return cache`))
	if err != nil {
		t.Errorf("restored read failed: %s", profile.Code(err))
	} else if len(r.Values) != 1 {
		t.Errorf("restored result count = %d", len(r.Values))
	} else {
		assertRestoreValue(t, r.Values[0], want)
	}
	// Observe the next operation before any cleanup or recovery can mask poison.
	next, nextErr := s.Execute(context.Background(), s.Token(), []byte(`return 6*7`))
	if nextErr != nil || len(next.Values) != 1 || next.Values[0].Kind != "integer" || next.Values[0].Number != "42" {
		t.Errorf("post-restore independent operation: code=%s values=%+v", profile.Code(nextErr), next.Values)
	}
	t.Logf("READ=%s FOLLOWUP=%s VM_POISONED=%v", profile.Code(err), profile.Code(nextErr), s.poisoned)
}

// ACC-M1-B002-010: retain all three independent acceptance inputs. The first
// follows the original public Session probe, including its package entrypoint.
func TestACC010OriginalReconstructionExamples(t *testing.T) {
	cases := []struct {
		name  string
		value checkpoint.Value
	}{
		{"nil_array_element", checkpoint.Array(checkpoint.Value{Kind: "nil"}, checkpoint.Int(7))},
		{"empty_array", checkpoint.Array()},
		{"nil_table_entry", checkpoint.Object(map[string]checkpoint.Value{"value": {Kind: "nil"}, "kept": checkpoint.Int(7)})},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			opts := options(t, "independent-public-reconstruct", fixture(t, 1, "", `cache = checkpoint or state`))
			s, err := New(context.Background(), opts)
			if err != nil {
				t.Fatal(err)
			}
			defer s.Destroy()
			before, err := s.Capture(context.Background(), s.Token(), []byte(`return cache`))
			if err != nil {
				t.Fatal(err)
			}
			saved, err := checkpoint.Seal(before.Binding, tc.value)
			if err != nil {
				t.Fatal("original accepted input rejected", err)
			}
			old, token := s.client, s.Token()
			if err := s.Reconstruct(context.Background(), opts.State, &saved); err != nil {
				t.Fatal("compatible reconstruction rejected", err)
			}
			if old.PID() == s.PID() || token == s.Token() {
				t.Fatal("worker/generation not replaced")
			}
			if _, err := old.Call(context.Background(), ipc.Request{Operation: "execute", Source: []byte(`return 7`)}); !errors.Is(err, ipc.ErrRunner) {
				t.Fatal("old worker still usable", err)
			}
			t.Logf("RECONSTRUCT=PASS OLD_PID=%d REPLACEMENT_PID=%d", old.PID(), s.PID())
			readRestoreValue(t, s, saved.State)
		})
	}
}

func TestACC010AcceptedValuesSurviveWorkerLoss(t *testing.T) {
	nilValue := checkpoint.Value{Kind: "nil"}
	values := []checkpoint.Value{
		nilValue, checkpoint.Bool(false), checkpoint.Bool(true), checkpoint.Int(math.MinInt64), checkpoint.Int(math.MaxInt64),
		checkpoint.Int(1<<53 + 1), checkpoint.Int(-1<<53 - 1), checkpoint.Int(0),
		checkpoint.Text(""), checkpoint.Text("检查点\x00🌏"), checkpoint.Array(), checkpoint.Object(nil),
		checkpoint.Array(nilValue), checkpoint.Array(nilValue, nilValue), checkpoint.Array(checkpoint.Int(7), nilValue),
		checkpoint.Object(map[string]checkpoint.Value{"": nilValue, "kept": checkpoint.Int(7)}),
		checkpoint.Object(map[string]checkpoint.Value{
			"nested": checkpoint.Array(checkpoint.Array(), checkpoint.Object(map[string]checkpoint.Value{"nil": nilValue}), nilValue, checkpoint.Array(nilValue, checkpoint.Int(7), nilValue)),
		}),
	}
	for _, f := range []float64{0, math.Copysign(0, -1), math.SmallestNonzeroFloat64, -0.5, 1 << 53, -1 << 53} {
		values = append(values, checkpoint.Value{Kind: "float", Number: strconv.FormatFloat(f, 'g', -1, 64)})
	}
	deep := nilValue
	for range checkpoint.MaxDepth {
		deep = checkpoint.Array(deep)
	}
	values = append(values, deep)
	for i, value := range values {
		t.Run(fmt.Sprintf("%02d_%s", i, value.Kind), func(t *testing.T) {
			opts := options(t, "accepted-domain", fixture(t, 1, "", `cache=state; if checkpoint~=nil then cache=checkpoint end`))
			opts.State.Value = value
			s, err := New(context.Background(), opts)
			if err != nil {
				t.Fatal(err)
			}
			defer s.Destroy()
			captured, err := s.Capture(context.Background(), s.Token(), []byte(`return cache`))
			if err != nil {
				t.Fatal("capture of supported state failed", err)
			}
			assertRestoreValue(t, captured.State, value)
			raw, err := checkpoint.Encode(captured)
			if err != nil {
				t.Fatal(err)
			}
			saved, err := checkpoint.Decode(raw, captured.Binding)
			if err != nil {
				t.Fatal(err)
			}
			execute(t, s, `unpersisted=99`)
			old := s.client
			old.Kill() // Destroy and reap the original process before replacement.
			if _, err := s.Execute(context.Background(), s.Token(), []byte(`return 7`)); !IsBoundaryFailure(err) || !s.poisoned {
				t.Fatal("worker loss was not isolated", err)
			}
			if err := s.Reconstruct(context.Background(), opts.State, &saved); err != nil {
				t.Fatal(err)
			}
			if old.PID() == s.PID() {
				t.Fatal("original process reused")
			}
			readRestoreValue(t, s, value)
			execute(t, s, `assert(unpersisted==nil)`)
			for range 2 {
				before := s.PID()
				again, err := s.RebuildForMemoryPressure(context.Background(), s.Token(), []byte(`return cache`))
				if err != nil || s.PID() == before || again.Digest != saved.Digest {
					t.Fatalf("successive memory-pressure recovery changed state: %v digest=%s want=%s", err, again.Digest, saved.Digest)
				}
				readRestoreValue(t, s, value)
			}
		})
	}
}

func TestACC010InvalidCheckpointPreservesLifecycle(t *testing.T) {
	mutations := map[string]func(*checkpoint.Checkpoint){
		"digest":  func(c *checkpoint.Checkpoint) { c.Digest = checkpoint.Hash([]byte("wrong")) },
		"session": func(c *checkpoint.Checkpoint) { c.Binding.SessionID = "another-session" },
		"package": func(c *checkpoint.Checkpoint) {
			c.Binding.PackageHashes["example.test/runtime"] = checkpoint.Hash([]byte("another-package"))
		},
		"profile":     func(c *checkpoint.Checkpoint) { c.Binding.LuaProfile = "other" },
		"runtime":     func(c *checkpoint.Checkpoint) { c.Binding.RuntimeVersion = "other" },
		"unsupported": func(c *checkpoint.Checkpoint) { c.State = checkpoint.Value{Kind: "function"} },
		"nonfinite":   func(c *checkpoint.Checkpoint) { c.State = checkpoint.Value{Kind: "float", Number: "NaN"} },
		"capability":  func(c *checkpoint.Checkpoint) { c.State = checkpoint.Text("cap:forbidden") },
	}
	for name, mutate := range mutations {
		for _, poisoned := range []bool{false, true} {
			t.Run(fmt.Sprintf("%s/poisoned=%t", name, poisoned), func(t *testing.T) {
				s := start(t, "invalid-checkpoint")
				good, err := s.Capture(context.Background(), s.Token(), []byte(`return {cache=cache}`))
				if err != nil {
					t.Fatal(err)
				}
				raw, _ := checkpoint.Encode(good)
				bad, err := checkpoint.Decode(raw, good.Binding)
				if err != nil {
					t.Fatal(err)
				}
				mutate(&bad)
				if poisoned {
					if _, err := s.Execute(context.Background(), s.Token(), []byte(`partial=41;return function() end`)); profile.Code(err) != profile.ErrValue {
						t.Fatal(err)
					}
				}
				old, token, state := s.client, s.Token(), s.state
				if err := s.Reconstruct(context.Background(), state, &bad); !errors.Is(err, checkpoint.ErrRejected) {
					t.Fatal("invalid checkpoint not explicitly rejected", err)
				}
				if s.client != old || token != s.Token() || s.poisoned != poisoned || s.state.Version != state.Version {
					t.Fatal("rejected restore published or changed lifecycle")
				}
				if poisoned {
					if _, err := s.Execute(context.Background(), token, []byte(`return 7`)); profile.Code(err) != profile.ErrPoisoned {
						t.Fatal("rejection cleared existing poison", err)
					}
				} else {
					execute(t, s, `assert(partial==nil);return cache`)
				}
				if err := s.Reconstruct(context.Background(), state, &good); err != nil {
					t.Fatal(err)
				}
				execute(t, s, `assert(partial==nil and cache==7)`)
			})
		}
	}
}

// Read real child PIDs for the Linux Core runner; no substitute runner is used.
func restoreChildPIDs(t *testing.T) map[string]bool {
	t.Helper()
	paths, err := filepath.Glob("/proc/self/task/*/children")
	if err != nil || len(paths) == 0 {
		t.Fatal("Linux child-process evidence unavailable", err)
	}
	children := map[string]bool{}
	for _, path := range paths {
		raw, err := os.ReadFile(path)
		if os.IsNotExist(err) { // A Go runtime thread may have just exited.
			continue
		}
		if err != nil {
			t.Fatal(err)
		}
		for _, pid := range strings.Fields(string(raw)) {
			children[pid] = true
		}
	}
	return children
}

func TestACC010FailedReplacementIsReaped(t *testing.T) {
	opts := options(t, "replacement-rejection", fixture(t, 1, "", `if checkpoint and checkpoint.reject then partial=99; error("reject") end; cache=state.counter`))
	var before map[string]bool
	var failedPIDs []string
	opts.Audit = func(a profile.Audit) error {
		if before != nil && a.Kind == "runner-start" && a.Outcome != "PASS" {
			for pid := range restoreChildPIDs(t) {
				if !before[pid] {
					failedPIDs = append(failedPIDs, pid)
				}
			}
		}
		return nil
	}
	s, err := New(context.Background(), opts)
	if err != nil {
		t.Fatal(err)
	}
	defer s.Destroy()
	c, err := s.Capture(context.Background(), s.Token(), []byte(`return {reject=true}`))
	if err != nil {
		t.Fatal(err)
	}
	old, token := s.client, s.Token()
	before = restoreChildPIDs(t)
	if err := s.Reconstruct(context.Background(), opts.State, &c); profile.Code(err) != profile.ErrScript {
		t.Fatal("replacement entrypoint failure not returned", err)
	}
	if s.client != old || token != s.Token() || s.poisoned {
		t.Fatal("partially initialized replacement was published")
	}
	if len(failedPIDs) != 1 {
		t.Fatalf("expected one real failed replacement, observed %v", failedPIDs)
	}
	if _, err := os.Stat("/proc/" + failedPIDs[0]); !os.IsNotExist(err) {
		t.Fatalf("failed replacement not reaped: PID=%s err=%v", failedPIDs[0], err)
	}
	execute(t, s, `assert(partial==nil and cache==7);return 42`)
	t.Logf("FAILED_REPLACEMENT_PID=%s REAPED=true ORIGINAL_PID=%d USABLE=true", failedPIDs[0], old.PID())
}

func TestACC010RestoreIsolationAndMutation(t *testing.T) {
	pkg := fixture(t, 2, "", `cache=checkpoint or state`)
	opts := options(t, "shape-a", pkg)
	opts.State.Value = checkpoint.Object(map[string]checkpoint.Value{
		"nil": {Kind: "nil"}, "empty": checkpoint.Array(), "nested": checkpoint.Object(map[string]checkpoint.Value{"nil": {Kind: "nil"}}),
	})
	a, err := New(context.Background(), opts)
	if err != nil {
		t.Fatal(err)
	}
	defer a.Destroy()
	saved, err := a.Capture(context.Background(), a.Token(), []byte(`return cache`))
	if err != nil {
		t.Fatal(err)
	}
	assertRestoreValue(t, saved.State, opts.State.Value)
	for _, other := range []Options{
		options(t, "shape-b", pkg),
		options(t, "shape-a", fixture(t, 2, "", `cache=state;different_package=true`)),
	} {
		b, err := New(context.Background(), other)
		if err != nil {
			t.Fatal(err)
		}
		defer b.Destroy()
		old, token := b.client, b.Token()
		if err := b.Reconstruct(context.Background(), other.State, &saved); !errors.Is(err, checkpoint.ErrRejected) || b.client != old || token != b.Token() {
			t.Fatal("cross-session/package checkpoint was published", err)
		}
		readRestoreValue(t, b, other.State.Value)
	}
	if err := a.Reconstruct(context.Background(), opts.State, &saved); err != nil {
		t.Fatal(err)
	}
	execute(t, a, `assert(type(cache)=="table" and getmetatable(cache)==nil and cache["nil"]==nil);cache["nil"]=9;cache.empty[1]=7;cache.nested={}`)
	want := checkpoint.Object(map[string]checkpoint.Value{"nil": checkpoint.Int(9), "empty": checkpoint.Array(checkpoint.Int(7)), "nested": checkpoint.Object(nil)})
	readRestoreValue(t, a, want)
	execute(t, a, `rawset(cache,"nil",nil);table.remove(cache.empty,1)`)
	want = checkpoint.Object(map[string]checkpoint.Value{"empty": checkpoint.Array(), "nested": checkpoint.Object(nil)})
	readRestoreValue(t, a, want)
	for range 2 {
		if _, err := a.RebuildForMemoryPressure(context.Background(), a.Token(), []byte(`return cache`)); err != nil {
			t.Fatal(err)
		}
		readRestoreValue(t, a, want)
	}
}
