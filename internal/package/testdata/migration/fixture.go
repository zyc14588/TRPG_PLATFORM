// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// Package migrationfixture builds synthetic, rights-declared migration pairs.
// Keys and credentials here are deterministic test data, never operator keys.
package migrationfixture

import (
	"crypto/ed25519"
	"crypto/sha256"
	"fmt"
	"strings"

	"github.com/zyc14588/TRPG_PLATFORM/internal/hostapi"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/archive"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/capability"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/dependency"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/extension"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/install"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	sessionmigration "github.com/zyc14588/TRPG_PLATFORM/internal/session/migration"
)

const PackageID = "example.test/migration"
const PrivateValue = "migration-fixture-gm-private"

type Graph struct {
	Root         *archive.Package
	Dependencies []*archive.Package
}
type Pair struct {
	Old, New Graph
	Config   install.PolicyConfig
	Evidence map[string]install.Evidence
}

// Options alters synthetic callback cases while retaining actual installation
// and certification. Undeclared uses the ordinary integer result schema rather
// than a boolean safe-boundary contract; target failure occurs only on migrated
// state outside the certification seed cases.
type Options struct {
	Safe, Critical, Undeclared, TargetRestoreFailure bool
}

func State(next bool, n int64) checkpoint.Value {
	field := "counter"
	if next {
		field = "total"
	}
	return checkpoint.Object(map[string]checkpoint.Value{field: checkpoint.Int(n), "secret": checkpoint.Text(PrivateValue)})
}
func Command(delta int64) checkpoint.Value {
	return checkpoint.Object(map[string]checkpoint.Value{"type": checkpoint.Text("increment"), "seat_id": checkpoint.Text("gm"), "correlation_id": checkpoint.Text("fixture"), "payload": checkpoint.Object(map[string]checkpoint.Value{"delta": checkpoint.Int(delta)})})
}
func (g Graph) Hash() string { h, _ := g.Root.ExactLock().Digest(); return string(h) }
func (g Graph) Identities() []string {
	ids := []string{}
	for _, p := range g.Dependencies {
		ids = append(ids, string(p.ArtifactIdentity().Digest()))
	}
	return ids
}
func (p Pair) Request(credential string, workspace, sid string, next bool) install.SessionRequest {
	g := p.Old
	if next {
		g = p.New
	}
	evidence := map[string]install.Evidence{}
	for _, pkg := range append([]*archive.Package{g.Root}, g.Dependencies...) {
		id := string(pkg.ArtifactIdentity().Digest())
		if e, ok := p.Evidence[id]; ok {
			evidence[id] = e
		}
	}
	return install.SessionRequest{Credential: store.Credential(credential), Workspace: workspace, Session: sid, Root: string(g.Root.ArtifactIdentity().Digest()), Dependencies: g.Identities(), Evidence: evidence, EpochEvidence: map[string]map[string]install.Evidence{p.Old.Hash(): p.graphEvidence(p.Old), p.New.Hash(): p.graphEvidence(p.New)}}
}
func (p Pair) graphEvidence(g Graph) map[string]install.Evidence {
	out := map[string]install.Evidence{}
	for _, v := range append([]*archive.Package{g.Root}, g.Dependencies...) {
		id := string(v.ArtifactIdentity().Digest())
		if e, ok := p.Evidence[id]; ok {
			out[id] = e
		}
	}
	return out
}
func (p Pair) Plan() sessionmigration.Plan {
	return sessionmigration.Plan{ID: "fixture-v1-to-v2", FromLock: p.Old.Hash(), ToLock: p.New.Hash(), StateFields: []sessionmigration.FieldMove{{From: "counter", To: "total"}}, Namespaces: []sessionmigration.NamespaceMove{{PackageID: PackageID, From: "docs", To: "records", Fields: []sessionmigration.FieldMove{{From: "score", To: "value"}}}}}
}
func schema(field string) []byte {
	return []byte(fmt.Sprintf(`{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","properties":{%q:{"type":"integer","minimum":0,"maximum":1000000000},"secret":{"type":"string","maxLength":128}},"required":[%q,"secret"],"additionalProperties":false}`, field, field))
}
func manifest(id, kind, version, source string, deps []string) string {
	text := fmt.Sprintf("schema_version=1\nartifact_type=\"package\"\npackage_id=%q\npackage_kind=%q\nversion=%q\ndisplay_name=\"Synthetic migration fixture\"\n", id, kind, version)
	if source != "" {
		text += "entrypoint=\"lua/main.lua\"\nlua_profile=\"platform-lua-5.5-p1\"\n[host_api]\nmajor=1\nmin_minor=0\nmax_minor=0\n"
	}
	text += "[build]\nsource=\"fixture-local\"\nrevision=\"migration-v1\"\nbuilder=\"migration-fixture/1\"\n[rights]\nauthors=[\"Synthetic migration fixture\"]\nsource=\"original\"\nlicense_expression=\"PolyForm-Noncommercial-1.0.0\"\n[capabilities]\nrequired=[]\n"
	if id == PackageID {
		text = strings.Replace(text, "required=[]", "required=[\"host.state\",\"host.event\",\"host.db\",\"host.task\"]", 1)
	}
	for _, dep := range deps {
		text += fmt.Sprintf("\n[[dependencies]]\npackage_id=%q\nversion=%q\noptional=false\nfeatures=[\"fixture\"]\n", dep, version)
	}
	return text
}
func build(id, kind, version, source string, files map[string][]byte, direct []string, nodes []dependency.LockedPackage) (*archive.Package, error) {
	files[archive.ManifestPath] = []byte(manifest(id, kind, version, source, direct))
	if source != "" {
		files["lua/main.lua"] = []byte(source)
	}
	node, err := dependency.NewLockedPackage(id, version, "sha256:"+strings.Repeat("0", 64), []string{"fixture"}, direct)
	if err != nil {
		return nil, err
	}
	lock, err := dependency.BuildExactLock(id, append([]dependency.LockedPackage{node}, nodes...))
	if err != nil {
		return nil, err
	}
	return archive.FromFiles(files, lock, extension.DefaultSupport)
}
func graph(next, safe, critical, undeclared, restoreFailure bool) (Graph, error) {
	version, field, rowField, namespace := "1.0.0", "counter", "score", "docs"
	if next {
		version, field, rowField, namespace = "1.1.0", "total", "value", "records"
	}
	roleSchema := []byte(fmt.Sprintf(`{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"string","const":%q}`, version))
	lib, err := build("example.test/migration-library", "library", version, "return {version="+fmt.Sprintf("%q", version)+"}", map[string][]byte{"schemas/role.schema.json": roleSchema}, nil, nil)
	if err != nil {
		return Graph{}, err
	}
	libNode, _ := lib.ExactLock().Package("example.test/migration-library")
	content, err := build("example.test/migration-content", "content", version, "", map[string][]byte{"schemas/role.schema.json": roleSchema, "content/readme.txt": []byte(version)}, []string{"example.test/migration-library"}, []dependency.LockedPackage{libNode})
	if err != nil {
		return Graph{}, err
	}
	assets, err := build("example.test/migration-assets", "assets", version, "", map[string][]byte{"schemas/role.schema.json": roleSchema, "assets/readme.txt": []byte(version)}, nil, nil)
	if err != nil {
		return Graph{}, err
	}
	ui, err := build("example.test/migration-ui", "ui-extension", version, "", map[string][]byte{"schemas/role.schema.json": roleSchema, "ui/readme.txt": []byte(version)}, nil, nil)
	if err != nil {
		return Graph{}, err
	}
	safeText := "false"
	if safe {
		safeText = "true"
	}
	if undeclared && !next {
		safeText = "1"
	}
	restoreGuard := ""
	if restoreFailure && next {
		restoreGuard = "assert(input.total~=8)"
	}
	criticalText := ""
	if critical {
		criticalText = "local task=host.task.create({value=next});host.task.continuation(task,{value=next})"
	}
	source := fmt.Sprintf(`local M={}
local function state() return {%s=host.state.get({%q}),secret=host.state.get({"secret"})} end
for _,n in ipairs({"on_session_create","on_session_start","list_legal_actions","resume_continuation","cleanup"}) do M[n]=function(...) return {} end end
M.on_session_restore=function(input) assert(input.%s==host.state.get({%q}));%s;return {} end
M.restore_checkpoint=M.on_session_restore
M.project_view=function(input) local v=state();if input.seat_id=="player" then v.secret="redacted" end;return v end
M.create_checkpoint=state
M.on_safe_migration_boundary=function() return %s end
M.on_session_end=function() host.event.emit("change",state());return {} end
M.validate_command=function(command) return command.type=="increment" and math.type(command.payload.delta)=="integer" and command.payload.delta>=1 and command.payload.delta<=1000 end
M.execute_command=function(command)
 local next=host.state.get({%q})+command.payload.delta
 host.state.put({%q},next);host.db.put(%q,"one",{%s=next})
 host.db.named(%q,{key="one",delta=command.payload.delta})
 host.event.emit("change",state());%s;return next
end
return M`, field, field, field, field, restoreGuard, safeText, field, field, namespace, rowField, "increase", criticalText)
	files := map[string][]byte{"schemas/state.schema.json": schema(field), "schemas/event.schema.json": schema(field), "schemas/row.schema.json": []byte(fmt.Sprintf(`{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","properties":{%q:{"type":"integer"}},"required":[%q],"additionalProperties":false}`, rowField, rowField)), "schemas/result.schema.json": []byte(`{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"integer"}`), "schemas/lifecycle.schema.json": []byte(`{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false}`), "schemas/safe.schema.json": []byte(`{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"boolean"}`), "schemas/intent.schema.json": []byte(`{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","properties":{"value":{"type":"integer"}},"required":["value"],"additionalProperties":false}`), "schemas/named.schema.json": []byte(`{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","properties":{"key":{"type":"string"},"delta":{"type":"integer"}},"required":["key","delta"],"additionalProperties":false}`)}
	deps := []*archive.Package{assets, content, lib, ui}
	nodes := []dependency.LockedPackage{}
	for _, p := range deps {
		d, _ := p.Manifest()
		node, _ := p.ExactLock().Package(d.Package.PackageID)
		nodes = append(nodes, node)
	}
	root, err := build(PackageID, "game-system", version, source, files, []string{"example.test/migration-assets", "example.test/migration-content", "example.test/migration-ui"}, nodes)
	return Graph{Root: root, Dependencies: deps}, err
}

func BuildPair(runtime install.RuntimeConfig, safe, critical bool) (Pair, error) {
	return BuildPairWithOptions(runtime, Options{Safe: safe, Critical: critical})
}

func BuildPairWithOptions(runtime install.RuntimeConfig, options Options) (Pair, error) {
	old, err := graph(false, options.Safe, options.Critical, options.Undeclared, false)
	if err != nil {
		return Pair{}, err
	}
	next, err := graph(true, true, false, false, options.TargetRestoreFailure)
	if err != nil {
		return Pair{}, err
	}
	caps := []string{"host.state", "host.event", "host.db", "host.task"}
	config := install.PolicyConfig{Context: "ci", HostMajor: profile.HostMajor, HostMinor: profile.HostMinor, Artifacts: map[string]install.Approval{}, Host: &install.HostAuthorization{Trust: map[capability.TrustLevel][]string{}, Execution: caps, RunnerHash: runtime.SHA256, Limits: runtime.Limits}}
	for _, level := range []capability.TrustLevel{capability.TrustOfficial, capability.TrustSigned, capability.TrustPrivateUnverified, capability.TrustDevelopment} {
		config.Host.Trust[level] = append([]string(nil), caps...)
	}
	for n, g := range []Graph{old, next} {
		for _, p := range g.Dependencies {
			doc, _ := p.Manifest()
			approval := install.Approval{RightsDigest: install.RightsDigest(*doc.Package), Retention: "synthetic", Safety: "ACTIVE"}
			if doc.Package.Entrypoint != "" {
				approval.Tests = []install.Test{{Name: "library-pure", Source: []byte("return true")}}
			}
			config.Artifacts[string(p.ArtifactIdentity().Digest())] = approval
		}
		p := g.Root
		ref := func(name string, seed checkpoint.Value) install.SchemaReference {
			path := "schemas/" + name + ".schema.json"
			entry, _ := p.Entry(path)
			return install.SchemaReference{PackageID: PackageID, Path: path, Digest: checkpoint.Hash(entry.Bytes()), Seed: seed}
		}
		empty := checkpoint.Object(map[string]checkpoint.Value{})
		state := ref("state", State(n == 1, 1))
		result := ref("result", checkpoint.Int(1))
		ns, field := "docs", "score"
		if n == 1 {
			ns, field = "records", "value"
		}
		row := ref("row", checkpoint.Object(map[string]checkpoint.Value{field: checkpoint.Int(1)}))
		intent := ref("intent", checkpoint.Object(map[string]checkpoint.Value{"value": checkpoint.Int(1)}))
		input := ref("named", checkpoint.Object(map[string]checkpoint.Value{"key": checkpoint.Text("one"), "delta": checkpoint.Int(1)}))
		contract := &install.HostContract{State: state, Result: result, Results: map[string]install.SchemaReference{}, Namespaces: map[string]install.NamespaceContract{PackageID + "/" + ns: {Schema: row, Indices: map[string][]string{field: {field}}, MaxRows: 128, MaxBytes: 256 << 10}}, Events: map[string]install.SchemaReference{PackageID + "/change": ref("event", State(n == 1, 1))}, Intents: map[string]install.SchemaReference{PackageID + "/task": intent, PackageID + "/continuation": intent}, Named: map[string]install.NamedContract{}, Budget: hostapi.DefaultBudget()}
		for name, plan := range map[string]string{"increase": "quantity-add", "read": "quantity-get"} {
			contract.Named[PackageID+"/"+name] = install.NamedContract{PackageID: PackageID, ID: name, Plan: plan, Input: input, Output: result}
		}
		count := int64(1)
		seenCommand := false
		tests := []install.Test{{Name: "pure", Source: []byte("return true")}}
		for _, name := range profile.StandardCallbackNames() {
			if name == "validate_command" || name == "execute_command" {
				if seenCommand {
					continue
				}
				seenCommand = true
				name = "command"
			}
			arg, expected := empty, empty
			switch name {
			case "command":
				count++
				arg = Command(1)
				expected = checkpoint.Int(count)
			case "project_view", "create_checkpoint":
				expected = State(n == 1, count)
				contract.Results[name] = state
			case "on_session_restore", "restore_checkpoint":
				arg = State(n == 1, count)
				contract.Results[name] = ref("lifecycle", empty)
			case "on_safe_migration_boundary":
				value := options.Safe
				if n == 1 {
					value = true
				}
				expected = checkpoint.Bool(value)
				contract.Results[name] = ref("safe", checkpoint.Bool(true))
				if n == 0 && options.Undeclared {
					expected = checkpoint.Int(1)
					delete(contract.Results, name)
				}
			default:
				contract.Results[name] = ref("lifecycle", empty)
			}
			tests = append(tests, install.Test{Name: "case-" + name, Host: &install.HostCase{Callback: name, Input: arg, Expected: expected}})
		}
		doc, _ := p.Manifest()
		config.Artifacts[string(p.ArtifactIdentity().Digest())] = install.Approval{RightsDigest: install.RightsDigest(*doc.Package), Retention: "synthetic", Safety: "ACTIVE", Host: contract, Tests: tests}
	}
	pubSeed, certSeed := sha256.Sum256([]byte("B007 synthetic publisher")), sha256.Sum256([]byte("B007 synthetic certification"))
	pub, cert := ed25519.NewKeyFromSeed(pubSeed[:]), ed25519.NewKeyFromSeed(certSeed[:])
	config.Keys = map[string]install.Key{"fixture-publisher": {Public: pub.Public().(ed25519.PublicKey), Publisher: "example.test", State: "ACTIVE", NotBefore: 1, NotAfter: 4000000000}, "fixture-certification": {Public: cert.Public().(ed25519.PublicKey), Certification: true, State: "ACTIVE", NotBefore: 1, NotAfter: 4000000000}}
	policy, err := install.NewPolicy(config)
	if err != nil {
		return Pair{}, err
	}
	evidence := map[string]install.Evidence{}
	for _, g := range []Graph{old, next} {
		const signedAt = int64(1700000000)
		p, c := &install.Attestation{KeyID: "fixture-publisher", SignedAt: signedAt}, &install.Attestation{KeyID: "fixture-certification", SignedAt: signedAt}
		msg, err := policy.SigningBytes("publisher", g.Root, signedAt)
		if err != nil {
			return Pair{}, err
		}
		p.Signature = ed25519.Sign(pub, msg)
		msg, err = policy.SigningBytes("certification", g.Root, signedAt)
		if err != nil {
			return Pair{}, err
		}
		c.Signature = ed25519.Sign(cert, msg)
		evidence[string(g.Root.ArtifactIdentity().Digest())] = install.Evidence{Publisher: p, Certification: c}
	}
	return Pair{Old: old, New: next, Config: config, Evidence: evidence}, nil
}
