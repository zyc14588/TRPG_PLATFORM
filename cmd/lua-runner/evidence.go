// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package main

import (
	"bytes"
	"context"
	"encoding/hex"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"io"
	"os"
	"os/exec"
	"strings"
)

type evidenceTest struct {
	TestID   string `json:"test_id"`
	Command  string `json:"command"`
	ExitCode int    `json:"exit_code"`
	Result   string `json:"result"`
}

type evidenceReport struct {
	CandidateSHA string         `json:"candidate_sha"`
	Result       string         `json:"result"`
	Tests        []evidenceTest `json:"tests"`
}

type evidenceSpec struct {
	id      string
	command string
	args    []string
}

var evidenceSpecs = []evidenceSpec{
	{
		id:      "TEST-LUA-001",
		command: "go test -count=1 ./internal/luaruntime/profile/... ./cmd/lua-runner/...",
		args:    []string{"go", "test", "-count=1", "./internal/luaruntime/profile/...", "./cmd/lua-runner/..."},
	},
	{
		id:      "TEST-LUA-002",
		command: "go test -count=1 ./internal/luaruntime/vm/... ./internal/luaruntime/checkpoint/...",
		args:    []string{"go", "test", "-count=1", "./internal/luaruntime/vm/...", "./internal/luaruntime/checkpoint/..."},
	},
}

func runEvidence(ctx context.Context, args []string, stdout, stderr io.Writer) int {
	flags := flag.NewFlagSet("evidence", flag.ContinueOnError)
	flags.SetOutput(stderr)
	candidate := flags.String("candidate-sha", "", "exact 40-character code candidate SHA")
	if err := flags.Parse(args); err != nil || flags.NArg() != 0 {
		return 2
	}
	if !validSHA(*candidate) {
		fmt.Fprintln(stderr, "evidence requires --candidate-sha with exactly 40 lowercase hexadecimal characters")
		return 2
	}
	root, err := gitOutput(ctx, "rev-parse", "--show-toplevel")
	if err != nil {
		fmt.Fprintf(stderr, "resolve repository root: %v\n", err)
		return 1
	}
	if err := verifyCandidate(ctx, root, *candidate); err != nil {
		fmt.Fprintf(stderr, "candidate preflight: %v\n", err)
		return 1
	}

	report := evidenceReport{CandidateSHA: *candidate, Result: "PASS", Tests: make([]evidenceTest, 0, len(evidenceSpecs))}
	for _, spec := range evidenceSpecs {
		result, childOutput := executeEvidenceTest(ctx, root, spec)
		report.Tests = append(report.Tests, result)
		if result.Result != "PASS" {
			report.Result = "FAIL"
			fmt.Fprintf(stderr, "%s output:\n%s", spec.id, childOutput)
			if len(childOutput) > 0 && childOutput[len(childOutput)-1] != '\n' {
				fmt.Fprintln(stderr)
			}
		}
	}
	if err := verifyCandidate(ctx, root, *candidate); err != nil {
		report.Result = "FAIL"
		fmt.Fprintf(stderr, "candidate postflight: %v\n", err)
	}
	encoder := json.NewEncoder(stdout)
	encoder.SetEscapeHTML(false)
	if err := encoder.Encode(report); err != nil {
		fmt.Fprintf(stderr, "encode evidence: %v\n", err)
		return 1
	}
	if report.Result != "PASS" {
		return 1
	}
	return 0
}

func validSHA(value string) bool {
	if len(value) != 40 || value != strings.ToLower(value) {
		return false
	}
	_, err := hex.DecodeString(value)
	return err == nil
}

func gitOutput(ctx context.Context, args ...string) (string, error) {
	command := exec.CommandContext(ctx, "git", args...)
	output, err := command.Output()
	if err != nil {
		return "", err
	}
	return strings.TrimSpace(string(output)), nil
}

func verifyCandidate(ctx context.Context, root, candidate string) error {
	head := exec.CommandContext(ctx, "git", "rev-parse", "HEAD")
	head.Dir = root
	output, err := head.Output()
	if err != nil {
		return err
	}
	if got := strings.TrimSpace(string(output)); got != candidate {
		return fmt.Errorf("HEAD is %s, require %s", got, candidate)
	}
	status := exec.CommandContext(ctx, "git", "status", "--porcelain=v1", "--untracked-files=no")
	status.Dir = root
	dirty, err := status.Output()
	if err != nil {
		return err
	}
	if len(bytes.TrimSpace(dirty)) != 0 {
		return errors.New("tracked worktree is not clean")
	}
	return nil
}

func executeEvidenceTest(ctx context.Context, root string, spec evidenceSpec) (evidenceTest, []byte) {
	command := exec.CommandContext(ctx, spec.args[0], spec.args[1:]...)
	command.Dir = root
	command.Env = os.Environ()
	var output bytes.Buffer
	command.Stdout = &output
	command.Stderr = &output
	err := command.Run()
	exitCode := 0
	result := "PASS"
	if err != nil {
		result = "FAIL"
		exitCode = 127
		var exitErr *exec.ExitError
		if errors.As(err, &exitErr) {
			exitCode = exitErr.ExitCode()
		}
	}
	return evidenceTest{TestID: spec.id, Command: spec.command, ExitCode: exitCode, Result: result}, output.Bytes()
}
