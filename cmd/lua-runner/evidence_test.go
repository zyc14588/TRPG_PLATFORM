// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package main

import (
	"strings"
	"testing"
)

func TestEvidenceRejectsSelectionOverridesACC009(t *testing.T) {
	for _, flags := range []string{"-run=^$", "-skip=.", "-list=.", "-exec=/bin/true", "-overlay=elsewhere.json", "-modfile=other.mod"} {
		if err := validateTestEnvironment(map[string]string{"GOFLAGS": flags, "GOOS": "linux", "GOARCH": "amd64", "GOVERSION": "go1.26.5", "GOWORK": ""}); err == nil {
			t.Fatalf("009 override accepted: %s", flags)
		}
	}
	if err := validateTestEnvironment(map[string]string{"GOFLAGS": "", "GOOS": "linux", "GOARCH": "amd64", "GOVERSION": "go1.26.5", "GOWORK": ""}); err != nil {
		t.Fatal(err)
	}
}
func TestEvidenceRequiresExecutedTestsACC009(t *testing.T) {
	expected := map[string]bool{"p/TestRequired": true}
	valid := `{"Action":"run","Package":"p","Test":"TestRequired"}` + "\n" + `{"Action":"pass","Package":"p","Test":"TestRequired"}` + "\n" + `{"Action":"pass","Package":"p"}` + "\n"
	if _, err := verifyEvents(strings.NewReader(valid), expected); err != nil {
		t.Fatal(err)
	}
	for _, events := range []string{`{"Action":"pass","Package":"p"}`, `{"Action":"pass","Package":"p","Test":"TestRequired"}`, strings.Replace(valid, `"pass","Package":"p","Test"`, `"skip","Package":"p","Test"`, 1), strings.ReplaceAll(valid, "TestRequired", "TestOther"), "not JSON"} {
		if _, err := verifyEvents(strings.NewReader(events), expected); err == nil {
			t.Fatalf("false PASS for %s", events)
		}
	}
}

func TestEvidenceCatalogCoversCandidateTests(t *testing.T) {
	for _, suite := range requiredSuites() {
		tests, err := expectedTests("../..", suite)
		if err != nil || len(tests) == 0 {
			t.Fatalf("%s: %v", suite.ID, err)
		}
	}
}
