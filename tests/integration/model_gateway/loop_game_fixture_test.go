//go:build linux && integration

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// Package sessionfixture supplies a synthetic minimal M1 loopFixture. Its policy
// grants are explicit operator data; its package identity confers no permission.
package model_gateway_test

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

const loopPackageID = base.PackageID
const loopPrivateValue = "loopFixture-gm-private-value"
const loopIncrementSource = `local M={}
local function state() return {counter=host.state.get({"counter"}),secret=host.state.get({"secret"})} end
for _,n in ipairs({"on_session_create","on_session_restore","on_session_start","list_legal_actions","restore_checkpoint","resume_continuation","on_safe_migration_boundary","cleanup"}) do M[n]=function(...) return {} end end
M.project_view=function(input)
 if input.seat_id then
  local view={counter=host.state.get({"counter"}),pending_action="choose-"..input.seat_id}
  if input.seat_id=="gm" then view.secret=host.state.get({"secret"}) end
  return view
 end
 return state()
end
M.create_checkpoint=function() return state() end
M.on_session_end=function() host.event.emit("change",state());return {} end
M.validate_command=function(command) return type(command)=="table" and (command.type=="increment" or command.type=="fail") and type(command.payload)=="table" and math.type(command.payload.delta)=="integer" and command.payload.delta>=1 and command.payload.delta<=1000 end
M.execute_command=function(command)
 local before=host.state.get({"counter"}); local next=before+command.payload.delta
 host.state.put({"counter"},next)
 host.db.put("docs","one",{score=next})
 host.event.emit("change",state())
 local task=host.ai.request({seat_id="ai",selection="selected",mode="proposal"});host.task.continuation(task,{value=next})
 assert(host.time.now()>0);local draw=host.random.next(10);assert(draw>=0 and draw<10)
 assert(host.content.get("content.txt")=="fixture content")
 assert(host.rules.call("integer.compare",before,next)==-1)
 host.log.write("synthetic metadata only")
 if command.type=="fail" then error("synthetic callback failure after intents") end
 return next
end
M.resume_continuation=function(command)
 if not command.payload then return {} end
 assert(command.seat_id=="task-system" and command.type=="resume-continuation")
 local result=command.payload.result
 assert(type(result)=="table")
 if result.status=="paused" then
  if result.mode=="narrative" then assert(type(result.narrative)=="string" and #result.narrative>0) end
  host.event.emit("change",state());return {}
 end
 assert(result.status=="complete")
 if result.mode=="proposal" then
  local a=result.action
  assert(a.type=="increment" and type(a.payload)=="table" and math.type(a.payload.delta)=="integer" and a.payload.delta==1)
  local next=host.state.get({"counter"})+a.payload.delta
  assert(next<=1000000000)
  host.state.put({"counter"},next);host.event.emit("change",state())
  local narrative=host.ai.request({seat_id="ai",selection="selected",mode="narrative"});host.task.continuation(narrative,{value=next})
 elseif result.mode=="narrative" then
  assert(type(result.narrative)=="string");host.event.emit("change",state())
 else error("invalid synthetic model result") end
 return {}
end
return M
`

func loopState(counter int64) checkpoint.Value {
	return checkpoint.Object(map[string]checkpoint.Value{"counter": checkpoint.Int(counter), "secret": checkpoint.Text(loopPrivateValue)})
}
func loopCommandInput(typ string, delta int64) checkpoint.Value {
	return checkpoint.Object(map[string]checkpoint.Value{"type": checkpoint.Text(typ), "seat_id": checkpoint.Text("gm"), "correlation_id": checkpoint.Text("loopFixture-case"), "payload": checkpoint.Object(map[string]checkpoint.Value{"delta": checkpoint.Int(delta)})})
}
func loopBuild(runtime install.RuntimeConfig, source string) (*archive.Package, install.PolicyConfig, error) {
	if source == "" {
		source = loopIncrementSource
	}
	stateSchema := []byte(`{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","properties":{"counter":{"type":"integer","minimum":0,"maximum":1000000000},"secret":{"type":"string","maxLength":128}},"required":["counter","secret"],"additionalProperties":false}`)
	inputs := []byte(`{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","properties":{"delta":{"type":"integer","minimum":1,"maximum":1000}},"required":["delta"],"additionalProperties":false}`)
	// The default also validates recorded external ToolResults. Human command
	// integer results retain their exact original, separately named Schema.
	pkg, err := base.Package("", source, map[string][]byte{"schemas/state.schema.json": stateSchema, "schemas/event.schema.json": stateSchema, "schemas/command.schema.json": inputs, "schemas/view.schema.json": []byte(`{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","properties":{"counter":{"type":"integer"},"secret":{"type":"string","maxLength":128},"pending_action":{"type":"string","maxLength":128}},"required":["counter"],"additionalProperties":false}`), "schemas/ai-intent.schema.json": []byte(`{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","properties":{"seat_id":{"type":"string"},"selection":{"type":"string"},"mode":{"type":"string","enum":["proposal","narrative"]}},"required":["seat_id","selection","mode"],"additionalProperties":false}`), "schemas/lifecycle.schema.json": []byte(`{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false}`), "schemas/model-result.schema.json": []byte(`{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","properties":{"mode":{"enum":["proposal","narrative"]},"status":{"enum":["complete","paused"]},"action":{"type":"object","properties":{"type":{"const":"increment"},"expected_state_version":{"type":"integer","minimum":1,"maximum":9007199254740991},"payload":{"type":"object","properties":{"delta":{"type":"integer","const":1}},"required":["delta"],"additionalProperties":false}},"required":["type","expected_state_version","payload"],"additionalProperties":false},"narrative":{"type":"string","minLength":1,"maxLength":16384}},"required":["mode","status"],"additionalProperties":false,"oneOf":[{"properties":{"mode":{"const":"proposal"},"status":{"const":"complete"}},"required":["action"],"not":{"required":["narrative"]}},{"properties":{"mode":{"const":"narrative"},"status":{"const":"complete"}},"required":["narrative"],"not":{"required":["action"]}},{"properties":{"status":{"const":"paused"}},"not":{"required":["action"]}}]}`)})
	if err != nil {
		return nil, install.PolicyConfig{}, err
	}
	ref := func(name string, seed checkpoint.Value) install.SchemaReference {
		path := "schemas/" + name + ".schema.json"
		entry, _ := pkg.Entry(path)
		return install.SchemaReference{PackageID: loopPackageID, Path: path, Digest: checkpoint.Hash(entry.Bytes()), Seed: seed}
	}
	state := ref("state", loopState(1))
	empty := checkpoint.Object(map[string]checkpoint.Value{})
	result := ref("model-result", checkpoint.Object(map[string]checkpoint.Value{"mode": checkpoint.Text("proposal"), "status": checkpoint.Text("paused")}))
	row := ref("row", checkpoint.Object(map[string]checkpoint.Value{"score": checkpoint.Int(1)}))
	intent := ref("intent", checkpoint.Object(map[string]checkpoint.Value{"value": checkpoint.Int(1)}))
	object := ref("lifecycle", empty)
	contract := &install.HostContract{State: state, Result: result, Results: map[string]install.SchemaReference{}, Namespaces: map[string]install.NamespaceContract{loopPackageID + "/docs": {Schema: row, Indices: map[string][]string{"score": {"score"}}, MaxRows: 128, MaxBytes: 256 << 10}}, Events: map[string]install.SchemaReference{loopPackageID + "/change": ref("event", loopState(1))}, Intents: map[string]install.SchemaReference{loopPackageID + "/task": intent, loopPackageID + "/continuation": intent, loopPackageID + "/ai": ref("ai-intent", checkpoint.Object(map[string]checkpoint.Value{"seat_id": checkpoint.Text("ai"), "selection": checkpoint.Text("selected"), "mode": checkpoint.Text("proposal")}))}, Named: map[string]install.NamedContract{}, Budget: hostapi.DefaultBudget()}
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
			input = loopCommandInput("increment", 1)
			expected = checkpoint.Int(counter)
			contract.Results[name] = ref("result", checkpoint.Int(1))
		} else if name == "project_view" || name == "create_checkpoint" {
			expected = loopState(counter)
			contract.Results[name] = state
			if name == "project_view" {
				contract.Results[name] = ref("view", loopState(counter))
			}
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
		return nil, install.PolicyConfig{}, fmt.Errorf("loopFixture requires actual loopRunner hash")
	}
	return pkg, policy, nil
}
