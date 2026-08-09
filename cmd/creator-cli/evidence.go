// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package main

import (
	"context"
	"errors"
	"flag"
	"fmt"
	"io"
	"os/exec"
	"regexp"
	"strings"
)

const packageEvidenceSchemaVersion = 1

var candidateSHAPattern = regexp.MustCompile(`^[0-9a-f]{40}$`)

type packageEvidenceSpec struct {
	TestID string
	Name   string
	Args   []string
}

var packageEvidenceSpecs = []packageEvidenceSpec{
	{TestID: "TEST-PACKAGE-001", Name: "go", Args: []string{"test", "-count=1", "./internal/package/..."}},
	{TestID: "TEST-PACKAGE-002", Name: "go", Args: []string{"test", "-count=1", "./internal/package/..."}},
	{TestID: "TEST-PACKAGE-003", Name: "go", Args: []string{"test", "-count=1", "./internal/package/capability/..."}},
	{TestID: "TEST-PACKAGE-004", Name: "go", Args: []string{"test", "-count=1", "./internal/package/dependency/..."}},
}

type packageEvidenceResult struct {
	TestID   string `json:"test_id"`
	Command  string `json:"command"`
	ExitCode int    `json:"exit_code"`
	Result   string `json:"result"`
}

type packageEvidenceReport struct {
	SchemaVersion int                     `json:"schema_version"`
	CandidateSHA  string                  `json:"candidate_sha"`
	Tests         []packageEvidenceResult `json:"tests"`
}

type packageEvidenceDependencies struct {
	verifyCandidate func(context.Context, string) error
	runCommand      func(context.Context, io.Writer, string, ...string) int
}

func runPackageEvidence(ctx context.Context, args []string, stdout, stderr io.Writer) int {
	return runPackageEvidenceWith(ctx, args, stdout, stderr, packageEvidenceDependencies{
		verifyCandidate: verifyEvidenceCandidate,
		runCommand:      runEvidenceCommand,
	})
}

func runPackageEvidenceWith(ctx context.Context, args []string, stdout, stderr io.Writer, dependencies packageEvidenceDependencies) int {
	flags := flag.NewFlagSet("package evidence", flag.ContinueOnError)
	flags.SetOutput(stderr)
	candidateSHA := flags.String("candidate-sha", "", "exact 40-character Git commit SHA under test")
	if err := flags.Parse(args); err != nil {
		return 2
	}
	if flags.NArg() != 0 || !candidateSHAPattern.MatchString(*candidateSHA) {
		fmt.Fprintln(stderr, "package evidence requires --candidate-sha with an exact lower-case 40-character Git SHA")
		return 2
	}
	if dependencies.verifyCandidate == nil || dependencies.runCommand == nil {
		return reportPackageError(stderr, errors.New("package evidence dependencies are not configured"))
	}
	if err := dependencies.verifyCandidate(ctx, *candidateSHA); err != nil {
		return reportPackageError(stderr, err)
	}

	report := packageEvidenceReport{
		SchemaVersion: packageEvidenceSchemaVersion,
		CandidateSHA:  *candidateSHA,
		Tests:         make([]packageEvidenceResult, 0, len(packageEvidenceSpecs)),
	}
	allPassed := true
	for _, spec := range packageEvidenceSpecs {
		exitCode := dependencies.runCommand(ctx, stderr, spec.Name, spec.Args...)
		result := "PASS"
		if exitCode != 0 {
			result = "FAIL"
			allPassed = false
		}
		report.Tests = append(report.Tests, packageEvidenceResult{
			TestID: spec.TestID, Command: strings.Join(append([]string{spec.Name}, spec.Args...), " "),
			ExitCode: exitCode, Result: result,
		})
	}
	if err := writeJSON(stdout, report); err != nil {
		return reportPackageError(stderr, err)
	}
	if !allPassed {
		return 1
	}
	return 0
}

func verifyEvidenceCandidate(ctx context.Context, candidateSHA string) error {
	headCommand := exec.CommandContext(ctx, "git", "rev-parse", "HEAD")
	headOutput, err := headCommand.Output()
	if err != nil {
		return fmt.Errorf("resolve evidence candidate HEAD: %w", err)
	}
	head := strings.TrimSpace(string(headOutput))
	if !candidateSHAPattern.MatchString(head) {
		return fmt.Errorf("resolved HEAD %q is not an exact Git SHA", head)
	}
	if head != candidateSHA {
		return fmt.Errorf("candidate SHA %s does not match HEAD %s", candidateSHA, head)
	}
	statusCommand := exec.CommandContext(ctx, "git", "status", "--porcelain", "--untracked-files=no")
	statusOutput, err := statusCommand.Output()
	if err != nil {
		return fmt.Errorf("check evidence worktree: %w", err)
	}
	if len(statusOutput) != 0 {
		return errors.New("tracked worktree is not clean; evidence would not be bound to the candidate SHA")
	}
	return nil
}

func runEvidenceCommand(ctx context.Context, stderr io.Writer, name string, args ...string) int {
	command := exec.CommandContext(ctx, name, args...)
	command.Stdout = stderr
	command.Stderr = stderr
	if err := command.Run(); err != nil {
		var exitError *exec.ExitError
		if errors.As(err, &exitError) {
			return exitError.ExitCode()
		}
		fmt.Fprintf(stderr, "run evidence command %q: %v\n", strings.Join(append([]string{name}, args...), " "), err)
		return -1
	}
	return 0
}
