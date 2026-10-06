// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package main

import (
	"bufio"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"go/ast"
	"go/parser"
	"go/token"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"sort"
	"strings"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
)

const modulePath = "github.com/zyc14588/TRPG_PLATFORM"

type suite struct {
	ID          string
	Directories []string
	Minimum     map[string][]string
}

func requiredSuites() []suite {
	return []suite{
		{ID: "TEST-LUA-001", Directories: []string{"internal/luaruntime/profile", "cmd/lua-runner"}, Minimum: map[string][]string{
			"internal/luaruntime/profile": {"TestLua55Conformance", "TestSourceOnlyAndProductionDenial", "TestPostExecutionConversionPoisonsACC008", "TestExecutionBudgetsAndCancellation", "TestCaughtOutputBudgetIsSticky", "TestCoroutineBudgetAndDeterministicModules", "TestOpaqueIdentitiesCannotEnterDeterministicResults"},
			"cmd/lua-runner":              {"TestEvidenceRejectsSelectionOverridesACC009", "TestEvidenceRequiresExecutedTestsACC009", "TestEvidenceCatalogCoversCandidateTests", "TestRunnerProductionBoundaryAndBudgets", "TestRunnerRejectsMalformedIPC"},
		}},
		{ID: "TEST-LUA-002", Directories: []string{"internal/luaruntime/vm", "internal/luaruntime/checkpoint"}, Minimum: map[string][]string{
			"internal/luaruntime/vm":         {"TestSessionIsolationLifecycleAndHandles", "TestACC008IPCContaminationAndReconstruction", "TestCheckpointReconstructionAndMemoryPressure", "TestRunnerFaultMemoryAndCancellationIsolation", "TestB011PackageInputsAndRejections", "TestMultiSessionRaceIsolation", "TestMandatoryAuditAndFailureRecovery", "TestCapabilitiesRequireTestedFallbackOnExactPackage", "TestExactDependencyGraphAndModuleIsolation"},
			"internal/luaruntime/checkpoint": {"TestCheckpointRoundTripAndDeterminism", "TestCheckpointRejectsInvalidValuesAndBindings", "TestCheckpointSizeDepthAndTampering", "TestCheckpointUnicodeAndMaximumValidDepth"},
		}},
	}
}

type event struct {
	Action  string
	Package string
	Test    string
}
type execution struct {
	Test   string `json:"test"`
	Runs   int    `json:"runs"`
	Passes int    `json:"passes"`
}
type suiteResult struct {
	TestID        string      `json:"test_id"`
	Status        string      `json:"status"`
	Command       []string    `json:"command"`
	ExitCode      int         `json:"exit_code"`
	StartedUTC    string      `json:"started_utc"`
	EndedUTC      string      `json:"ended_utc"`
	ExpectedTests []string    `json:"expected_tests"`
	Execution     []execution `json:"execution"`
	Stdout        string      `json:"stdout"`
	StdoutHash    string      `json:"stdout_sha256"`
	Stderr        string      `json:"stderr"`
	StderrHash    string      `json:"stderr_sha256"`
	Error         string      `json:"error,omitempty"`
}
type evidenceReport struct {
	SchemaVersion    int               `json:"schema_version"`
	CandidateSHA     string            `json:"candidate_sha"`
	CandidateTree    string            `json:"candidate_tree"`
	Status           string            `json:"status"`
	WorkingDirectory string            `json:"working_directory"`
	Environment      map[string]string `json:"environment"`
	Rows             []suiteResult     `json:"tests"`
	Error            string            `json:"error,omitempty"`
}

func validateTestEnvironment(env map[string]string) error {
	if env["GOFLAGS"] != "" {
		return errors.New("test overrides are forbidden: GOFLAGS must be empty (ACC-M1-B002-009)")
	}
	if env["GOWORK"] != "" && env["GOWORK"] != "off" {
		return errors.New("external Go workspace is forbidden")
	}
	if env["GOOS"] != "linux" || env["GOARCH"] != "amd64" || env["GOVERSION"] != "go1.26.5" {
		return errors.New("required native Linux amd64 Go 1.26.5 environment is absent")
	}
	return nil
}

func expectedTests(root string, s suite) (map[string]bool, error) {
	result := map[string]bool{}
	for _, dir := range s.Directories {
		files, err := filepath.Glob(filepath.Join(root, dir, "*_test.go"))
		if err != nil {
			return nil, err
		}
		for _, name := range files {
			file, err := parser.ParseFile(token.NewFileSet(), name, nil, 0)
			if err != nil {
				return nil, err
			}
			for _, decl := range file.Decls {
				f, ok := decl.(*ast.FuncDecl)
				if !ok || f.Recv != nil || f.Name.Name == "TestMain" || !strings.HasPrefix(f.Name.Name, "Test") {
					continue
				}
				result[modulePath+"/"+dir+"/"+f.Name.Name] = true
			}
		}
		for _, name := range s.Minimum[dir] {
			if !result[modulePath+"/"+dir+"/"+name] {
				return nil, fmt.Errorf("required candidate test missing: %s/%s", dir, name)
			}
		}
	}
	if len(result) == 0 {
		return nil, errors.New("zero required tests")
	}
	return result, nil
}

func verifyEvents(reader io.Reader, expected map[string]bool) ([]execution, error) {
	if len(expected) == 0 {
		return nil, errors.New("zero expected tests")
	}
	runs, passes, packages := map[string]int{}, map[string]int{}, map[string]int{}
	scanner := bufio.NewScanner(reader)
	scanner.Buffer(make([]byte, 4096), 1<<20)
	for scanner.Scan() {
		var e event
		if err := json.Unmarshal(scanner.Bytes(), &e); err != nil {
			return nil, errors.New("invalid structured go test event")
		}
		if e.Action == "fail" || e.Action == "skip" {
			return nil, fmt.Errorf("test execution %s: %s/%s", e.Action, e.Package, e.Test)
		}
		if e.Test == "" {
			if e.Action == "pass" {
				packages[e.Package]++
			}
			continue
		}
		key := e.Package + "/" + e.Test
		if e.Action == "run" {
			runs[key]++
		}
		if e.Action == "pass" {
			passes[key]++
		}
	}
	if err := scanner.Err(); err != nil {
		return nil, err
	}
	keys := make([]string, 0, len(expected))
	for k := range expected {
		keys = append(keys, k)
	}
	sort.Strings(keys)
	records := make([]execution, 0, len(keys))
	for _, key := range keys {
		pkg := key[:strings.LastIndex(key, "/")]
		record := execution{Test: key, Runs: runs[key], Passes: passes[key]}
		records = append(records, record)
		if record.Runs != 1 || record.Passes != 1 || packages[pkg] != 1 {
			return records, fmt.Errorf("required test did not run and pass exactly once: %s", key)
		}
	}
	return records, nil
}

func gitText(root string, args ...string) (string, error) {
	cmd := exec.Command("git", append([]string{"--no-optional-locks", "-C", root}, args...)...)
	out, err := cmd.Output()
	return strings.TrimSpace(string(out)), err
}
func candidateIdentity(root, candidate string) (string, error) {
	if len(candidate) != 40 || strings.Trim(candidate, "0123456789abcdef") != "" {
		return "", errors.New("exact candidate SHA required")
	}
	actual, err := gitText(root, "rev-parse", "HEAD")
	if err != nil || actual != candidate {
		return "", errors.New("candidate SHA mismatch")
	}
	status, err := gitText(root, "status", "--porcelain=v1", "--untracked-files=all")
	if err != nil || status != "" {
		return "", errors.New("candidate must have a clean tracked and visible untracked tree")
	}
	return gitText(root, "rev-parse", "HEAD^{tree}")
}

func evidenceMain(args []string, stdout, stderr io.Writer) int {
	flags := flag.NewFlagSet("evidence", flag.ContinueOnError)
	flags.SetOutput(stderr)
	sha := flags.String("candidate-sha", "", "exact frozen candidate SHA")
	output := flags.String("output-dir", "", "new external evidence directory")
	if err := flags.Parse(args); err != nil || flags.NArg() != 0 || *output == "" {
		return 2
	}
	root, err := os.Getwd()
	if err != nil {
		fmt.Fprintln(stderr, err)
		return 1
	}
	outputPath, err := filepath.Abs(*output)
	if err != nil {
		return 1
	}
	if relative, err := filepath.Rel(root, outputPath); err != nil || relative == "." || (!strings.HasPrefix(relative, ".."+string(filepath.Separator)) && relative != "..") {
		fmt.Fprintln(stderr, "evidence directory must be outside the candidate")
		return 1
	}
	if err := os.Mkdir(outputPath, 0o700); err != nil {
		fmt.Fprintln(stderr, err)
		return 1
	}
	report := evidenceReport{SchemaVersion: 1, CandidateSHA: *sha, Status: "FAIL", WorkingDirectory: root, Environment: map[string]string{}}
	for _, required := range requiredSuites() {
		report.Rows = append(report.Rows, suiteResult{TestID: required.ID, Status: "NOT_RUN", ExitCode: -1})
	}
	finish := func(err error) int {
		if err != nil {
			report.Error = err.Error()
		}
		raw, marshalErr := json.MarshalIndent(report, "", "  ")
		if marshalErr != nil {
			return 1
		}
		if err := os.WriteFile(filepath.Join(outputPath, "result.json"), append(raw, '\n'), 0o600); err != nil {
			return 1
		}
		fmt.Fprintln(stdout, string(raw))
		if report.Status == "PASS" {
			return 0
		}
		return 1
	}
	report.CandidateTree, err = candidateIdentity(root, *sha)
	if err != nil {
		return finish(err)
	}
	cmd := exec.Command("go", "env", "-json", "GOFLAGS", "GOWORK", "GOOS", "GOARCH", "GOVERSION", "GOMOD", "GOENV", "GOTOOLCHAIN", "CGO_ENABLED", "GOEXPERIMENT")
	cmd.Dir = root
	envRaw, err := cmd.Output()
	if err != nil {
		return finish(err)
	}
	if err = json.Unmarshal(envRaw, &report.Environment); err != nil {
		return finish(err)
	}
	report.Environment["inherited_GOFLAGS"] = os.Getenv("GOFLAGS")
	if err := validateTestEnvironment(report.Environment); err != nil {
		return finish(err)
	}
	if report.Environment["GOMOD"] != filepath.Join(root, "go.mod") || report.Environment["GOEXPERIMENT"] != "" {
		return finish(errors.New("Go inputs do not match the frozen candidate"))
	}
	for suiteIndex, s := range requiredSuites() {
		row := suiteResult{TestID: s.ID, Status: "NOT_RUN", ExitCode: -1}
		expected, err := expectedTests(root, s)
		if err != nil {
			report.Rows[suiteIndex] = row
			return finish(err)
		}
		for name := range expected {
			row.ExpectedTests = append(row.ExpectedTests, name)
		}
		sort.Strings(row.ExpectedTests)
		row.Command = []string{"go", "test", "-json", "-count=1", "-timeout=10m"}
		for _, dir := range s.Directories {
			row.Command = append(row.Command, "./"+dir+"/...")
		}
		row.Stdout = filepath.Join(outputPath, s.ID+".jsonl")
		row.Stderr = filepath.Join(outputPath, s.ID+".stderr")
		out, err := os.OpenFile(row.Stdout, os.O_CREATE|os.O_EXCL|os.O_WRONLY, 0o600)
		if err != nil {
			return finish(err)
		}
		errFile, err := os.OpenFile(row.Stderr, os.O_CREATE|os.O_EXCL|os.O_WRONLY, 0o600)
		if err != nil {
			out.Close()
			return finish(err)
		}
		command := exec.Command(row.Command[0], row.Command[1:]...)
		command.Dir = root
		command.Env = os.Environ()
		command.Stdout = out
		command.Stderr = errFile
		row.StartedUTC = time.Now().UTC().Format(time.RFC3339Nano)
		runErr := command.Run()
		row.EndedUTC = time.Now().UTC().Format(time.RFC3339Nano)
		out.Close()
		errFile.Close()
		row.ExitCode = 0
		if runErr != nil {
			row.ExitCode = -1
			if command.ProcessState != nil {
				row.ExitCode = command.ProcessState.ExitCode()
			}
		}
		stdoutBytes, err := os.ReadFile(row.Stdout)
		if err != nil {
			return finish(err)
		}
		stderrBytes, err := os.ReadFile(row.Stderr)
		if err != nil {
			return finish(err)
		}
		row.StdoutHash = checkpoint.Hash(stdoutBytes)
		row.StderrHash = checkpoint.Hash(stderrBytes)
		row.Execution, err = verifyEvents(strings.NewReader(string(stdoutBytes)), expected)
		row.Status = "PASS"
		if runErr != nil || err != nil {
			row.Status = "FAIL"
			if err != nil {
				row.Error = err.Error()
			} else {
				row.Error = runErr.Error()
			}
		}
		report.Rows[suiteIndex] = row
		if row.Status != "PASS" {
			return finish(errors.New("required suite failed"))
		}
	}
	after, err := candidateIdentity(root, *sha)
	if err != nil || after != report.CandidateTree {
		return finish(errors.New("candidate changed during test execution"))
	}
	report.Status = "PASS"
	return finish(nil)
}
