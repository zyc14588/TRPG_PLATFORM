// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package exitproof

import (
	"bufio"
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"sort"
	"strings"
	"time"
)

const Schema = "m2-exit-evidence/1"
const Fingerprint = "SHA256:zR55HATrI3+mzVZW3uc8MsCxE+g5fyImXdD8X7WKkt4"
const module = "github.com/zyc14588/TRPG_PLATFORM/"

type File struct {
	Path, SHA256, Mode string
	Bytes              int64
}
type Source struct {
	Commit, Tree, Signature, ManifestSHA256 string
	Clean                                   bool
	Files                                   []File
}
type Capture struct {
	Path, SHA256 string
	Bytes        int64
}
type Command struct {
	Name           string
	Argv           []string
	Exit           int
	StartedUTC     string
	DurationMillis int64
	Stdout, Stderr Capture
	Private        bool
}
type Stage struct {
	Name, Package             string
	Passed, Cleanup           bool
	Required                  []string
	Assertions                map[string]string
	Commands                  []int
	SourceBefore, SourceAfter Source
	Findings                  []string
}
type Resource struct {
	Stage, Batch, Run, ID, Image, Name, Database, User, Port string
	Labels                                                   map[string]string
	Verified, Removed                                        bool
	Volumes                                                  []string
}
type TestResult struct {
	ID, Verdict string
	Assertions  []Assertion
}
type Report struct {
	Schema, Task, Run, Verdict, CreatedUTC string
	Before, After, Recovered               Source
	ToolVersions                           map[string]string
	Commands                               []Command
	Stages                                 []Stage
	Tests                                  map[string]TestResult
	Resources                              []Resource
	Captures                               []Capture
	Cleanup, Recovery                      bool
	Findings, Limitations                  []string
}

type event struct {
	Time                          time.Time
	Action, Package, Test, Output string
	Elapsed                       float64
}

// ParseGo accepts actual one-package -count=1 -json stdout only. Every observed
// test must run once and finish once. A top-level success cannot hide a skip,
// cache, failed subtest, duplicate terminal, unfinished test or missing name.
func ParseGo(raw []byte, pkg string, required []string) (map[string]string, error) {
	if len(raw) == 0 || raw[len(raw)-1] != '\n' {
		return nil, errors.New("empty or truncated Go JSON evidence")
	}
	scan := bufio.NewScanner(bytes.NewReader(raw))
	scan.Buffer(make([]byte, 4096), 2<<20)
	starts, terminals := map[string]bool{}, map[string]string{}
	packageStarted, packageDone := false, false
	lines := 0
	for scan.Scan() {
		lines++
		var v event
		if len(scan.Bytes()) == 0 || json.Unmarshal(scan.Bytes(), &v) != nil || v.Time.IsZero() || v.Package != module+strings.TrimPrefix(pkg, "./") {
			return nil, fmt.Errorf("malformed or foreign Go event at line %d", lines)
		}
		if strings.Contains(v.Output, "(cached)") {
			return nil, errors.New("cached Go execution cannot qualify")
		}
		switch v.Action {
		case "start":
			if v.Test != "" || packageStarted || packageDone {
				return nil, errors.New("duplicate package start")
			}
			packageStarted = true
		case "run":
			if !packageStarted || packageDone || v.Test == "" || starts[v.Test] || terminals[v.Test] != "" {
				return nil, errors.New("invalid or duplicate test start")
			}
			starts[v.Test] = true
		case "pass":
			if !packageStarted || packageDone {
				return nil, errors.New("unexpected terminal")
			}
			if v.Test == "" {
				packageDone = true
				break
			}
			if !starts[v.Test] || terminals[v.Test] != "" {
				return nil, errors.New("missing start or duplicate test terminal")
			}
			terminals[v.Test] = "PASS"
		case "fail", "skip":
			return nil, fmt.Errorf("Go mandatory execution %s", v.Action)
		case "pause", "cont":
			if !starts[v.Test] || terminals[v.Test] != "" || packageDone {
				return nil, errors.New("invalid parallel event")
			}
		case "output":
			if !packageStarted || packageDone {
				return nil, errors.New("output outside package execution")
			}
		default:
			return nil, fmt.Errorf("unknown Go event action %q", v.Action)
		}
	}
	if scan.Err() != nil {
		return nil, scan.Err()
	}
	if !packageStarted || !packageDone || len(starts) == 0 {
		return nil, errors.New("missing package or named test terminal")
	}
	for name := range starts {
		if terminals[name] != "PASS" {
			return nil, errors.New("unfinished named test")
		}
	}
	for _, name := range required {
		if terminals[name] != "PASS" {
			return nil, fmt.Errorf("missing required named assertion %s", name)
		}
	}
	return terminals, nil
}

func SameSource(a, b Source) bool {
	return a.Clean && b.Clean && a.Commit != "" && a.Tree != "" && a.Signature == Fingerprint && b.Signature == Fingerprint && a.Commit == b.Commit && a.Tree == b.Tree && a.ManifestSHA256 != "" && a.ManifestSHA256 == b.ManifestSHA256
}

// Qualify is intentionally all-or-nothing. Only the runner constructs Reports;
// the CLI has no report-import, stage selection, external PASS or skip option.
func Qualify(r *Report) error {
	if r.Schema != Schema || !SameSource(r.Before, r.After) || !r.Recovery || !SameSource(r.Before, r.Recovered) {
		return errors.New("source drift or unverified recovery")
	}
	if len(r.Findings) != 0 || !r.Cleanup {
		return errors.New("unresolved findings or incomplete cleanup")
	}
	seen := map[string]Stage{}
	for _, s := range r.Stages {
		if _, ok := seen[s.Name]; ok {
			return errors.New("duplicate execution stage")
		}
		if !s.Passed || !s.Cleanup || len(s.Findings) != 0 || !SameSource(r.Before, s.SourceBefore) || !SameSource(r.Before, s.SourceAfter) || len(s.Commands) == 0 {
			return fmt.Errorf("unqualified stage %s", s.Name)
		}
		for _, i := range s.Commands {
			if i < 0 || i >= len(r.Commands) || r.Commands[i].Exit != 0 || r.Commands[i].Stdout.SHA256 == "" || r.Commands[i].Stderr.SHA256 == "" {
				return fmt.Errorf("nonzero or missing command in %s", s.Name)
			}
		}
		seen[s.Name] = s
	}
	specs := Stages()
	specs = append(specs, StageSpec{Name: "frontend-build"}, StageSpec{Name: "compose", Assertions: LifecycleAssertions()})
	if len(seen) != len(specs) {
		return errors.New("missing or extra mandatory stage")
	}
	for _, spec := range specs {
		s, ok := seen[spec.Name]
		if !ok || s.Package != spec.Package {
			return fmt.Errorf("missing mandatory stage %s", spec.Name)
		}
		for _, name := range spec.Assertions {
			if s.Assertions[name] != "PASS" {
				return fmt.Errorf("missing required assertion %s/%s", spec.Name, name)
			}
		}
	}
	for _, resource := range r.Resources {
		if !resource.Verified || !resource.Removed {
			return errors.New("unverified or incompletely removed owned fixture")
		}
	}
	if len(r.Tests) != 13 || len(r.Tests) != len(requiredTests) {
		return errors.New("missing or extra original TEST ID")
	}
	for id, refs := range requiredTests {
		test, ok := r.Tests[id]
		if !ok || test.ID != id || test.Verdict != "PASS" || len(test.Assertions) != len(refs) {
			return fmt.Errorf("missing or failed %s", id)
		}
		for i, ref := range refs {
			if test.Assertions[i] != ref || seen[ref.Stage].Assertions[ref.Name] != "PASS" {
				return fmt.Errorf("unproven %s: %s/%s", id, ref.Stage, ref.Name)
			}
		}
	}
	return nil
}

func TestResults(stages []Stage) map[string]TestResult {
	observed := map[string]Stage{}
	for _, s := range stages {
		observed[s.Name] = s
	}
	results := map[string]TestResult{}
	for id, refs := range requiredTests {
		result := TestResult{ID: id, Verdict: "PASS", Assertions: append([]Assertion(nil), refs...)}
		for _, ref := range refs {
			s := observed[ref.Stage]
			if !s.Passed || s.Assertions[ref.Name] != "PASS" {
				result.Verdict = "FAIL"
			}
		}
		results[id] = result
	}
	return results
}
func IDs() []string {
	ids := []string{}
	for id := range requiredTests {
		ids = append(ids, id)
	}
	sort.Strings(ids)
	return ids
}
