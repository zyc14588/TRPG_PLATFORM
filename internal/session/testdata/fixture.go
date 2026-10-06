// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// Package sessionfixture supplies a synthetic minimal M1 fixture. Its policy
// grants are explicit operator data; its package identity confers no permission.
package sessionfixture

import (
	"fmt"

	"github.com/zyc14588/TRPG_PLATFORM/internal/hostapi"
	base "github.com/zyc14588/TRPG_PLATFORM/internal/hostapi/hostapitest"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/archive"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/capability"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/install"
)

const PackageID = base.PackageID
const PrivateValue = "fixture-gm-private-value"
const IncrementSource = `local M={}
local function state() return {counter=host.state.get({"counter"}),secret=host.state.get({"secret"})} end
for _,n in ipairs({"on_session_create","on_session_restore","on_session_start","list_legal_actions","restore_checkpoint","resume_continuation","on_safe_migration_boundary","cleanup"}) do M[n]=function(...) return {} end end
M.project_view=function() return state() end
M.create_checkpoint=function() return state() end
M.on_session_end=function() host.event.emit("change",state());return {} end
M.validate_command=function(command) return type(command)=="table" and (command.type=="increment" or command.type=="fail") and type(command.payload)=="table" and math.type(command.payload.delta)=="integer" and command.payload.delta>=1 and command.payload.delta<=1000 end
M.execute_command=function(command)
 if command.type=="fail" then error("synthetic callback failure") end
 local before=host.state.get({"counter"}); local next=before+command.payload.delta
 host.state.put({"counter"},next)
 host.db.put("docs","one",{score=next})
 host.event.emit("change",state())
 local task=host.task.create({value=next});host.task.continuation(task,{value=next});host.ai.request({value=next})
 assert(host.time.now()==1000);assert(host.random.next(10)==7)
 assert(host.content.get("content.txt")=="fixture content")
 assert(host.rules.call("integer.compare",before,next)==-1)
 host.log.write("synthetic metadata only")
 return next
end
return M
`

func State(counter int64) checkpoint.Value {
	return checkpoint.Object(map[string]checkpoint.Value{"counter": checkpoint.Int(counter), "secret": checkpoint.Text(PrivateValue)})
}
func CommandInput(typ string, delta int64) checkpoint.Value {
	return checkpoint.Object(map[string]checkpoint.Value{"type": checkpoint.Text(typ), "seat_id": checkpoint.Text("gm"), "correlation_id": checkpoint.Text("fixture-case"), "payload": checkpoint.Object(map[string]checkpoint.Value{"delta": checkpoint.Int(delta)})})
}
func Build(runtime install.RuntimeConfig, source string) (*archive.Package, install.PolicyConfig, error) {
	if source == "" {
		source = IncrementSource
	}
	stateSchema := []byte(`{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","properties":{"counter":{"type":"integer","minimum":0,"maximum":1000000000},"secret":{"type":"string","maxLength":128}},"required":["counter","secret"],"additionalProperties":false}`)
	inputs := []byte(`{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","properties":{"delta":{"type":"integer","minimum":1,"maximum":1000}},"required":["delta"],"additionalProperties":false}`)
	pkg, err := base.Package("", source, map[string][]byte{"schemas/state.schema.json": stateSchema, "schemas/event.schema.json": stateSchema, "schemas/command.schema.json": inputs, "schemas/lifecycle.schema.json": []byte(`{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false}`)})
	if err != nil {
		return nil, install.PolicyConfig{}, err
	}
	ref := func(name string, seed checkpoint.Value) install.SchemaReference {
		path := "schemas/" + name + ".schema.json"
		entry, _ := pkg.Entry(path)
		return install.SchemaReference{PackageID: PackageID, Path: path, Digest: checkpoint.Hash(entry.Bytes()), Seed: seed}
	}
	state := ref("state", State(1))
	empty := checkpoint.Object(map[string]checkpoint.Value{})
	result := ref("result", checkpoint.Int(1))
	row := ref("row", checkpoint.Object(map[string]checkpoint.Value{"score": checkpoint.Int(1)}))
	intent := ref("intent", checkpoint.Object(map[string]checkpoint.Value{"value": checkpoint.Int(1)}))
	object := ref("lifecycle", empty)
	contract := &install.HostContract{State: state, Result: result, Results: map[string]install.SchemaReference{}, Namespaces: map[string]install.NamespaceContract{PackageID + "/docs": {Schema: row, Indices: map[string][]string{"score": {"score"}}, MaxRows: 128, MaxBytes: 256 << 10}}, Events: map[string]install.SchemaReference{PackageID + "/change": ref("event", State(1))}, Intents: map[string]install.SchemaReference{PackageID + "/task": intent, PackageID + "/continuation": intent, PackageID + "/ai": intent}, Named: map[string]install.NamedContract{}, Budget: hostapi.DefaultBudget()}
	tests := []install.Test{{Name: "raw-pure", Source: []byte("return true")}}
	counter := int64(1)
	added := false
	for _, name := range profile.StandardCallbackNames() {
		if name == "validate_command" || name == "execute_command" {
			if added {
				continue
			}
			added = true
			name = "command"
		}
		input := empty
		expected := empty
		if name == "command" {
			counter++
			input = CommandInput("increment", 1)
			expected = checkpoint.Int(counter)
		} else if name == "project_view" || name == "create_checkpoint" {
			expected = State(counter)
			contract.Results[name] = state
		} else {
			contract.Results[name] = object
		}
		tests = append(tests, install.Test{Name: "case-" + name, Host: &install.HostCase{Callback: name, Input: input, Expected: expected, Time: 1000, Random: []int64{7}}})
	}
	doc, err := pkg.Manifest()
	if err != nil {
		return nil, install.PolicyConfig{}, err
	}
	policy := install.PolicyConfig{Context: "ci", HostMajor: profile.HostMajor, HostMinor: profile.HostMinor, Artifacts: map[string]install.Approval{string(pkg.ArtifactIdentity().Digest()): {RightsDigest: install.RightsDigest(*doc.Package), Retention: "synthetic", Safety: "ACTIVE", Tests: tests, Host: contract}}, Host: &install.HostAuthorization{Trust: map[capability.TrustLevel][]string{}, Execution: append([]string(nil), base.AllCapabilities...), RunnerHash: runtime.SHA256, Limits: runtime.Limits}}
	for _, level := range []capability.TrustLevel{capability.TrustOfficial, capability.TrustSigned, capability.TrustPrivateUnverified, capability.TrustDevelopment} {
		policy.Host.Trust[level] = append([]string(nil), base.AllCapabilities...)
	}
	if policy.Host.RunnerHash == "" {
		return nil, install.PolicyConfig{}, fmt.Errorf("fixture requires actual runner hash")
	}
	return pkg, policy, nil
}
