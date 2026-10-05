//go:build linux

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package install

import (
	"context"
	"fmt"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/archive"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/testdata/install"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/object"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
)

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
	return RuntimeConfig{Runner: path, SHA256: object.Hash(raw), Limits: profile.DefaultLimits()}
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
				if _, err = os.Stat(fmt.Sprintf("/proc/%d", pid)); !os.IsNotExist(err) {
					t.Fatal("runner survived validation", pid, err)
				}
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
			for _, e := range events {
				if e.Reaped {
					if _, err = os.Stat(fmt.Sprintf("/proc/%d", e.PID)); !os.IsNotExist(err) {
						t.Fatal("failed runner leaked", err)
					}
				}
			}
		})
	}
	bad := c
	bad.SHA256 = "sha256:" + strings.Repeat("0", 64)
	p := scriptPackage(t, "library", "return true")
	a := approval(t, p)
	a.Tests = []Test{{Name: "pass", Source: []byte("return true")}}
	if _, err := validateRuntime(context.Background(), bad, []staged{{pkg: p, approval: a}}, func(Execution) error { return nil }); err == nil {
		t.Fatal("unbound runner accepted")
	}
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
