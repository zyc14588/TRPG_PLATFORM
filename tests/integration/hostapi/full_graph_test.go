//go:build integration

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package hostapi_test

import (
	"bytes"
	"context"
	"crypto/ed25519"
	"crypto/sha256"
	"encoding/json"
	"fmt"
	"os"
	"reflect"
	"strings"
	"syscall"
	"testing"

	host "github.com/zyc14588/TRPG_PLATFORM/internal/hostapi"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/vm"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/archive"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/capability"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/dependency"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/extension"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/install"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	fixtures "github.com/zyc14588/TRPG_PLATFORM/internal/package/testdata/install"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/object"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/postgres"
)

const fullRootID = "example.test/full-root"
const fullLibraryID = "example.test/full-library"
const fullContentID = "example.test/full-content"
const fullAssetsID = "example.test/full-assets"
const fullUIID = "example.test/full-ui"
const fullCredential = store.Credential("b014-synthetic-fixture-operator")

func fullState(n int64) checkpoint.Value {
	return checkpoint.Object(map[string]checkpoint.Value{"counter": checkpoint.Int(n)})
}
func fullCommand(delta int64) checkpoint.Value {
	return checkpoint.Object(map[string]checkpoint.Value{"delta": checkpoint.Int(delta)})
}

func fullPackage(t *testing.T, id, kind, source string, direct []string, nodes []dependency.LockedPackage, files map[string][]byte) *archive.Package {
	t.Helper()
	manifest := fmt.Sprintf("schema_version=1\nartifact_type=\"package\"\npackage_id=%q\npackage_kind=%q\nversion=\"1.0.0\"\ndisplay_name=\"Synthetic full graph fixture\"\n", id, kind)
	if source != "" {
		manifest += "entrypoint=\"lua/main.lua\"\nlua_profile=\"platform-lua-5.5-p1\"\n[host_api]\nmajor=1\nmin_minor=0\nmax_minor=0\n"
		files["lua/main.lua"] = []byte(source)
	}
	manifest += "[build]\nsource=\"fixture-local\"\nrevision=\"B014\"\nbuilder=\"full-graph-fixture/1\"\n[rights]\nauthors=[\"Synthetic full graph fixture\"]\nsource=\"original\"\nlicense_expression=\"PolyForm-Noncommercial-1.0.0\"\n[capabilities]\nrequired=[]\n"
	if id == fullRootID {
		manifest = strings.Replace(manifest, "required=[]", "required=[\"host.state\",\"host.event\",\"host.db\"]", 1)
	}
	for _, d := range direct {
		manifest += fmt.Sprintf("\n[[dependencies]]\npackage_id=%q\nversion=\"1.0.0\"\noptional=false\nfeatures=[\"fixture\"]\n", d)
	}
	files[archive.ManifestPath] = []byte(manifest)
	root, err := dependency.NewLockedPackage(id, "1.0.0", "sha256:"+strings.Repeat("0", 64), []string{"fixture"}, direct)
	if err != nil {
		t.Fatal(err)
	}
	lock, err := dependency.BuildExactLock(id, append([]dependency.LockedPackage{root}, nodes...))
	if err != nil {
		t.Fatal(err)
	}
	p, err := archive.FromFiles(files, lock, extension.DefaultSupport)
	if err != nil {
		t.Fatal(err)
	}
	return p
}

func fullPackages(t *testing.T) (*archive.Package, []*archive.Package) {
	t.Helper()
	role := []byte(`{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"string","const":"1.0.0"}`)
	lib := fullPackage(t, fullLibraryID, "library", `return {version="1.0.0",borrow=function() return host.state.get({"counter"}) end}`, nil, nil, map[string][]byte{"schemas/role.schema.json": role})
	node, _ := lib.ExactLock().Package(fullLibraryID)
	content := fullPackage(t, fullContentID, "content", "", []string{fullLibraryID}, []dependency.LockedPackage{node}, map[string][]byte{"schemas/role.schema.json": role, "content/fixture.txt": []byte("content")})
	assets := fullPackage(t, fullAssetsID, "assets", "", nil, nil, map[string][]byte{"schemas/role.schema.json": role, "assets/fixture.txt": []byte("assets")})
	ui := fullPackage(t, fullUIID, "ui-extension", "", nil, nil, map[string][]byte{"schemas/role.schema.json": role, "ui/fixture.txt": []byte("ui")})
	deps := []*archive.Package{assets, content, lib, ui}
	nodes := []dependency.LockedPackage{}
	for _, p := range deps {
		d, _ := p.Manifest()
		n, _ := p.ExactLock().Package(d.Package.PackageID)
		nodes = append(nodes, n)
	}
	source := `local lib=require("example.test/full-library:lua.main");assert(lib.version=="1.0.0")
local M={};local function state() return {counter=host.state.get({"counter"})} end
for _,n in ipairs({"on_session_create","on_session_restore","on_session_start","list_legal_actions","restore_checkpoint","resume_continuation","on_session_end","cleanup"}) do M[n]=function() return {} end end
M.create_checkpoint=state;M.project_view=state;M.on_safe_migration_boundary=function() return true end
M.validate_command=function(c) return math.type(c.delta)=="integer" and c.delta>=1 end
M.execute_command=function(c)
 if c.forge then return lib.borrow() end
 if c.passive_data then host.db.put("example.test/full-content/docs","one",{counter=1}) end
 local n=host.state.get({"counter"})+c.delta;host.state.put({"counter"},n);host.db.put("docs","one",{counter=n});host.event.emit("change",{counter=n});return n
end;return M`
	state := []byte(`{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","properties":{"counter":{"type":"integer","minimum":0}},"required":["counter"],"additionalProperties":false}`)
	files := map[string][]byte{"schemas/state.schema.json": state, "schemas/row.schema.json": state, "schemas/event.schema.json": state, "schemas/result.schema.json": []byte(`{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"integer"}`), "schemas/lifecycle.schema.json": []byte(`{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false}`), "schemas/safe.schema.json": []byte(`{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"boolean"}`)}
	return fullPackage(t, fullRootID, "game-system", source, []string{fullAssetsID, fullContentID, fullUIID}, nodes, files), deps
}

func fullPolicy(t *testing.T, root *archive.Package, deps []*archive.Package, runtime install.RuntimeConfig) (*install.Policy, map[string]install.Evidence) {
	t.Helper()
	caps := []string{"host.state", "host.event", "host.db"}
	config := install.PolicyConfig{Context: "ci", HostMajor: profile.HostMajor, HostMinor: profile.HostMinor, Artifacts: map[string]install.Approval{}, Host: &install.HostAuthorization{Trust: map[capability.TrustLevel][]string{}, Execution: caps, RunnerHash: runtime.SHA256, Limits: runtime.Limits}}
	for _, level := range []capability.TrustLevel{capability.TrustOfficial, capability.TrustSigned, capability.TrustPrivateUnverified, capability.TrustDevelopment} {
		config.Host.Trust[level] = append([]string(nil), caps...)
	}
	for _, p := range deps {
		d, _ := p.Manifest()
		a := install.Approval{RightsDigest: install.RightsDigest(*d.Package), Retention: "synthetic", Safety: "ACTIVE"}
		if d.Package.Entrypoint != "" {
			a.Tests = []install.Test{{Name: "library-pure", Source: []byte("return true")}}
		}
		config.Artifacts[string(p.ArtifactIdentity().Digest())] = a
	}
	ref := func(name string, seed checkpoint.Value) install.SchemaReference {
		path := "schemas/" + name + ".schema.json"
		entry, _ := root.Entry(path)
		return install.SchemaReference{PackageID: fullRootID, Path: path, Digest: checkpoint.Hash(entry.Bytes()), Seed: seed}
	}
	empty := checkpoint.Object(map[string]checkpoint.Value{})
	state := ref("state", fullState(1))
	result := ref("result", checkpoint.Int(1))
	row := ref("row", fullState(1))
	contract := &install.HostContract{State: state, Result: result, Results: map[string]install.SchemaReference{}, Namespaces: map[string]install.NamespaceContract{fullRootID + "/docs": {Schema: row, Indices: map[string][]string{"counter": {"counter"}}, MaxRows: 128, MaxBytes: 256 << 10}}, Events: map[string]install.SchemaReference{fullRootID + "/change": ref("event", fullState(1))}, Budget: host.DefaultBudget()}
	count := int64(1)
	commandSeen := false
	tests := []install.Test{{Name: "pure", Source: []byte("return true")}}
	for _, name := range profile.StandardCallbackNames() {
		if name == "validate_command" || name == "execute_command" {
			if commandSeen {
				continue
			}
			commandSeen = true
			name = "command"
		}
		input, expected := empty, empty
		switch name {
		case "command":
			count++
			input = fullCommand(1)
			expected = checkpoint.Int(count)
		case "project_view", "create_checkpoint":
			expected = fullState(count)
			contract.Results[name] = state
		case "on_safe_migration_boundary":
			expected = checkpoint.Bool(true)
			contract.Results[name] = ref("safe", expected)
		default:
			contract.Results[name] = ref("lifecycle", empty)
		}
		tests = append(tests, install.Test{Name: "case-" + name, Host: &install.HostCase{Callback: name, Input: input, Expected: expected}})
	}
	d, _ := root.Manifest()
	config.Artifacts[string(root.ArtifactIdentity().Digest())] = install.Approval{RightsDigest: install.RightsDigest(*d.Package), Retention: "synthetic", Safety: "ACTIVE", Host: contract, Tests: tests}
	pubSeed, certSeed := sha256.Sum256([]byte("B014 synthetic publisher")), sha256.Sum256([]byte("B014 synthetic certification"))
	pub, cert := ed25519.NewKeyFromSeed(pubSeed[:]), ed25519.NewKeyFromSeed(certSeed[:])
	config.Keys = map[string]install.Key{"fixture-publisher": {Public: pub.Public().(ed25519.PublicKey), Publisher: "example.test", State: "ACTIVE", NotBefore: 1, NotAfter: 4000000000}, "fixture-certification": {Public: cert.Public().(ed25519.PublicKey), Certification: true, State: "ACTIVE", NotBefore: 1, NotAfter: 4000000000}}
	policy, err := install.NewPolicy(config)
	if err != nil {
		t.Fatal(err)
	}
	const at = int64(1700000000)
	p, c := &install.Attestation{KeyID: "fixture-publisher", SignedAt: at}, &install.Attestation{KeyID: "fixture-certification", SignedAt: at}
	raw, err := policy.SigningBytes("publisher", root, at)
	if err != nil {
		t.Fatal(err)
	}
	p.Signature = ed25519.Sign(pub, raw)
	raw, err = policy.SigningBytes("certification", root, at)
	if err != nil {
		t.Fatal(err)
	}
	c.Signature = ed25519.Sign(cert, raw)
	return policy, map[string]install.Evidence{string(root.ArtifactIdentity().Digest()): {Publisher: p, Certification: c}}
}

type fullEnvironment struct {
	root       *archive.Package
	deps       []*archive.Package
	session    *install.InstalledSession
	repository *postgres.HostRepository
	options    host.Options
}

func fullSetup(t *testing.T) *fullEnvironment {
	t.Helper()
	ctx := context.Background()
	root, deps := fullPackages(t)
	raw, err := os.ReadFile(runner)
	if err != nil {
		t.Fatal(err)
	}
	runtime := install.RuntimeConfig{Runner: runner, SHA256: checkpoint.Hash(raw), Limits: profile.DefaultLimits()}
	policy, evidence := fullPolicy(t, root, deps, runtime)
	workspace := fmt.Sprintf("%s-full-%d", runID, seq.Add(1))
	storage, staging := t.TempDir(), t.TempDir()
	for _, dir := range []string{storage, staging} {
		if err := os.Chmod(dir, 0700); err != nil {
			t.Fatal(err)
		}
	}
	objects, err := object.Open(storage)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { objects.Close() })
	ir, err := postgres.OpenInstallationRepository(ctx, dsn, objects, extension.DefaultSupport, nil)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { ir.Close() })
	if err := ir.Bootstrap(ctx); err != nil {
		t.Fatal(err)
	}
	if err := ir.ProvisionWorkspace(ctx, workspace); err != nil {
		t.Fatal(err)
	}
	repo, err := postgres.OpenHostRepository(ctx, dsn, nil)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { repo.Close() })
	if err := repo.Bootstrap(ctx); err != nil {
		t.Fatal(err)
	}
	access, err := store.NewAccess(map[store.Credential][]store.Membership{fullCredential: {{Principal: "operator", Workspace: workspace, Read: true, Install: true}}})
	if err != nil {
		t.Fatal(err)
	}
	reader, err := store.NewReader(ir, objects, access, extension.DefaultSupport)
	if err != nil {
		t.Fatal(err)
	}
	executions := []install.Execution{}
	observe := func(v install.Execution) error { executions = append(executions, v); return nil }
	t.Cleanup(func() {
		for _, v := range executions {
			if v.PID > 0 && syscall.Kill(v.PID, 0) != syscall.ESRCH {
				t.Errorf("production worker not reaped: %d", v.PID)
			}
		}
		t.Logf("FULL_GRAPH_WORKERS_REAPED %d", len(executions))
	})
	installer, err := install.New(install.Options{StagingRoot: staging, Policy: policy, Access: access, Objects: objects, Repository: ir, Support: extension.DefaultSupport, Runtime: runtime, Observe: func(string) error { return nil }, Execution: observe})
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
	request := install.Request{Credential: fullCredential, Workspace: workspace, ID: "full-graph-install", Root: input(root)}
	identities := []string{}
	for _, p := range deps {
		request.Dependencies = append(request.Dependencies, input(p))
		identities = append(identities, string(p.ArtifactIdentity().Digest()))
	}
	if _, err := installer.Install(ctx, request); err != nil {
		t.Fatalf("signed full graph quarantine or installation failed: %v; executions=%+v", err, executions)
	}
	factory, err := install.NewSessionFactory(install.SessionOptions{Reader: reader, Policy: policy, Repository: repo, Runtime: runtime, Execution: observe, Audit: func(data.Audit) error { return nil }, Validate: func(context.Context, data.Commit) error { return nil }})
	if err != nil {
		t.Fatal(err)
	}
	session, err := factory.Create(ctx, install.SessionRequest{Credential: fullCredential, Workspace: workspace, Session: "passive-full-graph", Root: string(root.ArtifactIdentity().Digest()), Dependencies: identities, Evidence: evidence})
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		if err := session.Close(); err != nil {
			t.Error(err)
		}
	})
	inspection, err := ir.InspectInstallation(ctx, workspace)
	if err != nil {
		t.Fatal(err)
	}
	if inspection.Artifacts != 5 || inspection.Grants != 5 || inspection.DataTargets != 5 {
		t.Fatal("full installed graph evidence missing", inspection)
	}
	bind := func(name string, seed checkpoint.Value) host.Schema {
		path := "schemas/" + name + ".schema.json"
		entry, _ := root.Entry(path)
		s, err := host.BindSchema(root, path, checkpoint.Hash(entry.Bytes()), seed)
		if err != nil {
			t.Fatal(err)
		}
		return s
	}
	state, result := bind("state", fullState(1)), bind("result", checkpoint.Int(1))
	pkgs := map[string]*archive.Package{fullRootID: root}
	for _, p := range deps {
		d, _ := p.Manifest()
		pkgs[string(d.Package.PackageID)] = p
	}
	options := host.Options{Session: session.VM, Binding: session.Binding, Packages: pkgs, Repository: repo, StateSchema: state, ResultSchema: result, ResultSchemas: map[string]host.Schema{"create_checkpoint": state, "project_view": state}, Namespaces: map[string]host.Namespace{fullRootID + "/docs": {Schema: bind("row", fullState(1)), Indices: map[string][]string{"counter": {"counter"}}, MaxRows: 128, MaxBytes: 256 << 10}}, EventSchemas: map[string]host.Schema{fullRootID + "/change": bind("event", fullState(1))}, Budget: host.DefaultBudget(), Audit: func(data.Audit) error { return nil }, Validate: func(context.Context, data.Commit) error { return nil }}
	return &fullEnvironment{root: root, deps: deps, session: session, repository: repo, options: options}
}

func TestInstalledPassiveFiveRoleGraphCreationAndCommandCommit(t *testing.T) {
	e := fullSetup(t)
	s := e.session
	roles := map[string]bool{}
	for _, p := range append([]*archive.Package{e.root}, e.deps...) {
		doc, _ := p.Manifest()
		roles[string(doc.Package.PackageKind)] = true
		if doc.Package.PackageKind == "content" || doc.Package.PackageKind == "assets" || doc.Package.PackageKind == "ui-extension" {
			if doc.Package.Entrypoint != "" {
				t.Fatal("passive fixture has script")
			}
			for _, entry := range p.Entries() {
				if strings.HasSuffix(entry.Path(), ".lua") {
					t.Fatal("passive fixture has Lua")
				}
			}
		}
	}
	if len(roles) != 5 || len(s.VM.PackageHashes()) != 5 || len(s.VM.ModuleBindings()) != 2 {
		t.Fatal("package and module sets conflated", roles, s.VM.PackageHashes(), s.VM.ModuleBindings())
	}
	if _, err := host.New(e.options); err != nil {
		t.Fatal("complete graph denied", err)
	}
	result, err := s.Commands.Execute(context.Background(), s.VM.Token(), host.Command{ID: "full-command", Principal: "operator", ExpectedVersion: 1, Input: fullCommand(2)})
	if err != nil || result.Version != 2 || !reflect.DeepEqual(result.Result, checkpoint.Int(3)) {
		t.Fatal(result, err)
	}
	x := inspect(t, e.repository, s.Binding)
	if x.Version != 2 || x.Documents != 1 || x.Events != 1 || x.Requests != 1 || !reflect.DeepEqual(x.State, fullState(3)) {
		t.Fatal("real full graph command did not commit", x)
	}
	read, err := s.Commands.Read(context.Background(), s.VM.Token(), host.Command{Callback: "create_checkpoint", ID: "full-read", Principal: "operator", ExpectedVersion: 2, Input: checkpoint.Object(nil)})
	if err != nil || !reflect.DeepEqual(read.Result, fullState(3)) {
		t.Fatal("full graph read failed", read, err)
	}
	raw, err := json.Marshal(x.State)
	if err != nil {
		t.Fatal(err)
	}
	t.Logf("FULL_GRAPH_COMMIT graph=%s packages=5 script_modules=2 state_hash=%s", s.Binding.GraphHash, checkpoint.Hash(raw))
}

func TestFullGraphOptionsRejectMissingExtraAndSubstitutedPackages(t *testing.T) {
	e := fullSetup(t)
	for _, id := range []string{fullRootID, fullLibraryID, fullContentID, fullAssetsID, fullUIID} {
		t.Run("missing-"+id, func(t *testing.T) {
			o := e.options
			o.Packages = map[string]*archive.Package{}
			for k, p := range e.options.Packages {
				if k != id {
					o.Packages[k] = p
				}
			}
			if _, err := host.New(o); profile.Code(err) != profile.ErrConfiguration {
				t.Fatal("missing graph package accepted", err)
			}
		})
	}
	t.Run("extra", func(t *testing.T) {
		o := e.options
		o.Packages = map[string]*archive.Package{}
		for k, p := range e.options.Packages {
			o.Packages[k] = p
		}
		o.Packages["example.test/extra"] = e.root
		if _, err := host.New(o); profile.Code(err) != profile.ErrConfiguration {
			t.Fatal("extra package accepted", err)
		}
	})
	for _, c := range []struct{ id, kind string }{{fullAssetsID, "assets"}, {fullUIID, "ui-extension"}, {fullContentID, "content"}} {
		t.Run("substitution-"+c.id, func(t *testing.T) {
			p := fullPackage(t, c.id, c.kind, "", nil, nil, map[string][]byte{"data/changed.txt": []byte("different authenticated content")})
			o := e.options
			o.Packages = map[string]*archive.Package{}
			for k, p := range e.options.Packages {
				o.Packages[k] = p
			}
			o.Packages[c.id] = p
			if _, err := host.New(o); profile.Code(err) != profile.ErrConfiguration {
				t.Fatal("passive hash substitution accepted", err)
			}
		})
	}
	if x := inspect(t, e.repository, e.session.Binding); x.Version != 1 || x.Requests != 0 || x.Events != 0 || x.Documents != 0 {
		t.Fatal("configuration rejection changed state", x)
	}
}

func TestFullGraphBindingCopiesCannotCreatePassiveScriptAuthority(t *testing.T) {
	e := fullSetup(t)
	s := e.session.VM
	before := s.PackageHashes()
	copy := s.PackageHashes()
	delete(copy, fullContentID)
	copy[fullAssetsID] = "sha256:" + strings.Repeat("a", 64)
	copy["example.test/forged"] = "sha256:" + strings.Repeat("b", 64)
	modules := s.ModuleBindings()
	modules[fullContentID+":lua/main.lua"] = vm.ModuleIdentity{PackageID: fullContentID, ContentHash: before[fullContentID]}
	if !reflect.DeepEqual(before, s.PackageHashes()) || len(s.ModuleBindings()) != 2 {
		t.Fatal("returned maps changed VM authority")
	}
	for _, m := range s.ModuleBindings() {
		if m.PackageID != fullRootID && m.PackageID != fullLibraryID {
			t.Fatal("passive package gained module identity", m)
		}
	}
	service, err := host.New(e.options)
	if err != nil {
		t.Fatal(err)
	}
	r, err := service.Execute(context.Background(), s.Token(), host.Command{ID: "copy-owned-command", Principal: "operator", ExpectedVersion: 1, Input: fullCommand(1)})
	if err != nil || r.Version != 2 {
		t.Fatal("copy mutation affected command", r, err)
	}
}

func TestFullGraphDoesNotGrantLibraryOrPassivePackageAuthority(t *testing.T) {
	for name, flag := range map[string]string{"library-origin": "forge", "passive-package-data": "passive_data"} {
		t.Run(name, func(t *testing.T) {
			e := fullSetup(t)
			s := e.session.VM
			pid := s.PID()
			input := fullCommand(1)
			input.Table[flag] = checkpoint.Bool(true)
			before := inspect(t, e.repository, e.session.Binding)
			r, err := e.session.Commands.Execute(context.Background(), s.Token(), host.Command{ID: "origin-denied", Principal: "operator", ExpectedVersion: 1, Input: input})
			if profile.Code(err) != profile.ErrCapability || r.Version != 0 {
				t.Fatal("complete graph membership granted callback authority", r, err)
			}
			after := inspect(t, e.repository, e.session.Binding)
			if !reflect.DeepEqual(before, after) {
				t.Fatal("denied origin leaked an effect")
			}
			if syscall.Kill(pid, 0) != syscall.ESRCH {
				t.Fatal("contaminated runner not reaped")
			}
		})
	}
}

func TestPassiveFullGraphRetainsDefaultZeroAuthorization(t *testing.T) {
	root, deps := fullPackages(t)
	session, err := vm.New(context.Background(), vm.Options{SessionID: "full-graph-zero", Runner: runner, Package: root, Dependencies: deps, State: vm.State{Version: 1, Value: fullState(1)}, Limits: profile.DefaultLimits(), Audit: func(profile.Audit) error { return nil }})
	if session != nil {
		session.Destroy()
	}
	if profile.Code(err) != profile.ErrCapability {
		t.Fatal("complete graph implicitly enabled required Host authority", err)
	}
}
