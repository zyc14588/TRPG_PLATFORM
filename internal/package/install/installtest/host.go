// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// Package installtest builds synthetic conformance inputs. Explicit operator
// policy is returned separately from package bytes; identity implies no grant.
package installtest

import (
	"encoding/json"
	"fmt"
	"strings"

	"github.com/zyc14588/TRPG_PLATFORM/internal/hostapi"
	fixture "github.com/zyc14588/TRPG_PLATFORM/internal/hostapi/hostapitest"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/archive"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/capability"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/install"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/manifest"
	fixtures "github.com/zyc14588/TRPG_PLATFORM/internal/package/testdata/install"
)

const LibraryID = "example.test/library"

func Graph(source string) (*archive.Package, *archive.Package, error) {
	return GraphWithLibrary(source, []string{"host.rules"})
}
func GraphWithLibrary(source string, names []string) (*archive.Package, *archive.Package, error) {
	files := fixtures.Files(LibraryID, "library", `return {check=function(n) return host.rules.call("integer.compare",n,n+1) end}`)
	if names == nil {
		names = []string{}
	}
	raw, _ := json.Marshal(names)
	files[archive.ManifestPath] = []byte(strings.Replace(string(files[archive.ManifestPath]), "required = []", "required = "+string(raw), 1))
	dep, err := fixtures.Build(files)
	if err != nil {
		return nil, nil, err
	}
	if source == "" {
		source = fixture.Source(`assert(require("example.test/library:lua.main").check(before)==-1)`)
	}
	source = strings.Replace(source, "M.execute_command=function(command)", `M.create_checkpoint=function() return {counter=host.state.get({"counter"})} end
M.execute_command=function(command)`, 1)
	p, err := fixture.Package("", source, map[string][]byte{"schemas/lifecycle.schema.json": []byte(`{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false}`)})
	if err != nil {
		return nil, nil, err
	}
	rootFiles := map[string][]byte{}
	for _, entry := range p.Entries() {
		rootFiles[entry.Path()] = entry.Bytes()
	}
	rootFiles[archive.ManifestPath] = p.ManifestBytes()
	rootFiles[archive.ManifestPath] = append(rootFiles[archive.ManifestPath], []byte(fmt.Sprintf("\n[[dependencies]]\npackage_id=%q\nversion=\"1.0.0\"\noptional=false\nfeatures=[]\n", LibraryID))...)
	root, err := fixtures.Build(rootFiles, dep.ExactLock().Packages()...)
	return root, dep, err
}
func Config(root, dep *archive.Package, runtime install.RuntimeConfig) (install.PolicyConfig, error) {
	ref := func(name string, seed checkpoint.Value) (install.SchemaReference, error) {
		path := "schemas/" + name + ".schema.json"
		entry, ok := root.Entry(path)
		if !ok {
			return install.SchemaReference{}, fmt.Errorf("missing schema")
		}
		return install.SchemaReference{PackageID: fixture.PackageID, Path: path, Digest: checkpoint.Hash(entry.Bytes()), Seed: seed}, nil
	}
	state, err := ref("state", fixture.State())
	if err != nil {
		return install.PolicyConfig{}, err
	}
	result, _ := ref("result", checkpoint.Int(1))
	row, _ := ref("row", checkpoint.Object(map[string]checkpoint.Value{"score": checkpoint.Int(1)}))
	event, _ := ref("event", fixture.State())
	intent, _ := ref("intent", checkpoint.Object(map[string]checkpoint.Value{"value": checkpoint.Int(1)}))
	input, _ := ref("named_input", checkpoint.Object(map[string]checkpoint.Value{"key": checkpoint.Text("one"), "delta": checkpoint.Int(1)}))
	c := &install.HostContract{State: state, Result: result, Results: map[string]install.SchemaReference{}, Namespaces: map[string]install.NamespaceContract{fixture.PackageID + "/docs": {Schema: row, Indices: map[string][]string{"score": {"score"}}, MaxRows: 128, MaxBytes: 256 << 10}}, Events: map[string]install.SchemaReference{fixture.PackageID + "/change": event}, Intents: map[string]install.SchemaReference{fixture.PackageID + "/task": intent, fixture.PackageID + "/ai": intent, fixture.PackageID + "/continuation": intent}, Named: map[string]install.NamedContract{fixture.PackageID + "/increase": {PackageID: fixture.PackageID, ID: "increase", Plan: "quantity-add", Input: input, Output: result}}, Budget: hostapi.DefaultBudget()}
	empty := checkpoint.Object(map[string]checkpoint.Value{})
	// All lifecycle callbacks share an artifact-local, immutable result schema.
	// A permissive schema is still explicit operator configuration, not a grant.
	entry, ok := root.Entry("schemas/lifecycle.schema.json")
	if !ok {
		return install.PolicyConfig{}, fmt.Errorf("missing lifecycle schema")
	}
	object := install.SchemaReference{PackageID: fixture.PackageID, Path: entry.Path(), Digest: checkpoint.Hash(entry.Bytes()), Seed: empty}
	tests := []install.Test{{Name: "raw-pure", Source: []byte("return true")}}
	counter := int64(1)
	commandAdded := false
	for _, name := range profile.StandardCallbackNames() {
		if name == "validate_command" || name == "execute_command" {
			if commandAdded {
				continue
			}
			name = "command"
			commandAdded = true
		}
		expected := empty
		if name == "command" {
			counter++
			expected = checkpoint.Int(counter)
		} else if name == "create_checkpoint" {
			expected = checkpoint.Object(map[string]checkpoint.Value{"counter": checkpoint.Int(counter)})
			c.Results[name] = state
		} else {
			c.Results[name] = object
		}
		tests = append(tests, install.Test{Name: "case-" + name, Host: &install.HostCase{Callback: name, Input: empty, Expected: expected, Time: 1000, Random: []int64{7}}})
	}
	config := install.PolicyConfig{Context: "ci", HostMajor: profile.HostMajor, HostMinor: profile.HostMinor, Artifacts: map[string]install.Approval{string(root.ArtifactIdentity().Digest()): {RightsDigest: install.RightsDigest(*mustManifest(root)), Retention: "synthetic", Safety: "ACTIVE", Tests: tests, Host: c}, string(dep.ArtifactIdentity().Digest()): {RightsDigest: install.RightsDigest(*mustManifest(dep)), Retention: "synthetic", Safety: "ACTIVE", Tests: []install.Test{{Name: "library-exports", Source: []byte(`local m=require("lua.main");return type(m.check)=="function"`)}}}}, Host: &install.HostAuthorization{Trust: map[capability.TrustLevel][]string{}, Execution: append([]string(nil), fixture.AllCapabilities...)}}
	for _, level := range []capability.TrustLevel{capability.TrustOfficial, capability.TrustSigned, capability.TrustPrivateUnverified, capability.TrustDevelopment} {
		config.Host.Trust[level] = append([]string(nil), fixture.AllCapabilities...)
	}
	config.Host.RunnerHash = runtime.SHA256
	config.Host.Limits = runtime.Limits
	return config, nil
}
func mustManifest(pkg *archive.Package) *manifest.Package { d, _ := pkg.Manifest(); return d.Package }
