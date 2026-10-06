// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// Package hostapitest supplies synthetic non-product packages to Host contract
// tests. It builds the actual production runner and uses verified archive APIs.
package hostapitest

import (
	"context"
	"fmt"
	"os/exec"
	"path/filepath"
	"strings"

	host "github.com/zyc14588/TRPG_PLATFORM/internal/hostapi"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/vm"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/archive"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/capability"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/dependency"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/extension"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/manifest"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
)

const PackageID = "example.test/host"
const Manifest = `schema_version = 1
artifact_type = "package"
package_id = "example.test/host"
package_kind = "game-system"
version = "1.0.0"
display_name = "Synthetic Host contract fixture"
entrypoint = "lua/main.lua"
lua_profile = "platform-lua-5.5-p1"
[host_api]
major = 1
min_minor = 0
max_minor = 0
[build]
source = "https://example.invalid/host-contract"
revision = "0123456789abcdef"
builder = "host-contract-test/1"
[rights]
authors = ["Host conformance fixture"]
source = "original"
license_expression = "PolyForm-Noncommercial-1.0.0"
[capabilities]
required = ["host.ai","host.content","host.db","host.event","host.log","host.random","host.rules","host.state","host.task","host.time"]
`

var AllCapabilities = []string{"host.ai", "host.content", "host.db", "host.event", "host.log", "host.random", "host.rules", "host.state", "host.task", "host.time"}

const Callbacks = `local M={}
for _,n in ipairs({"on_session_create","on_session_restore","on_session_start","list_legal_actions","project_view","create_checkpoint","restore_checkpoint","resume_continuation","on_safe_migration_boundary","on_session_end","cleanup"}) do M[n]=function(...) return {} end end
M.validate_command=function(command) return not command.reject end
`
const SuccessBody = `
 local before=host.state.get({"counter"})
 host.state.put({"counter"},before+1)
 host.db.put("docs","one",{score=before+1})
 assert(host.db.get("docs","one").score==before+1)
 assert(host.db.compare_and_set("docs","one",{score=before+1},{score=before+2}))
 assert(#host.db.list("docs","score",before+2,1)==1)
 host.db.put("docs","temporary",{score=1});host.db.delete("docs","temporary")
 host.event.emit("change",{counter=before+1})
 local task=host.task.create({value=before})
 host.task.continuation(task,{value=before+1})
 host.ai.request({value=before})
 assert(host.time.now()==1000)
 assert(host.random.next(10)==7)
 assert(host.content.get("content.txt")=="fixture content")
 assert(host.rules.call("integer.compare",before,before+1)==-1)
 host.log.write(command.secret or "redacted")
`

func Source(tail string) string {
	return Callbacks + "M.execute_command=function(command)\n" + SuccessBody + tail + "\n return host.state.get({\"counter\"})\nend\nreturn M"
}
func BuildRunner(root, directory string) (string, error) {
	p := filepath.Join(directory, "lua-runner")
	cmd := exec.Command("go", "build", "-trimpath", "-o", p, "./cmd/lua-runner")
	cmd.Dir = root
	if b, e := cmd.CombinedOutput(); e != nil {
		return "", fmt.Errorf("runner build: %s: %w", b, e)
	}
	return p, nil
}
func Package(text, source string, extra map[string][]byte) (*archive.Package, error) {
	if text == "" {
		text = Manifest
	}
	files := map[string][]byte{archive.ManifestPath: []byte(text), "lua/main.lua": []byte(source), "content.txt": []byte("fixture content")}
	schemas := map[string]string{
		"state":       `{"type":"object","properties":{"counter":{"type":"integer","minimum":0}},"required":["counter"],"additionalProperties":false}`,
		"row":         `{"type":"object","properties":{"score":{"type":"integer"}},"required":["score"],"additionalProperties":false}`,
		"event":       `{"type":"object","properties":{"counter":{"type":"integer"}},"required":["counter"],"additionalProperties":false}`,
		"intent":      `{"type":"object","properties":{"value":{"type":"integer"}},"required":["value"],"additionalProperties":false}`,
		"result":      `{"type":"integer"}`,
		"named_input": `{"type":"object","properties":{"key":{"type":"string"},"delta":{"type":"integer"}},"required":["key","delta"],"additionalProperties":false}`,
	}
	for n, s := range schemas {
		files["schemas/"+n+".schema.json"] = []byte(`{"$schema":"https://json-schema.org/draft/2020-12/schema",` + strings.TrimPrefix(s, "{"))
	}
	for k, v := range extra {
		files[k] = v
	}
	doc, e := manifest.Parse([]byte(text))
	if e != nil {
		return nil, e
	}
	id := string(doc.Package.PackageID)
	node, e := dependency.NewLockedPackage(id, "1.0.0", "sha256:"+strings.Repeat("0", 64), nil, nil)
	if e != nil {
		return nil, e
	}
	lock, e := dependency.BuildExactLock(id, []dependency.LockedPackage{node})
	if e != nil {
		return nil, e
	}
	return archive.FromFiles(files, lock, extension.DefaultSupport)
}
func State() checkpoint.Value {
	return checkpoint.Object(map[string]checkpoint.Value{"counter": checkpoint.Int(1)})
}
func Runtime(ctx context.Context, runner, sid string, pkg *archive.Package, level capability.TrustLevel) (*vm.Session, error) {
	policy, e := capability.NewTrustPolicy(map[capability.TrustLevel][]string{capability.TrustOfficial: AllCapabilities, capability.TrustSigned: AllCapabilities, capability.TrustPrivateUnverified: AllCapabilities, capability.TrustDevelopment: AllCapabilities})
	if e != nil {
		return nil, e
	}
	grant, e := capability.NewGrantSet(AllCapabilities)
	if e != nil {
		return nil, e
	}
	return vm.New(ctx, vm.Options{SessionID: sid, Runner: runner, Package: pkg, State: vm.State{Version: 1, Value: State()}, Limits: profile.DefaultLimits(), Audit: func(a profile.Audit) error { return nil }, Host: &vm.HostOptions{Trust: map[string]capability.TrustLevel{PackageID: level}, Policy: policy, Execution: grant}})
}
func Options(session *vm.Session, pkg *archive.Package, repo data.Repository, workspace string) (host.Options, error) {
	bind := func(name string, seed checkpoint.Value) (host.Schema, error) {
		p := "schemas/" + name + ".schema.json"
		file, ok := pkg.Entry(p)
		if !ok {
			return host.Schema{}, fmt.Errorf("missing fixture schema")
		}
		return host.BindSchema(pkg, p, checkpoint.Hash(file.Bytes()), seed)
	}
	state, e := bind("state", State())
	if e != nil {
		return host.Options{}, e
	}
	row, e := bind("row", checkpoint.Object(map[string]checkpoint.Value{"score": checkpoint.Int(1)}))
	if e != nil {
		return host.Options{}, e
	}
	event, e := bind("event", checkpoint.Object(map[string]checkpoint.Value{"counter": checkpoint.Int(1)}))
	if e != nil {
		return host.Options{}, e
	}
	intent, e := bind("intent", checkpoint.Object(map[string]checkpoint.Value{"value": checkpoint.Int(1)}))
	if e != nil {
		return host.Options{}, e
	}
	result, e := bind("result", checkpoint.Int(1))
	if e != nil {
		return host.Options{}, e
	}
	input, e := bind("named_input", checkpoint.Object(map[string]checkpoint.Value{"key": checkpoint.Text("one"), "delta": checkpoint.Int(1)}))
	if e != nil {
		return host.Options{}, e
	}
	return host.Options{Session: session, Binding: data.Binding{Workspace: workspace, Session: session.SessionID(), GraphHash: session.GraphHash()}, Packages: map[string]*archive.Package{PackageID: pkg}, Repository: repo, StateSchema: state, ResultSchema: result, Namespaces: map[string]host.Namespace{PackageID + "/docs": {Schema: row, Indices: map[string][]string{"score": {"score"}}, MaxRows: 128, MaxBytes: 256 << 10}}, EventSchemas: map[string]host.Schema{PackageID + "/change": event}, IntentSchemas: map[string]host.Schema{PackageID + "/task": intent, PackageID + "/ai": intent, PackageID + "/continuation": intent}, NamedOperations: map[string]host.NamedOperation{PackageID + "/increase": {PackageID: PackageID, ID: "increase", Plan: "quantity-add", Input: input, Output: result}, PackageID + "/read": {PackageID: PackageID, ID: "read", Plan: "quantity-get", Input: input, Output: result}}, Budget: host.DefaultBudget(), Audit: func(data.Audit) error { return nil }, Validate: func(context.Context, data.Commit) error { return nil }}, nil
}
func Command(id string) host.Command {
	return host.Command{ID: id, Principal: "fixture-principal", ExpectedVersion: 1, Input: checkpoint.Object(map[string]checkpoint.Value{"secret": checkpoint.Text("sensitive-fixture-value")}), Time: 1000, Random: []int64{7}}
}
