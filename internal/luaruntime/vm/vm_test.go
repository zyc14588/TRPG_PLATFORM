// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package vm

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/archive"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/dependency"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/extension"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/manifest"
)

var runnerPath string

func TestMain(m *testing.M) {
	dir, err := os.MkdirTemp("", "b002-vm-runner-")
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
	runnerPath = filepath.Join(dir, "lua-runner")
	cmd := exec.Command("go", "build", "-trimpath", "-o", runnerPath, "./cmd/lua-runner")
	cmd.Dir = "../../.."
	if output, err := cmd.CombinedOutput(); err != nil {
		fmt.Fprintln(os.Stderr, string(output), err)
		os.RemoveAll(dir)
		os.Exit(1)
	}
	code := m.Run()
	os.RemoveAll(dir)
	os.Exit(code)
}

const fixtureManifest = `schema_version = 1
artifact_type = "package"
package_id = "example.test/runtime"
package_kind = "game-system"
version = "1.0.0"
display_name = "Runtime conformance fixture"
entrypoint = "lua/main.lua"
lua_profile = "platform-lua-5.5-p1"
[host_api]
major = 1
min_minor = 0
max_minor = 0
[build]
source = "https://example.invalid/runtime"
revision = "0123456789abcdef"
builder = "runtime-test/1"
[rights]
authors = ["Runtime conformance"]
source = "original"
license_expression = "PolyForm-Noncommercial-1.0.0"
[capabilities]
required = []
`

func fixture(t *testing.T, version int, manifestText, source string) *archive.Package {
	t.Helper()
	if manifestText == "" {
		manifestText = fixtureManifest
	}
	files := map[string][]byte{archive.ManifestPath: []byte(manifestText), "lua/main.lua": []byte(source), "helper.lua": []byte(`return {count=0}`)}
	if version == 2 {
		schema := []byte(`{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","properties":{"entrypoint":{"type":"string"}},"additionalProperties":false}`)
		files[archive.ManifestPath] = []byte(strings.Replace(manifestText, "schema_version = 1", "schema_version = 2", 1) + fmt.Sprintf(`
[[extensions]]
namespace = "example.third.runtime"
required = false
contract_version = 1
schema_path = "extensions/example.third.runtime/value.schema.json"
schema_sha256 = "%s"
payload_path = "extensions/example.third.runtime/value.json"
host_api_major = 1
host_api_min_minor = 0
host_api_max_minor = 0
`, checkpoint.Hash(schema)))
		files["extensions/example.third.runtime/value.schema.json"] = schema
		files["extensions/example.third.runtime/value.json"] = []byte(`{"entrypoint":"evil.lua"}`)
	}
	document, err := manifest.Parse([]byte(manifestText))
	if err != nil {
		t.Fatal(err)
	}
	id := string(document.Package.PackageID)
	node, err := dependency.NewLockedPackage(id, "1.0.0", "sha256:"+strings.Repeat("0", 64), nil, nil)
	if err != nil {
		t.Fatal(err)
	}
	lock, err := dependency.BuildExactLock(id, []dependency.LockedPackage{node})
	if err != nil {
		t.Fatal(err)
	}
	pkg, err := archive.FromFiles(files, lock, extension.DefaultSupport)
	if err != nil {
		t.Fatal(err)
	}
	return pkg
}
func options(t *testing.T, id string, pkg *archive.Package) Options {
	t.Helper()
	return Options{SessionID: id, Runner: runnerPath, Package: pkg, State: State{Version: 1, Value: checkpoint.Object(map[string]checkpoint.Value{"counter": checkpoint.Int(7)})}, Limits: profile.DefaultLimits(), Audit: func(a profile.Audit) error {
		if a.Level != "AUDIT-0" || a.Sequence == 0 {
			return fmt.Errorf("invalid audit")
		}
		return nil
	}}
}
func start(t *testing.T, id string) *Session {
	t.Helper()
	s, err := New(context.Background(), options(t, id, fixture(t, 1, "", `cache=checkpoint and checkpoint.cache or state.counter`)))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { s.Destroy() })
	return s
}
func execute(t *testing.T, s *Session, source string) profile.Result {
	t.Helper()
	r, err := s.Execute(context.Background(), s.Token(), []byte(source))
	if err != nil {
		t.Fatal(err)
	}
	return r
}

func TestSessionIsolationLifecycleAndHandles(t *testing.T) {
	a := start(t, "a")
	b := start(t, "b")
	if a.PID() == b.PID() || a.PID() == os.Getpid() {
		t.Fatal("not isolated processes")
	}
	execute(t, a, `global_marker=42; m=require("helper");m.count=11; co=coroutine.create(function() coroutine.yield(7);return 9 end);assert(select(2,coroutine.resume(co))==7)`)
	execute(t, b, `assert(global_marker==nil and co==nil);assert(require("helper").count==0);assert(math.random==nil and math.randomseed==nil)`)
	execute(t, a, `assert(require("helper").count==11);assert(select(2,coroutine.resume(co))==9)`)
	if _, err := b.Execute(context.Background(), a.Token(), []byte(`return 42`)); profile.Code(err) != profile.ErrCapability {
		t.Fatal("cross-VM handle accepted", err)
	}
	if _, err := a.Execute(context.Background(), Token{}, []byte(`return 42`)); profile.Code(err) != profile.ErrCapability {
		t.Fatal("forged handle accepted", err)
	}
	if _, err := json.Marshal(a.Token()); err == nil {
		t.Fatal("token serialized")
	}
	old := a.Token()
	if err := a.Reconstruct(context.Background(), a.state, nil); err != nil {
		t.Fatal(err)
	}
	if _, err := a.Execute(context.Background(), old, []byte(`return 42`)); profile.Code(err) != profile.ErrCapability {
		t.Fatal("stale handle accepted", err)
	}
	if err := a.Destroy(); err != nil {
		t.Fatal(err)
	}
	if _, err := a.Execute(context.Background(), a.Token(), []byte(`return 42`)); profile.Code(err) != profile.ErrDestroyed {
		t.Fatal("destroyed VM executed", err)
	}
	execute(t, b, `assert(require("helper").count==0);return 7`)
}

func TestACC008IPCContaminationAndReconstruction(t *testing.T) {
	s := start(t, "acc008")
	before := s.PID()
	if _, err := s.Execute(context.Background(), s.Token(), []byte(`partial=41;return function() end`)); profile.Code(err) != profile.ErrValue {
		t.Fatal(err)
	}
	if _, err := s.Execute(context.Background(), s.Token(), []byte(`return partial+1`)); profile.Code(err) != profile.ErrPoisoned {
		t.Fatal("008 reproduced", err)
	}
	if err := s.Reconstruct(context.Background(), s.state, nil); err != nil {
		t.Fatal(err)
	}
	if before == s.PID() {
		t.Fatal("VM was not reconstructed")
	}
	execute(t, s, `assert(partial==nil);return cache`)
}

func TestCheckpointReconstructionAndMemoryPressure(t *testing.T) {
	s := start(t, "rebuild")
	execute(t, s, `cache=cache+1`)
	c, err := s.Capture(context.Background(), s.Token(), []byte(`return {cache=cache}`))
	if err != nil {
		t.Fatal(err)
	}
	bad := c
	bad.Binding.StateVersion++
	before := s.PID()
	if err := s.Reconstruct(context.Background(), s.state, &bad); err == nil || s.PID() != before {
		t.Fatal("incompatible checkpoint replaced VM")
	}
	if err := s.Reconstruct(context.Background(), s.state, &c); err != nil {
		t.Fatal(err)
	}
	r := execute(t, s, `return cache`)
	if r.Values[0].Number != "8" {
		t.Fatal(r)
	}
	c2, err := s.RebuildForMemoryPressure(context.Background(), s.Token(), []byte(`return {cache=cache}`))
	if err != nil || c.Digest != c2.Digest {
		t.Fatal("nondeterministic reconstruction", err)
	}
	before = s.PID()
	if _, err := s.RebuildForMemoryPressure(context.Background(), s.Token(), []byte(`cache=99;return function() end`)); err == nil || s.PID() != before {
		t.Fatal("memory pressure reconstructed without checkpoint")
	}
	if _, err := s.Execute(context.Background(), s.Token(), []byte(`return cache`)); profile.Code(err) != profile.ErrPoisoned {
		t.Fatal(err)
	}
}

func TestRunnerFaultMemoryAndCancellationIsolation(t *testing.T) {
	healthy := start(t, "healthy")
	for name, source := range map[string]string{"memory": `return string.rep("x",192*1024*1024,"")`, "wall": `return string.rep("",1000000000,"")`} {
		t.Run(name, func(t *testing.T) {
			s := start(t, name)
			begin := time.Now()
			if _, err := s.Execute(context.Background(), s.Token(), []byte(source)); !IsBoundaryFailure(err) {
				t.Fatalf("expected isolated runner fault: %v", err)
			}
			if time.Since(begin) > 3*time.Second {
				t.Fatal("hard wall bound failed")
			}
			if err := s.Reconstruct(context.Background(), s.state, nil); err != nil {
				t.Fatal(err)
			}
			execute(t, s, `return state.counter`)
			execute(t, healthy, `return state.counter`)
		})
	}
	s := start(t, "cancel")
	ctx, cancel := context.WithCancel(context.Background())
	cancel()
	if _, err := s.Execute(ctx, s.Token(), []byte(`while true do end`)); err == nil {
		t.Fatal("cancel ignored")
	}
	if err := s.Reconstruct(context.Background(), s.state, nil); err != nil {
		t.Fatal(err)
	}
}

func TestB011PackageInputsAndRejections(t *testing.T) {
	for _, version := range []int{1, 2} {
		pkg := fixture(t, version, "", `return state.counter`)
		before, err := pkg.Export()
		if err != nil {
			t.Fatal(err)
		}
		s, err := New(context.Background(), options(t, fmt.Sprintf("v%d", version), pkg))
		if err != nil {
			t.Fatal(err)
		}
		execute(t, s, `return state.counter`)
		s.Destroy()
		after, err := pkg.Export()
		if err != nil || before.Hash() != after.Hash() {
			t.Fatal("B011 archive changed")
		}
	}
	for _, text := range []string{strings.Replace(fixtureManifest, "platform-lua-5.5-p1", "platform-lua-5.4-p1", 1), strings.Replace(fixtureManifest, "lua/main.lua", "lua/missing.lua", 1), strings.Replace(fixtureManifest, "required = []", "required = [\"host.state\"]", 1)} {
		pkg := fixture(t, 1, text, `return 1`)
		if s, err := New(context.Background(), options(t, "invalid", pkg)); err == nil {
			s.Destroy()
			t.Fatal("invalid runtime package accepted")
		}
	}
	if _, err := New(context.Background(), options(t, "nil", nil)); err == nil {
		t.Fatal("nil package accepted")
	}
}

func TestMultiSessionRaceIsolation(t *testing.T) {
	const count = 4
	var group sync.WaitGroup
	for i := range count {
		group.Add(1)
		go func(i int) {
			defer group.Done()
			s := start(t, fmt.Sprintf("race-%d", i))
			for j := range 12 {
				r, err := s.Execute(context.Background(), s.Token(), []byte(`counter=(counter or 0)+1;return counter`))
				if err != nil || r.Values[0].Number != fmt.Sprint(j+1) {
					t.Errorf("cross-session state: %v %#v", err, r)
					return
				}
			}
		}(i)
	}
	group.Wait()
}

func TestMandatoryAuditAndFailureRecovery(t *testing.T) {
	pkg := fixture(t, 1, "", `cache=state.counter`)
	opts := options(t, "audit", pkg)
	opts.Audit = nil
	if s, err := New(context.Background(), opts); err == nil {
		s.Destroy()
		t.Fatal("disabled audit accepted")
	}
	var events []profile.Audit
	fail := false
	opts.Audit = func(a profile.Audit) error {
		events = append(events, a)
		if fail {
			return fmt.Errorf("audit storage unavailable")
		}
		return nil
	}
	s, err := New(context.Background(), opts)
	if err != nil {
		t.Fatal(err)
	}
	defer s.Destroy()
	fail = true
	if _, err := s.Execute(context.Background(), s.Token(), []byte(`cache=99`)); err == nil {
		t.Fatal("audit failure permitted execution result")
	}
	fail = false
	if _, err := s.Execute(context.Background(), s.Token(), []byte(`return cache`)); profile.Code(err) != profile.ErrPoisoned {
		t.Fatal("audit failure did not poison", err)
	}
	if err := s.Reconstruct(context.Background(), opts.State, nil); err != nil {
		t.Fatal(err)
	}
	if r := execute(t, s, `return cache`); r.Values[0].Number != "7" {
		t.Fatal("authoritative recovery lost", r)
	}
	before := len(events)
	bad := opts
	bad.Package = nil
	if _, err := New(context.Background(), bad); err == nil || len(events) != before+1 || events[len(events)-1].Outcome == "PASS" {
		t.Fatal("initialization failure not audited", err)
	}
	for _, a := range events {
		if a.Level != "AUDIT-0" || a.Sequence == 0 {
			t.Fatal("invalid minimum audit", a)
		}
	}
}

func TestCapabilitiesRequireTestedFallbackOnExactPackage(t *testing.T) {
	source := `assert(host==nil);fallback_counter=state.counter+1`
	text := fixtureManifest + "\n[[capabilities.optional]]\nname = \"host.log\"\nfallback = \"continue without package log records\"\n"
	pkg := fixture(t, 1, text, source)
	opts := options(t, "fallback", pkg)
	if s, err := New(context.Background(), opts); profile.Code(err) != profile.ErrCapability {
		if s != nil {
			s.Destroy()
		}
		t.Fatal("untested fallback admitted", err)
	}
	// A trusted validation caller actually exercises the missing-capability path
	// against this immutable package before supplying its evidence digest.
	e, err := profile.New(profile.Config{Limits: profile.DefaultLimits()})
	if err != nil {
		t.Fatal(err)
	}
	defer e.Close()
	if err := e.SetState(opts.State.Value, checkpoint.Value{Kind: "nil"}); err != nil {
		t.Fatal(err)
	}
	r, err := e.Execute(context.Background(), []byte(source+`;return fallback_counter`))
	if err != nil || r.Values[0].Number != "8" {
		t.Fatal("fallback behavior failed", err)
	}
	raw, err := json.Marshal(r)
	if err != nil {
		t.Fatal(err)
	}
	proof := FallbackProof{PackageHash: string(pkg.ContentHash()), Behavior: "continue without package log records", EvidenceDigest: checkpoint.Hash(raw)}
	opts.Fallbacks = map[string]FallbackProof{"example.test/runtime:host.log": proof}
	s, err := New(context.Background(), opts)
	if err != nil {
		t.Fatal(err)
	}
	if r := execute(t, s, `assert(host==nil);return fallback_counter`); r.Values[0].Number != "8" {
		t.Fatal(r)
	}
	s.Destroy()
	for _, bad := range []FallbackProof{{PackageHash: "sha256:" + strings.Repeat("f", 64), Behavior: proof.Behavior, EvidenceDigest: proof.EvidenceDigest}, {PackageHash: proof.PackageHash, Behavior: "different behavior", EvidenceDigest: proof.EvidenceDigest}, {PackageHash: proof.PackageHash, Behavior: proof.Behavior}} {
		opts.Fallbacks["example.test/runtime:host.log"] = bad
		if s, err := New(context.Background(), opts); profile.Code(err) != profile.ErrCapability {
			if s != nil {
				s.Destroy()
			}
			t.Fatal("unbound fallback evidence accepted", err)
		}
	}
}

func TestExactDependencyGraphAndModuleIsolation(t *testing.T) {
	depText := strings.Replace(fixtureManifest, "example.test/runtime", "example.test/library", 1)
	dep := fixture(t, 1, depText, `return 1`)
	rootText := fixtureManifest + "\n[[dependencies]]\npackage_id = \"example.test/library\"\nversion = \"1.0.0\"\noptional = false\n"
	rootNode, err := dependency.NewLockedPackage("example.test/runtime", "1.0.0", "sha256:"+strings.Repeat("0", 64), nil, []string{"example.test/library"})
	if err != nil {
		t.Fatal(err)
	}
	depNode, err := dependency.NewLockedPackage("example.test/library", "1.0.0", string(dep.ContentHash()), nil, nil)
	if err != nil {
		t.Fatal(err)
	}
	lock, err := dependency.BuildExactLock("example.test/runtime", []dependency.LockedPackage{rootNode, depNode})
	if err != nil {
		t.Fatal(err)
	}
	pkg, err := archive.FromFiles(map[string][]byte{archive.ManifestPath: []byte(rootText), "lua/main.lua": []byte(`m=require("example.test/library:helper");m.count=m.count+1`)}, lock, extension.DefaultSupport)
	if err != nil {
		t.Fatal(err)
	}
	opts := options(t, "deps", pkg)
	if s, err := New(context.Background(), opts); err == nil {
		s.Destroy()
		t.Fatal("missing exact dependency admitted")
	}
	opts.Dependencies = []*archive.Package{dep}
	a, err := New(context.Background(), opts)
	if err != nil {
		t.Fatal(err)
	}
	defer a.Destroy()
	opts.SessionID = "deps-b"
	b, err := New(context.Background(), opts)
	if err != nil {
		t.Fatal(err)
	}
	defer b.Destroy()
	execute(t, a, `m.count=42`)
	if r := execute(t, b, `return m.count`); r.Values[0].Number != "1" {
		t.Fatal("dependency module shared", r)
	}
	saved, err := a.Capture(context.Background(), a.Token(), []byte(`return {count=m.count}`))
	if err != nil || len(saved.Binding.PackageHashes) != 2 {
		t.Fatal("dependency checkpoint binding absent", err)
	}
	opts.Dependencies = []*archive.Package{fixture(t, 1, depText, `return 2`)}
	if s, err := New(context.Background(), opts); err == nil {
		s.Destroy()
		t.Fatal("wrong content hash admitted")
	}
	opts.Dependencies = []*archive.Package{dep, dep}
	if s, err := New(context.Background(), opts); err == nil {
		s.Destroy()
		t.Fatal("duplicate dependency admitted")
	}
}
