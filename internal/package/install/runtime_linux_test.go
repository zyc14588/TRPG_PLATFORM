//go:build linux

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package install

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strings"
	"testing"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/vm"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/archive"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/testdata/install"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/object"
)

func logRuntimeEvidence(t *testing.T, value any) {
	t.Helper()
	raw, err := json.Marshal(value)
	if err != nil {
		t.Fatal(err)
	}
	t.Logf("I03_EVIDENCE %s", raw)
}

func logExecution(t *testing.T, c RuntimeConfig, e Execution) {
	t.Helper()
	if e.Profile != profile.ID || e.Runtime != profile.RuntimeVersion || e.RunnerHash != c.SHA256 || e.PID <= 0 {
		t.Fatal("unbound production execution", e)
	}
	logRuntimeEvidence(t, struct {
		Kind string `json:"kind"`
		Execution
	}{"production-execution", e})
}

func assertReaped(t *testing.T, pid int) {
	t.Helper()
	if pid <= 0 {
		t.Fatal("missing actual runner PID")
	}
	if _, err := os.Stat(fmt.Sprintf("/proc/%d", pid)); !os.IsNotExist(err) {
		t.Fatal("runner survived validation", pid, err)
	}
}

func buildRunner(t *testing.T) RuntimeConfig {
	t.Helper()
	path := filepath.Join(t.TempDir(), "lua-runner")
	cmd := exec.Command("go", "build", "-trimpath", "-o", path, "../../../cmd/lua-runner")
	if out, err := cmd.CombinedOutput(); err != nil {
		t.Fatalf("runner build: %v\n%s", err, out)
	}
	raw, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	c := RuntimeConfig{Runner: path, SHA256: object.Hash(raw), Limits: profile.DefaultLimits()}
	identity := func(arg string) string {
		out, err := exec.Command("git", "rev-parse", arg).Output()
		if err != nil {
			t.Fatal(err)
		}
		return strings.TrimSpace(string(out))
	}
	status, err := exec.Command("git", "status", "--porcelain").Output()
	if err != nil {
		t.Fatal(err)
	}
	cwd, err := os.Getwd()
	if err != nil {
		t.Fatal(err)
	}
	logRuntimeEvidence(t, map[string]any{
		"kind": "runner-build", "argv": cmd.Args, "cwd": cwd,
		"candidate_commit": identity("HEAD"), "candidate_tree": identity("HEAD^{tree}"),
		"tracked_clean": len(status) == 0, "runner_sha256": c.SHA256,
		"profile": profile.ID, "runtime": profile.RuntimeVersion,
		"goos": runtime.GOOS, "goarch": runtime.GOARCH, "go_version": runtime.Version(),
	})
	return c
}

func scriptPackage(t *testing.T, kind, source string) *archive.Package {
	t.Helper()
	p, err := fixtures.Build(fixtures.Files("test.publisher/fixture", kind, source))
	if err != nil {
		t.Fatal(err)
	}
	return p
}

func TestProductionValidationExecutesAndReapsIsolatedRunners(t *testing.T) {
	c := buildRunner(t)
	for _, kind := range []string{"game-system", "library"} {
		t.Run(kind, func(t *testing.T) {
			p := scriptPackage(t, kind, "return {count=7}")
			a := approval(t, p)
			a.Tests = []Test{{Name: "actual-value", Source: []byte("return require('lua.main').count == 7")}}
			var events []Execution
			digest, err := validateRuntime(context.Background(), c, []staged{{pkg: p, approval: a}}, func(e Execution) error { events = append(events, e); return nil })
			if err != nil {
				t.Fatal(err)
			}
			if len(digest) != 71 || len(events) < 3 {
				t.Fatal("no actual runtime evidence", events)
			}
			pids := map[int]bool{}
			for _, e := range events {
				logExecution(t, c, e)
				if e.PID > 0 {
					pids[e.PID] = pids[e.PID] || e.Reaped
				}
				if e.Outcome != "PASS" {
					t.Fatal(e)
				}
			}
			for pid, reaped := range pids {
				if !reaped {
					t.Fatal("no reap evidence", pid)
				}
				assertReaped(t, pid)
			}
		})
	}
	for name, source := range map[string]string{"missing-tests": "return true", "dangerous-library": "return os.getenv('HOME')", "loop": "while true do end", "false-result": "return false", "no-scripts-game": ""} {
		t.Run(name, func(t *testing.T) {
			kind := "library"
			if name == "no-scripts-game" {
				kind = "game-system"
				source = "return true"
			}
			p := scriptPackage(t, kind, "return true")
			a := approval(t, p)
			if name != "missing-tests" {
				a.Tests = []Test{{Name: name, Source: []byte(source)}}
			}
			if name == "no-scripts-game" {
				a.Tests = nil
			}
			var events []Execution
			_, err := validateRuntime(context.Background(), c, []staged{{pkg: p, approval: a}}, func(e Execution) error { events = append(events, e); return nil })
			if err == nil {
				t.Fatal("production failure accepted")
			}
			logRuntimeEvidence(t, map[string]any{"kind": "validation-denied", "case": name, "outcome": runtimeCode(err), "execution_events": len(events)})
			for _, e := range events {
				logExecution(t, c, e)
				if e.Reaped {
					assertReaped(t, e.PID)
				}
			}
		})
	}
	t.Run("runner-hash-binding", func(t *testing.T) {
		bad := c
		bad.SHA256 = "sha256:" + strings.Repeat("0", 64)
		p := scriptPackage(t, "library", "return true")
		a := approval(t, p)
		a.Tests = []Test{{Name: "pass", Source: []byte("return true")}}
		if _, err := validateRuntime(context.Background(), bad, []staged{{pkg: p, approval: a}}, func(Execution) error { return nil }); profile.Code(err) != profile.ErrConfiguration {
			t.Fatal("unbound runner accepted", err)
		}
		logRuntimeEvidence(t, map[string]any{"kind": "runner-hash-binding", "actual_sha256": c.SHA256, "rejected_sha256": bad.SHA256})
	})
}

func TestProductionRunnerCancellationAndDeadlineReap(t *testing.T) {
	c := buildRunner(t)
	for _, mode := range []string{"cancel", "deadline"} {
		t.Run(mode, func(t *testing.T) {
			var ctx context.Context
			var cancel context.CancelFunc
			want := error(context.Canceled)
			if mode == "deadline" {
				ctx, cancel = context.WithTimeout(context.Background(), 2*time.Second)
				want = context.DeadlineExceeded
			} else {
				ctx, cancel = context.WithCancel(context.Background())
			}
			defer cancel()
			p := scriptPackage(t, "library", "return true")
			a := approval(t, p)
			a.Tests = []Test{{Name: "after-cancellation", Source: []byte("return true")}}
			var events []Execution
			cancelledLiveRunner := false
			digest, err := validateRuntime(ctx, c, []staged{{pkg: p, approval: a}}, func(e Execution) error {
				events = append(events, e)
				if strings.HasPrefix(e.Case, "module:") && e.Outcome == "PASS" {
					if _, err := os.Stat(fmt.Sprintf("/proc/%d", e.PID)); err != nil {
						t.Fatal("cancellation did not target a live initialized runner", err)
					}
					cancelledLiveRunner = true
					if mode == "cancel" {
						cancel()
					} else {
						<-ctx.Done()
					}
				}
				return nil
			})
			if !cancelledLiveRunner || !errors.Is(err, want) || digest != "" {
				t.Fatal("cancelled validation was accepted or did not execute the live-runner boundary", cancelledLiveRunner, digest, err)
			}
			reaped := false
			for _, e := range events {
				logExecution(t, c, e)
				if e.Reaped {
					reaped = true
					assertReaped(t, e.PID)
				}
			}
			if !reaped {
				t.Fatal("cancelled runner has no terminal reap evidence")
			}
			logRuntimeEvidence(t, map[string]any{"kind": "cancelled-validation", "case": mode, "outcome": runtimeCode(err), "boundary": "live initialized runner after module validation, before trusted test", "digest_absent": digest == "", "reaped": reaped})
		})
	}
}

func TestProductionRunnerPoisonsAndReconstructsBeforeReuse(t *testing.T) {
	c := buildRunner(t)
	p := scriptPackage(t, "game-system", "cache=1;return true")
	state := vm.State{Value: checkpoint.Value{Kind: "nil"}}
	s, err := vm.New(context.Background(), vm.Options{SessionID: "install-poison-proof", Runner: c.Runner, Package: p, Limits: c.Limits, State: state, Audit: func(profile.Audit) error { return nil }})
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = s.Destroy() })
	oldPID, oldToken := s.PID(), s.Token()
	_, scriptErr := s.Execute(context.Background(), oldToken, []byte("cache=99;error('rejected-validation')"))
	_, poisonErr := s.Execute(context.Background(), oldToken, []byte("return cache"))
	if profile.Code(scriptErr) != profile.ErrScript || profile.Code(poisonErr) != profile.ErrPoisoned {
		t.Fatal("contaminated VM was reusable", scriptErr, poisonErr)
	}
	if err = s.Reconstruct(context.Background(), state, nil); err != nil {
		t.Fatal(err)
	}
	newPID := s.PID()
	if newPID == oldPID {
		t.Fatal("reconstruction reused contaminated runner")
	}
	assertReaped(t, oldPID)
	_, staleErr := s.Execute(context.Background(), oldToken, []byte("return true"))
	if profile.Code(staleErr) != profile.ErrCapability {
		t.Fatal("previous VM token survived reconstruction", staleErr)
	}
	result, err := s.Execute(context.Background(), s.Token(), []byte("return cache==1"))
	if err != nil || len(result.Values) != 1 || result.Values[0].Kind != "boolean" || !result.Values[0].Boolean {
		t.Fatal("replacement did not restore fresh authoritative lifecycle", result, err)
	}
	if err := s.Destroy(); err != nil {
		t.Fatal(err)
	}
	assertReaped(t, newPID)
	logRuntimeEvidence(t, map[string]any{"kind": "poisoned-vm-reconstruction", "runner_sha256": c.SHA256, "profile": profile.ID, "runtime": profile.RuntimeVersion, "old_pid": oldPID, "new_pid": newPID, "script_outcome": profile.Code(scriptErr), "followup_outcome": profile.Code(poisonErr), "old_token_outcome": profile.Code(staleErr), "replacement_outcome": "PASS", "old_and_new_reaped": true})
}

func TestRuntimeDoesNotGrantUndeclaredParentModulesToDependency(t *testing.T) {
	c := buildRunner(t)
	dep, err := fixtures.Build(fixtures.Files("test.publisher/library", "library", "return true"))
	if err != nil {
		t.Fatal(err)
	}
	files := fixtures.Files("test.publisher/root", "game-system", "return true")
	files[archive.ManifestPath] = append(files[archive.ManifestPath], []byte("\n[[dependencies]]\npackage_id = \"test.publisher/library\"\nversion = \"1.0.0\"\noptional = false\nfeatures = []\n")...)
	root, err := fixtures.Build(files, dep.ExactLock().Packages()...)
	if err != nil {
		t.Fatal(err)
	}
	ra := approval(t, root)
	ra.Tests = []Test{{Name: "root", Source: []byte("return true")}}
	da := approval(t, dep)
	da.Tests = []Test{{Name: "undeclared-parent", Source: []byte("local value = require('test.publisher/root:lua.main'); return true")}}
	if _, err = validateRuntime(context.Background(), c, []staged{{pkg: root, approval: ra}, {pkg: dep, approval: da}}, func(Execution) error { return nil }); err == nil {
		t.Fatal("dependency accessed an undeclared parent module")
	}
}

func TestOptionalCapabilityFallbackUsesActualPassedProductionExecution(t *testing.T) {
	c := buildRunner(t)
	files := fixtures.Files("test.publisher/fixture", "game-system", "return {mode='fallback'}")
	files[archive.ManifestPath] = append(files[archive.ManifestPath], []byte("\n[[capabilities.optional]]\nname = \"host.log\"\nfallback = \"continue without log\"\n")...)
	p, err := fixtures.Build(files)
	if err != nil {
		t.Fatal(err)
	}
	a := approval(t, p)
	a.Tests = []Test{{Name: "tested-log-fallback", Source: []byte("return require('lua.main').mode == 'fallback' and os == nil"), Capability: "host.log", Behavior: "continue without log"}}
	policy, err := NewPolicy(PolicyConfig{Context: "ci", HostMajor: 1, Artifacts: map[string]Approval{string(p.ArtifactIdentity().Digest()): a}})
	if err != nil {
		t.Fatal(err)
	}
	if _, _, err = policy.validate(p, Evidence{}); err != nil {
		t.Fatal(err)
	}
	if _, err = validateRuntime(context.Background(), c, []staged{{pkg: p, approval: a}}, func(Execution) error { return nil }); err != nil {
		t.Fatal("actual fallback proof rejected", err)
	}
	a.Tests[0].Source = []byte("return false")
	if _, err = validateRuntime(context.Background(), c, []staged{{pkg: p, approval: a}}, func(Execution) error { return nil }); err == nil {
		t.Fatal("failed fallback became proof")
	}
}
