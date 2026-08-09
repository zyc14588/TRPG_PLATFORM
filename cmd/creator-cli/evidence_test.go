// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package main

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"io"
	"reflect"
	"testing"
)

const evidenceCandidateSHA = "0123456789abcdef0123456789abcdef01234567"

func TestPackageEvidenceRecordsEveryTestIDOnce(t *testing.T) {
	var calls []string
	dependencies := packageEvidenceDependencies{
		verifyCandidate: func(_ context.Context, candidate string) error {
			if candidate != evidenceCandidateSHA {
				t.Fatalf("candidate = %q", candidate)
			}
			return nil
		},
		runCommand: func(_ context.Context, _ io.Writer, name string, args ...string) int {
			calls = append(calls, name+" "+args[len(args)-1])
			return 0
		},
	}
	var stdout bytes.Buffer
	var stderr bytes.Buffer
	exitCode := runPackageEvidenceWith(context.Background(), []string{"--candidate-sha", evidenceCandidateSHA}, &stdout, &stderr, dependencies)
	if exitCode != 0 {
		t.Fatalf("exit = %d, stderr = %s", exitCode, stderr.String())
	}
	var report packageEvidenceReport
	if err := json.Unmarshal(stdout.Bytes(), &report); err != nil {
		t.Fatalf("evidence is not JSON: %v", err)
	}
	if report.SchemaVersion != packageEvidenceSchemaVersion || report.CandidateSHA != evidenceCandidateSHA || len(report.Tests) != 4 {
		t.Fatalf("report = %#v", report)
	}
	wantIDs := []string{"TEST-PACKAGE-001", "TEST-PACKAGE-002", "TEST-PACKAGE-003", "TEST-PACKAGE-004"}
	gotIDs := make([]string, len(report.Tests))
	for index, result := range report.Tests {
		gotIDs[index] = result.TestID
		if result.Command == "" || result.ExitCode != 0 || result.Result != "PASS" {
			t.Errorf("result = %#v", result)
		}
	}
	if !reflect.DeepEqual(gotIDs, wantIDs) {
		t.Fatalf("test IDs = %#v", gotIDs)
	}
	if len(calls) != 4 {
		t.Fatalf("command calls = %#v", calls)
	}
}

func TestPackageEvidenceRunsOnceAndFailsOverallOnAnyNonzeroExit(t *testing.T) {
	exitCodes := []int{0, 7, 0, 0}
	calls := 0
	dependencies := packageEvidenceDependencies{
		verifyCandidate: func(context.Context, string) error { return nil },
		runCommand: func(context.Context, io.Writer, string, ...string) int {
			exitCode := exitCodes[calls]
			calls++
			return exitCode
		},
	}
	var stdout bytes.Buffer
	var stderr bytes.Buffer
	exitCode := runPackageEvidenceWith(context.Background(), []string{"--candidate-sha", evidenceCandidateSHA}, &stdout, &stderr, dependencies)
	if exitCode == 0 {
		t.Fatal("evidence harness succeeded with a failed test")
	}
	if calls != len(packageEvidenceSpecs) {
		t.Fatalf("commands ran %d times, want %d", calls, len(packageEvidenceSpecs))
	}
	var report packageEvidenceReport
	if err := json.Unmarshal(stdout.Bytes(), &report); err != nil {
		t.Fatal(err)
	}
	if report.Tests[1].ExitCode != 7 || report.Tests[1].Result != "FAIL" {
		t.Fatalf("failed result = %#v", report.Tests[1])
	}
}

func TestPackageEvidenceRejectsUnboundCandidateBeforeRunningTests(t *testing.T) {
	runs := 0
	dependencies := packageEvidenceDependencies{
		verifyCandidate: func(context.Context, string) error { return errors.New("candidate mismatch") },
		runCommand: func(context.Context, io.Writer, string, ...string) int {
			runs++
			return 0
		},
	}
	var stdout bytes.Buffer
	var stderr bytes.Buffer
	if exitCode := runPackageEvidenceWith(context.Background(), []string{"--candidate-sha", evidenceCandidateSHA}, &stdout, &stderr, dependencies); exitCode == 0 {
		t.Fatal("unbound candidate succeeded")
	}
	if runs != 0 || stdout.Len() != 0 {
		t.Fatalf("runs = %d, stdout = %q", runs, stdout.String())
	}
}
