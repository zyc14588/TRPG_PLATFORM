// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package m2_test

import (
	"encoding/json"
	"strings"
	"testing"

	"github.com/zyc14588/TRPG_PLATFORM/tools/m2/exitproof"
)

func goEvents(action string) []byte {
	events := []map[string]any{{"Time": "2026-10-11T00:00:00Z", "Action": "start", "Package": "github.com/zyc14588/TRPG_PLATFORM/tests/m2"}, {"Time": "2026-10-11T00:00:00Z", "Action": "run", "Package": "github.com/zyc14588/TRPG_PLATFORM/tests/m2", "Test": "TestRequired"}, {"Time": "2026-10-11T00:00:01Z", "Action": action, "Package": "github.com/zyc14588/TRPG_PLATFORM/tests/m2", "Test": "TestRequired"}, {"Time": "2026-10-11T00:00:01Z", "Action": "pass", "Package": "github.com/zyc14588/TRPG_PLATFORM/tests/m2"}}
	raw := []byte{}
	for _, v := range events {
		b, _ := json.Marshal(v)
		raw = append(raw, b...)
		raw = append(raw, '\n')
	}
	return raw
}
func TestGoJSONRejectsUnqualifiedExecution(t *testing.T) {
	valid := goEvents("pass")
	if _, e := exitproof.ParseGo(valid, "./tests/m2", []string{"TestRequired"}); e != nil {
		t.Fatal(e)
	}
	terminal := strings.Split(string(valid), "\n")[2] + "\n"
	cases := map[string][]byte{"empty": nil, "malformed": []byte("not JSON\n"), "truncated": valid[:len(valid)-1], "unfinished": []byte(strings.ReplaceAll(string(valid), terminal, "")), "duplicate-terminal": []byte(strings.Replace(string(valid), terminal, terminal+terminal, 1)), "skip": goEvents("skip"), "fail": goEvents("fail"), "cache": []byte(strings.Replace(string(valid), "TestRequired", "TestRequired(cached)", 1)), "missing-name": []byte(strings.ReplaceAll(string(valid), "TestRequired", "TestOther")), "zero-run": []byte(strings.Split(string(valid), "\n")[0] + "\n" + strings.Split(string(valid), "\n")[3] + "\n")}
	// Cached execution is identified by actual Go output, even if package says PASS.
	cached := string(valid)
	line := `{"Time":"2026-10-11T00:00:01Z","Action":"output","Package":"github.com/zyc14588/TRPG_PLATFORM/tests/m2","Output":"ok (cached)\\n"}` + "\n"
	pieces := strings.Split(cached, "\n")
	cases["cache"] = []byte(strings.Join(pieces[:3], "\n") + "\n" + line + pieces[3] + "\n")
	for name, raw := range cases {
		t.Run(name, func(t *testing.T) {
			if _, e := exitproof.ParseGo(raw, "./tests/m2", []string{"TestRequired"}); e == nil {
				t.Fatal("negative Go proof qualified")
			}
		})
	}
}
func complete() exitproof.Report {
	source := exitproof.Source{Commit: strings.Repeat("a", 40), Tree: strings.Repeat("b", 40), Signature: exitproof.Fingerprint, ManifestSHA256: strings.Repeat("c", 64), Clean: true}
	r := exitproof.Report{Schema: exitproof.Schema, Before: source, After: source, Recovered: source, Recovery: true, Cleanup: true, Commands: []exitproof.Command{{Exit: 0, Stdout: exitproof.Capture{SHA256: "stdout"}, Stderr: exitproof.Capture{SHA256: "stderr"}}}}
	specs := exitproof.Stages()
	specs = append(specs, exitproof.StageSpec{Name: "frontend-build"}, exitproof.StageSpec{Name: "compose", Assertions: exitproof.LifecycleAssertions()})
	for _, spec := range specs {
		assertions := map[string]string{}
		for _, name := range spec.Assertions {
			assertions[name] = "PASS"
		}
		r.Stages = append(r.Stages, exitproof.Stage{Name: spec.Name, Package: spec.Package, Passed: true, Cleanup: true, Assertions: assertions, Commands: []int{0}, SourceBefore: source, SourceAfter: source})
	}
	r.Tests = exitproof.TestResults(r.Stages)
	return r
}
func TestQualificationRejectsIncompleteSourceStagesAndIDs(t *testing.T) {
	r := complete()
	if e := exitproof.Qualify(&r); e != nil {
		t.Fatal(e)
	}
	cases := map[string]func(*exitproof.Report){"source-drift": func(r *exitproof.Report) { r.After.Tree = "other" }, "dirty": func(r *exitproof.Report) { r.After.Clean = false }, "unverified-signature": func(r *exitproof.Report) { r.Before.Signature = "other" }, "nonzero-exit": func(r *exitproof.Report) { r.Commands[0].Exit = 1 }, "cleanup": func(r *exitproof.Report) { r.Cleanup = false }, "resource-cleanup": func(r *exitproof.Report) { r.Resources = []exitproof.Resource{{Verified: true, Removed: false}} }, "recovery": func(r *exitproof.Report) { r.Recovery = false }, "unresolved": func(r *exitproof.Report) { r.Findings = []string{"required"} }}
	// Every ID, mandatory stage and named assertion independently fails closed.
	for _, id := range exitproof.IDs() {
		cases["missing-ID-"+id] = func(r *exitproof.Report) { delete(r.Tests, id) }
	}
	specs := exitproof.Stages()
	specs = append(specs, exitproof.StageSpec{Name: "frontend-build"}, exitproof.StageSpec{Name: "compose", Assertions: exitproof.LifecycleAssertions()})
	for i, spec := range specs {
		cases["missing-stage-"+spec.Name] = func(r *exitproof.Report) { r.Stages = append(r.Stages[:i], r.Stages[i+1:]...) }
		for _, name := range spec.Assertions {
			cases["missing-assertion-"+spec.Name+"/"+name] = func(r *exitproof.Report) { delete(r.Stages[i].Assertions, name) }
		}
	}
	cases["missing-compose"] = func(r *exitproof.Report) { r.Stages = r.Stages[:len(r.Stages)-1] }
	cases["duplicate-stage"] = func(r *exitproof.Report) { r.Stages = append(r.Stages, r.Stages[0]) }
	cases["failed-stage"] = func(r *exitproof.Report) { r.Stages[0].Passed = false }
	for name, mutate := range cases {
		t.Run(name, func(t *testing.T) {
			r := complete()
			mutate(&r)
			if e := exitproof.Qualify(&r); e == nil {
				t.Fatal("negative complete proof qualified")
			}
		})
	}
}
