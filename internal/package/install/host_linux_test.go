//go:build linux

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package install_test

import (
	"bytes"
	"context"
	"crypto/ed25519"
	"crypto/rand"
	"encoding/json"
	"errors"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"syscall"
	"testing"
	"time"

	fixture "github.com/zyc14588/TRPG_PLATFORM/internal/hostapi/hostapitest"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/archive"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/capability"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/extension"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/install"
	helper "github.com/zyc14588/TRPG_PLATFORM/internal/package/install/installtest"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	fixtures "github.com/zyc14588/TRPG_PLATFORM/internal/package/testdata/install"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/object"
)

const hostCredential = store.Credential("b013-unit-explicit-credential")

type hostEnvironment struct {
	root, dep *archive.Package
	config    install.PolicyConfig
	runtime   install.RuntimeConfig
	objects   *object.Directory
	staging   string
	access    *store.Access
}

func hostSetup(t *testing.T, source string, library []string) *hostEnvironment {
	t.Helper()
	root, dep, err := helper.GraphWithLibrary(source, library)
	if err != nil {
		t.Fatal(err)
	}
	path, err := filepath.Abs("../../..")
	if err != nil {
		t.Fatal(err)
	}
	runner, err := fixture.BuildRunner(path, t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	data, err := os.ReadFile(runner)
	if err != nil {
		t.Fatal(err)
	}
	runtime := install.RuntimeConfig{Runner: runner, SHA256: checkpoint.Hash(data), Limits: profile.DefaultLimits()}
	config, err := helper.Config(root, dep, runtime)
	if err != nil {
		t.Fatal(err)
	}
	staging, storage := t.TempDir(), t.TempDir()
	os.Chmod(staging, 0700)
	os.Chmod(storage, 0700)
	objects, err := object.Open(storage)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { objects.Close() })
	access, err := store.NewAccess(map[store.Credential][]store.Membership{hostCredential: {{Principal: "operator", Workspace: "unit", Install: true, Read: true}}})
	if err != nil {
		t.Fatal(err)
	}
	return &hostEnvironment{root, dep, config, install.RuntimeConfig{Runner: runner, SHA256: checkpoint.Hash(data), Limits: profile.DefaultLimits()}, objects, staging, access}
}
func (e *hostEnvironment) install(t *testing.T, c install.PolicyConfig, evidence map[string]install.Evidence) (*helper.MemoryInstallation, []install.Execution, error) {
	t.Helper()
	childrenBefore := ownedChildren(t)
	t.Cleanup(func() {
		after := ownedChildren(t)
		for pid := range after {
			if !childrenBefore[pid] {
				t.Fatalf("validation left owned child process %s", pid)
			}
		}
	})
	policy, err := install.NewPolicy(c)
	if err != nil {
		return nil, nil, err
	}
	memory := helper.NewMemoryInstallation()
	var executions []install.Execution
	i, err := install.New(install.Options{StagingRoot: e.staging, Objects: e.objects, Access: e.access, Repository: memory, Policy: policy, Support: extension.DefaultSupport, Runtime: e.runtime, Observe: func(string) error { return nil }, Execution: func(v install.Execution) error { executions = append(executions, v); return nil }})
	if err != nil {
		t.Fatal(err)
	}
	input := func(p *archive.Package) install.Input {
		raw, err := fixtures.Archive(p)
		if err != nil {
			t.Fatal(err)
		}
		return install.Input{Archive: bytes.NewReader(raw), Evidence: evidence[string(p.ArtifactIdentity().Digest())]}
	}
	_, err = i.Install(context.Background(), install.Request{Credential: hostCredential, Workspace: "unit", ID: "request", Root: input(e.root), Dependencies: []install.Input{input(e.dep)}})
	for _, v := range executions {
		if v.Reaped && syscall.Kill(v.PID, 0) != syscall.ESRCH {
			t.Fatalf("runner %d survived validation", v.PID)
		}
	}
	return memory, executions, err
}
func ownedChildren(t *testing.T) map[string]bool {
	t.Helper()
	values := map[string]bool{}
	path := "/proc/" + strconv.Itoa(os.Getpid()) + "/task"
	tasks, err := os.ReadDir(path)
	if err != nil {
		t.Fatal(err)
	}
	for _, task := range tasks {
		raw, err := os.ReadFile(filepath.Join(path, task.Name(), "children"))
		if os.IsNotExist(err) {
			continue
		}
		if err != nil {
			t.Fatal(err)
		}
		for _, pid := range strings.Fields(string(raw)) {
			values[pid] = true
		}
	}
	return values
}
func TestTypedHostCertificationExecutesGraphAndReaps(t *testing.T) {
	e := hostSetup(t, "", []string{"host.rules"})
	memory, executions, err := e.install(t, e.config, nil)
	if err != nil {
		t.Fatal(err)
	}
	if len(memory.Publications) != 1 {
		t.Fatal("not published")
	}
	seen := map[string]bool{}
	reaped := 0
	for _, v := range executions {
		seen[v.Case] = true
		if v.Reaped {
			reaped++
		}
	}
	if !seen["host:case-command"] || !seen["host:case-create_checkpoint"] || !seen["host-quarantine-destroy"] || !seen["raw-pure"] || reaped != 3 {
		t.Fatal("actual Host/pure graph execution evidence missing", executions)
	}
	reader, _ := store.NewReader(memory, e.objects, e.access, extension.DefaultSupport)
	g, err := reader.LoadGraph(context.Background(), hostCredential, "unit", string(e.root.ArtifactIdentity().Digest()), []string{string(e.dep.ArtifactIdentity().Digest())})
	if err != nil {
		t.Fatal(err)
	}
	if len(g.Artifacts()) != 2 || len(g.Dependencies()) != 1 {
		t.Fatal("graph changed")
	}
	dependency := string(e.dep.ArtifactIdentity().Digest())
	root := string(e.root.ArtifactIdentity().Digest())
	for name, test := range map[string]struct {
		credential store.Credential
		workspace  string
		deps       []string
	}{
		"credential":  {store.Credential("wrong-unit-credential"), "unit", []string{dependency}},
		"workspace":   {hostCredential, "other", []string{dependency}},
		"missing":     {hostCredential, "unit", nil},
		"extra":       {hostCredential, "unit", []string{dependency, root}},
		"duplicate":   {hostCredential, "unit", []string{dependency, dependency}},
		"substituted": {hostCredential, "unit", []string{checkpoint.Hash([]byte("missing"))}},
	} {
		t.Run(name, func(t *testing.T) {
			if _, err := reader.LoadGraph(context.Background(), test.credential, test.workspace, root, test.deps); err == nil {
				t.Fatal("inexact or unauthorized installed graph loaded")
			}
		})
	}
}
func TestHostCertificationDeniesDefaultsIntersectionsAndUntypedSource(t *testing.T) {
	for _, name := range []string{"zero-default", "execution-denied", "trust-denied", "raw-source-nohost", "wrong-schema", "wrong-result", "missing-case", "source-plus-case", "runner-binding", "limits-binding", "callback-table"} {
		t.Run(name, func(t *testing.T) {
			e := hostSetup(t, "", []string{"host.rules"})
			if name == "callback-table" {
				e = hostSetup(t, "return {}", []string{"host.rules"})
			}
			id := string(e.root.ArtifactIdentity().Digest())
			a := e.config.Artifacts[id]
			switch name {
			case "zero-default":
				e.config.Host = nil
				a.Host = nil
				for j := range a.Tests {
					if a.Tests[j].Host != nil {
						a.Tests[j].Source = []byte("return true")
						a.Tests[j].Host = nil
					}
				}
			case "execution-denied":
				e.config.Host.Execution = []string{"host.state"}
			case "trust-denied":
				e.config.Host.Trust[capability.TrustDevelopment] = []string{"host.rules"}
			case "raw-source-nohost":
				a.Tests[0].Source = []byte("return host~=nil")
			case "wrong-schema":
				a.Host.State.Digest = checkpoint.Hash([]byte("wrong"))
			case "wrong-result":
				a.Tests[1].Host.Expected = checkpoint.Int(9)
			case "missing-case":
				a.Tests = a.Tests[:len(a.Tests)-1]
			case "source-plus-case":
				a.Tests[1].Source = []byte("return true")
			case "runner-binding":
				e.config.Host.RunnerHash = checkpoint.Hash([]byte("other runner"))
			case "limits-binding":
				e.config.Host.Limits.Instructions--
			}
			e.config.Artifacts[id] = a
			memory, _, err := e.install(t, e.config, nil)
			if err == nil || memory != nil && len(memory.Publications) != 0 {
				t.Fatal("denied certification published", name, err)
			}
		})
	}
}

func TestNilHostPolicyKeepsLegacySuiteAndAttestationBytes(t *testing.T) {
	pkg, err := fixtures.Build(fixtures.Files("test.publisher/legacy", "assets", ""))
	if err != nil {
		t.Fatal(err)
	}
	d, _ := pkg.Manifest()
	tests := []install.Test{{Name: "legacy", Source: []byte("return true")}}
	c := install.PolicyConfig{Context: "ci", Artifacts: map[string]install.Approval{string(pkg.ArtifactIdentity().Digest()): {RightsDigest: install.RightsDigest(*d.Package), Retention: "fixture", Safety: "ACTIVE", Tests: tests}}}
	p, err := install.NewPolicy(c)
	if err != nil {
		t.Fatal(err)
	}
	old, err := install.SigningBytes("certification", pkg, tests, 1)
	if err != nil {
		t.Fatal(err)
	}
	now, err := p.SigningBytes("certification", pkg, 1)
	if err != nil || !bytes.Equal(old, now) {
		t.Fatal("nil Host configuration changed legacy attestation bytes")
	}
	raw, _ := json.Marshal([]struct {
		Name                 string
		Source               []byte
		Capability, Behavior string
	}{{"legacy", []byte("return true"), "", ""}})
	if install.SuiteDigest(tests) != object.Hash(raw) {
		t.Fatal("nil Host case changed legacy suite wire digest")
	}
}
func TestInstalledGraphRequiresIdenticalFeatureAndVersionLocks(t *testing.T) {
	depFiles := fixtures.Files("test.publisher/dependency", "library", "")
	dep, err := fixtures.Build(depFiles)
	if err != nil {
		t.Fatal(err)
	}
	files := fixtures.Files("test.publisher/root", "assets", "")
	files[archive.ManifestPath] = append(files[archive.ManifestPath], []byte("\n[[dependencies]]\npackage_id=\"test.publisher/dependency\"\nversion=\"1.0.0\"\noptional=false\nfeatures=[]\n")...)
	root, err := fixtures.Build(files, dep.ExactLock().Packages()...)
	if err != nil {
		t.Fatal(err)
	}
	if err = store.VerifyExactGraph(root, []*archive.Package{dep}); err != nil {
		t.Fatal(err)
	}
	node := dep.ExactLock().Packages()[0]
	node.Features = []string{"extra"}
	files[archive.ManifestPath] = []byte(strings.Replace(string(files[archive.ManifestPath]), "features=[]", "features=[\"extra\"]", 1))
	featureRoot, err := fixtures.Build(files, node)
	if err != nil {
		t.Fatal(err)
	}
	if store.VerifyExactGraph(featureRoot, []*archive.Package{dep}) == nil {
		t.Fatal("dependency with different feature subgraph accepted")
	}
	depFiles[archive.ManifestPath] = []byte(strings.Replace(string(depFiles[archive.ManifestPath]), "version = \"1.0.0\"", "version = \"2.0.0\"", 1))
	other, err := fixtures.Build(depFiles)
	if err != nil {
		t.Fatal(err)
	}
	if store.VerifyExactGraph(root, []*archive.Package{other}) == nil {
		t.Fatal("dependency version substitution accepted")
	}
}
func TestHostDependencyCannotBorrowRootCapabilities(t *testing.T) {
	e := hostSetup(t, "", nil)
	memory, _, err := e.install(t, e.config, nil)
	if err == nil || len(memory.Publications) != 0 {
		t.Fatal("undeclared library callback borrowed root grant")
	}
}
func TestHostPolicyDeepCopyAndActualCertificationBinding(t *testing.T) {
	e := hostSetup(t, "", []string{"host.rules"})
	e.config.Context = "production"
	now := time.Now().Unix()
	pub, private, _ := ed25519.GenerateKey(rand.Reader)
	cert, certPrivate, _ := ed25519.GenerateKey(rand.Reader)
	e.config.Keys = map[string]install.Key{"publisher": {Public: pub, Publisher: "example.test", State: "ACTIVE", NotBefore: now - 60, NotAfter: now + 60}, "certification": {Public: cert, Certification: true, State: "ACTIVE", NotBefore: now - 60, NotAfter: now + 60}}
	p, err := install.NewPolicy(e.config)
	if err != nil {
		t.Fatal(err)
	}
	evidence := map[string]install.Evidence{}
	for _, pkg := range []*archive.Package{e.root, e.dep} {
		one, _ := p.SigningBytes("publisher", pkg, now)
		two, _ := p.SigningBytes("certification", pkg, now)
		evidence[string(pkg.ArtifactIdentity().Digest())] = install.Evidence{Publisher: &install.Attestation{KeyID: "publisher", SignedAt: now, Signature: ed25519.Sign(private, one)}, Certification: &install.Attestation{KeyID: "certification", SignedAt: now, Signature: ed25519.Sign(certPrivate, two)}}
	}
	memory, _, err := e.install(t, e.config, evidence)
	if err != nil || len(memory.Publications) != 1 {
		t.Fatal("actual signed Host suite failed", err)
	}
	before, _ := p.SigningBytes("certification", e.root, now)
	e.config.Host.Execution = e.config.Host.Execution[1:]
	id := string(e.root.ArtifactIdentity().Digest())
	a := e.config.Artifacts[id]
	a.Tests[1].Host.Time++
	e.config.Artifacts[id] = a
	after, _ := p.SigningBytes("certification", e.root, now)
	if !bytes.Equal(before, after) {
		t.Fatal("caller mutated immutable policy")
	}
	memory, _, err = e.install(t, e.config, evidence)
	if !errors.Is(err, install.ErrPolicy) || len(memory.Publications) != 0 {
		t.Fatal("changed Host input/context retained old signature", err)
	}
}
