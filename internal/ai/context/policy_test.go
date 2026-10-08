// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package context

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"testing"

	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/model"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/capability"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/command"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
)

func policyFixture(t *testing.T, execution []string) (Policy, SubjectData) {
	t.Helper()
	decl, e := capability.NewDeclaration(nil, []capability.OptionalSpec{{Name: "host.rules", Fallback: "pause"}, {Name: "host.state", Fallback: "pause"}})
	if e != nil {
		t.Fatal("declaration setup failed")
	}
	trust, e := capability.NewTrustPolicy(map[capability.TrustLevel][]string{capability.TrustOfficial: {"host.rules", "host.state"}, capability.TrustSigned: {}, capability.TrustPrivateUnverified: {}, capability.TrustDevelopment: {}})
	if e != nil {
		t.Fatal("trust setup failed")
	}
	grants, e := capability.NewGrantSet(execution)
	if e != nil {
		t.Fatal("execution setup failed")
	}
	v := SubjectData{Binding: data.Binding{Workspace: "w", Session: "s", GraphHash: checkpoint.Hash([]byte("graph"))}, SeatID: "ai", Tuple: model.Tuple{ToolMode: "structured"}}
	p := Policy{Binding: v.Binding, SeatID: v.SeatID, Tuple: v.Tuple, Role: "player", GeneratorVersion: "memory-v1", Views: command.ViewPolicy{ViewFields: []string{"public"}, EventFields: map[string][]string{"visible": {"public"}}}, Declaration: decl, TrustLevel: capability.TrustOfficial, Trust: trust, Execution: grants, Tools: []Tool{{ID: "rules", Capability: capability.HostRules, Mode: "read"}, {ID: "proposal", Capability: capability.HostState, Mode: "propose"}}}
	return p, v
}
func TestToolsRequireAllThreeGrantLayersAndDefaultToZero(t *testing.T) {
	p, v := policyFixture(t, nil)
	got, _, e := ownPolicy(p, v)
	if e != nil || len(got.Tools) != 0 {
		t.Fatal("default grants must be zero")
	}
	p, v = policyFixture(t, []string{"host.rules", "host.state"})
	got, _, e = ownPolicy(p, v)
	if e != nil || len(got.Tools) != 2 {
		t.Fatal("explicit intersection failed")
	}
	p.Views.ViewFields[0] = "hidden"
	p.Views.EventFields["visible"][0] = "hidden"
	p.Tools[0].ID = "changed"
	if got.Views.ViewFields[0] != "public" || got.Views.EventFields["visible"][0] != "public" || got.Tools[0].ID != "rules" {
		t.Fatal("policy must own filter and tool copies")
	}
	p.TrustLevel = capability.TrustSigned
	got, _, e = ownPolicy(p, v)
	if e != nil || len(got.Tools) != 0 {
		t.Fatal("trust grant must constrain execution grants")
	}
}
func TestForgedScopeTupleRoleAndToolModesAreRejected(t *testing.T) {
	for _, mutate := range []func(*Policy){func(p *Policy) { p.Binding.Session = "other" }, func(p *Policy) { p.SeatID = "other" }, func(p *Policy) { p.Tuple.TestVersion = "changed" }, func(p *Policy) { p.Role = "administrator" }, func(p *Policy) { p.Tools[0].Mode = "write" }, func(p *Policy) { p.Tools[0].Capability = "host.unregistered" }} {
		p, v := policyFixture(t, []string{"host.rules"})
		mutate(&p)
		if _, _, e := ownPolicy(p, v); e != auth.ErrDenied {
			t.Fatal("forged context policy must be denied")
		}
	}
}
func TestPromptOwnsBytesRedactsDiagnosticsAndAdviceHasNoProposalTools(t *testing.T) {
	marker := "synthetic-private-seat-memory"
	v := PayloadData{Role: "host", Memories: []MemoryData{{Text: marker}}, View: checkpoint.Value{Kind: "table", Table: map[string]checkpoint.Value{"public": {Kind: "string", String: "shown"}}}, Tools: []Tool{{ID: "read", Mode: "read"}, {ID: "propose", Mode: "propose"}}}
	p, e := ownPrompt(SubjectData{}, v)
	if e != nil {
		t.Fatal("prompt setup failed")
	}
	v.Memories[0].Text = "mutated"
	v.View.Table["public"] = checkpoint.Value{Kind: "nil"}
	v.Tools[0].ID = "changed"
	var borrowed []byte
	if e = p.Use(func(b []byte) error {
		if !bytes.Contains(b, []byte(marker)) || bytes.Contains(b, []byte("mutated")) {
			return auth.ErrDenied
		}
		borrowed = b
		clear(b)
		return nil
	}); e != nil {
		t.Fatal("prompt does not own its data")
	}
	if !bytes.Equal(borrowed, make([]byte, len(borrowed))) {
		t.Fatal("provider copy was not cleared")
	}
	for _, format := range []string{"%v", "%+v", "%#v", "%s", "%q", "%f", "%x", "%d", "%[0]v", "%*v"} {
		for _, value := range []any{p, &p} {
			if bytes.Contains([]byte(fmt.Sprintf(format, value)), []byte(marker)) {
				t.Fatal("diagnostics exposed private context")
			}
		}
	}
	if _, e = json.Marshal(p); e == nil {
		t.Fatal("ordinary context JSON must be denied")
	}
	if p.Use(func([]byte) error { return errors.New(marker) }) != auth.ErrUnavailable {
		t.Fatal("provider errors must be sanitized")
	}
	a, e := p.Advice()
	if e != nil {
		t.Fatal("advice context failed")
	}
	if e = a.Use(func(b []byte) error {
		var x PayloadData
		if json.Unmarshal(b, &x) != nil || x.Role != "advice" || len(x.Tools) != 1 || x.Tools[0].Mode != "read" {
			return auth.ErrDenied
		}
		return nil
	}); e != nil {
		t.Fatal("advisor gained proposal authority")
	}
}
func TestMemoryCannotClaimAuthorityOrLoseSourceProvenance(t *testing.T) {
	m := MemoryData{ID: "note", Text: "synthetic summary", FactLevel: "summary", GeneratorVersion: "memory-v1", Sources: []uint64{1}}
	if !validMemory(m, "memory-v1") {
		t.Fatal("valid sourced summary rejected")
	}
	for _, mutate := range []func(*MemoryData){func(m *MemoryData) { m.FactLevel = "world-fact" }, func(m *MemoryData) { m.Sources = nil }, func(m *MemoryData) { m.Sources = []uint64{1, 1} }, func(m *MemoryData) { m.GeneratorVersion = "other" }, func(m *MemoryData) { m.Text = string(make([]byte, MaxMemoryBytes+1)) }} {
		x := m
		mutate(&x)
		if validMemory(x, "memory-v1") {
			t.Fatal("invalid memory provenance accepted")
		}
	}
}
