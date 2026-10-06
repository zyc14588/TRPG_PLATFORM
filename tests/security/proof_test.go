//go:build linux && security

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package security_test

import (
	"bufio"
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/url"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"sort"
	"strings"
	"sync"
	"syscall"
	"testing"
	"time"
)

const module = "github.com/zyc14588/TRPG_PLATFORM/"

type target struct {
	path  string
	tests []string
}
type matrix struct {
	name        string
	integration bool
	targets     []target
}
type source struct {
	Commit string `json:"commit"`
	Tree   string `json:"tree"`
	Clean  bool   `json:"clean"`
}
type counts struct {
	Run  int `json:"run"`
	Pass int `json:"pass"`
	Fail int `json:"fail"`
	Skip int `json:"skip"`
}
type proof struct {
	Matrix      string   `json:"matrix"`
	Before      source   `json:"source_before"`
	After       source   `json:"source_after"`
	Argv        []string `json:"argv"`
	ExitCode    int      `json:"exit_code"`
	Named       counts   `json:"named_counts"`
	Executed    []string `json:"actual_named_passes"`
	Expected    []string `json:"required_named_tests"`
	Log         string   `json:"raw_log"`
	LogHash     string   `json:"raw_log_sha256"`
	Privacy     string   `json:"ordinary_output_privacy"`
	Verdict     string   `json:"verdict"`
	FailureCode string   `json:"failure_code,omitempty"`
	StartedUTC  string   `json:"started_utc"`
	EndedUTC    string   `json:"ended_utc"`
}

var root, evidence string
var candidate source
var childEnv []string
var forbiddenMarkers []string
var results []proof

func TestMain(m *testing.M) {
	var err error
	root, err = filepath.Abs("../..")
	if err != nil {
		setupFail()
	}
	candidate, err = snapshot()
	if err != nil || !candidate.Clean {
		setupFail()
	}
	u, err := url.Parse(os.Getenv("B008_POSTGRES_DSN"))
	if err != nil || u.Scheme != "postgres" || u.Hostname() != "127.0.0.1" || u.Port() == "" || u.Path != "/b008_fixture" || u.User == nil {
		setupFail()
	}
	password, ok := u.User.Password()
	if !ok || password == "" {
		setupFail()
	}
	childEnv = os.Environ()
	for key, db := range map[string]string{"B005_POSTGRES_DSN": "b005_fixture", "B006_POSTGRES_DSN": "b006_fixture", "B007_POSTGRES_DSN": "b007_fixture", "TRPG_HOSTAPI_PG_DSN": "b008_fixture"} {
		v := *u
		v.Path = "/" + db
		childEnv = replaceEnv(childEnv, key, v.String())
	}
	forbiddenMarkers = []string{"fixture-gm-private-value", "sensitive-fixture-value", "must-not-reach-runner", "migration-fixture-gm-private", password}
	parent := os.Getenv("TRPG_SECURITY_EVIDENCE_DIR")
	if parent == "" {
		evidence, err = os.MkdirTemp("", "trpg-security-evidence-")
	} else if !filepath.IsAbs(parent) {
		setupFail()
	} else {
		// The caller chooses an owned directory; every invocation gets new files,
		// so a rerun cannot overwrite or erase a failed attempt.
		if err = os.MkdirAll(parent, 0700); err == nil {
			evidence, err = os.MkdirTemp(parent, "run-")
		}
	}
	if err != nil {
		setupFail()
	}
	code := m.Run()
	final, err := snapshot()
	if err != nil || final != candidate {
		code = 1
	}
	verdict := "FAIL"
	if code == 0 && len(results) == len(matrices) {
		verdict = "PASS"
	} else if code == 0 {
		verdict = "NOT_RUN_SELECTION_INCOMPLETE"
		code = 1
	}
	result := struct {
		Schema    string  `json:"schema"`
		TestID    string  `json:"test_id"`
		Platform  string  `json:"platform"`
		Verdict   string  `json:"verdict"`
		Candidate source  `json:"candidate"`
		Matrices  []proof `json:"matrices"`
	}{"security-matrix-result/1", "TEST-SEC-001", "Linux", verdict, candidate, results}
	if err := saveJSON(filepath.Join(evidence, "RESULT.json"), result); err != nil {
		code = 1
	}
	fmt.Printf("SECURITY_RESULT candidate=%s verdict=%s evidence=%s\n", candidate.Commit, verdict, evidence)
	os.Exit(code)
}

func setupFail() {
	// Never echo supplied configuration, credentials, or low-level errors.
	fmt.Fprintln(os.Stderr, "SECURITY_REQUIRED_CLEAN_SOURCE_AND_OWNED_FIXTURE_NOT_RUN")
	os.Exit(2)
}
func snapshot() (source, error) {
	var out source
	values := []*string{&out.Commit, &out.Tree}
	for i, ref := range []string{"HEAD", "HEAD^{tree}"} {
		cmd := exec.Command("git", "rev-parse", ref)
		cmd.Dir = root
		raw, err := cmd.Output()
		if err != nil {
			return out, err
		}
		*values[i] = strings.TrimSpace(string(raw))
		if !regexp.MustCompile(`^[0-9a-f]{40}$`).MatchString(*values[i]) {
			return out, errors.New("SECURITY_SOURCE_ID")
		}
	}
	cmd := exec.Command("git", "status", "--porcelain=v1")
	cmd.Dir = root
	raw, err := cmd.Output()
	out.Clean = len(raw) == 0
	return out, err
}
func replaceEnv(env []string, key, value string) []string {
	out := make([]string, 0, len(env)+1)
	for _, entry := range env {
		if !strings.HasPrefix(entry, key+"=") {
			out = append(out, entry)
		}
	}
	return append(out, key+"="+value)
}
func saveJSON(path string, value any) error {
	raw, err := json.MarshalIndent(value, "", "  ")
	if err != nil {
		return err
	}
	f, err := os.OpenFile(path, os.O_WRONLY|os.O_CREATE|os.O_EXCL, 0600)
	if err != nil {
		return err
	}
	_, writeErr := f.Write(append(raw, '\n'))
	closeErr := f.Close()
	return errors.Join(writeErr, closeErr)
}

// Bound captured diagnostics before allocating/parsing any child output.
type boundedLog struct {
	mu sync.Mutex
	f  *os.File
	n  int64
}

func (w *boundedLog) Write(raw []byte) (int, error) {
	w.mu.Lock()
	defer w.mu.Unlock()
	if int64(len(raw)) > (32<<20)-w.n {
		return 0, errors.New("SECURITY_CHILD_OUTPUT_BUDGET")
	}
	n, err := w.f.Write(raw)
	w.n += int64(n)
	return n, err
}

func executeMatrix(t *testing.T, m matrix) {
	t.Helper()
	p := proof{Matrix: m.name, Before: candidate, Verdict: "FAIL", Privacy: "NOT_VERIFIED", StartedUTC: time.Now().UTC().Format(time.RFC3339Nano)}
	names, paths := make(map[string]bool), []string{}
	for _, target := range m.targets {
		paths = append(paths, "./"+target.path)
		for _, name := range target.tests {
			names[name] = true
			p.Expected = append(p.Expected, module+target.path+":"+name)
		}
	}
	sort.Strings(p.Expected)
	var alternatives []string
	for name := range names {
		alternatives = append(alternatives, regexp.QuoteMeta(name))
	}
	sort.Strings(alternatives)
	p.Argv = []string{"go", "test", "-count=1", "-json", "-timeout=6m"}
	if m.integration {
		p.Argv = append(p.Argv, "-tags=integration")
	}
	p.Argv = append(p.Argv, "-run=^("+strings.Join(alternatives, "|")+")$")
	p.Argv = append(p.Argv, paths...)
	p.Log = filepath.Join(evidence, m.name+".jsonl")
	f, err := os.OpenFile(p.Log, os.O_CREATE|os.O_EXCL|os.O_RDWR, 0600)
	if err != nil {
		t.Fatal("SECURITY_EVIDENCE_CREATE")
	}
	ctx, cancel := context.WithTimeout(context.Background(), 7*time.Minute)
	defer cancel()
	cmd := exec.CommandContext(ctx, p.Argv[0], p.Argv[1:]...)
	cmd.Dir, cmd.Env = root, childEnv
	cmd.SysProcAttr = &syscall.SysProcAttr{Setpgid: true}
	cmd.Cancel = func() error { return syscall.Kill(-cmd.Process.Pid, syscall.SIGKILL) }
	cmd.WaitDelay = 3 * time.Second
	log := &boundedLog{f: f}
	cmd.Stdout, cmd.Stderr = log, log
	err = cmd.Run()
	p.ExitCode = 0
	if err != nil {
		p.ExitCode = 1
		var exit *exec.ExitError
		if errors.As(err, &exit) {
			p.ExitCode = exit.ExitCode()
		}
		p.FailureCode = "SECURITY_CHILD_FAILED"
	}
	if _, err = f.Seek(0, io.SeekStart); err == nil {
		hash := sha256.New()
		_, err = io.Copy(hash, f)
		p.LogHash = hex.EncodeToString(hash.Sum(nil))
	}
	if err == nil {
		_, err = f.Seek(0, io.SeekStart)
	}
	if err == nil {
		p.Named, p.Executed, err = verifyEvents(f, p.Expected, forbiddenMarkers)
	}
	if err != nil {
		p.FailureCode = "SECURITY_CHILD_EVIDENCE_REJECTED"
	} else {
		p.Privacy = "PASS_NO_PRIVATE_FIXTURE_OR_CREDENTIAL_MARKERS"
	}
	if err = f.Close(); err != nil {
		p.FailureCode = "SECURITY_EVIDENCE_CLOSE"
	}
	p.After, err = snapshot()
	if err != nil || p.After != candidate {
		p.FailureCode = "SECURITY_SOURCE_CHANGED"
	}
	p.EndedUTC = time.Now().UTC().Format(time.RFC3339Nano)
	if p.ExitCode == 0 && p.FailureCode == "" {
		p.Verdict = "PASS"
	}
	if err = saveJSON(filepath.Join(evidence, m.name+".json"), p); err != nil {
		t.Fatal("SECURITY_EVIDENCE_WRITE")
	}
	results = append(results, p)
	raw, _ := json.Marshal(p)
	t.Logf("SECURITY_MATRIX_PROOF %s", raw)
	if p.Verdict != "PASS" {
		t.Fatal("SECURITY_MATRIX_FAILED; inspect owned restricted evidence")
	}
}

func verifyEvents(r io.Reader, expected, markers []string) (counts, []string, error) {
	var count counts
	tests := make(map[string]counts)
	scanner := bufio.NewScanner(r)
	scanner.Buffer(make([]byte, 4096), 2<<20)
	for scanner.Scan() {
		line := scanner.Text()
		for _, marker := range markers {
			if marker != "" && strings.Contains(line, marker) {
				return count, nil, errors.New("SECURITY_PRIVATE_DIAGNOSTIC")
			}
		}
		var event struct{ Action, Package, Test string }
		if err := json.Unmarshal([]byte(line), &event); err != nil || event.Package == "" {
			return count, nil, errors.New("SECURITY_NON_JSON_EVIDENCE")
		}
		if event.Test == "" {
			if event.Action == "fail" || event.Action == "skip" {
				return count, nil, errors.New("SECURITY_PACKAGE_NOT_PASSED")
			}
			continue
		}
		key := event.Package + ":" + event.Test
		value := tests[key]
		switch event.Action {
		case "run":
			count.Run++
			value.Run++
		case "pass":
			count.Pass++
			value.Pass++
		case "fail":
			count.Fail++
			value.Fail++
		case "skip":
			count.Skip++
			value.Skip++
		}
		tests[key] = value
	}
	if scanner.Err() != nil || count.Run == 0 || count.Run != count.Pass || count.Fail != 0 || count.Skip != 0 {
		return count, nil, errors.New("SECURITY_NAMED_TESTS_NOT_PASSED")
	}
	for _, name := range expected {
		if tests[name] != (counts{Run: 1, Pass: 1}) {
			return count, nil, errors.New("SECURITY_REQUIRED_TEST_NOT_EXECUTED")
		}
	}
	var passed []string
	for name, value := range tests {
		if value != (counts{Run: 1, Pass: 1}) {
			return count, nil, errors.New("SECURITY_INCOMPLETE_TEST")
		}
		passed = append(passed, name)
	}
	sort.Strings(passed)
	return count, passed, nil
}

func TestSecurityProofRejectsMissingSkippedFailedAndPrivateOutput(t *testing.T) {
	const name = "fixture:TestBoundary"
	good := "{\"Action\":\"run\",\"Package\":\"fixture\",\"Test\":\"TestBoundary\"}\n{\"Action\":\"pass\",\"Package\":\"fixture\",\"Test\":\"TestBoundary\"}\n"
	for label, raw := range map[string]string{
		"no-tests":   "{\"Action\":\"pass\",\"Package\":\"fixture\"}\n",
		"missing":    strings.ReplaceAll(good, "TestBoundary", "TestOther"),
		"skipped":    strings.Replace(good, "\"pass\"", "\"skip\"", 1),
		"failed":     strings.Replace(good, "\"pass\"", "\"fail\"", 1),
		"unfinished": strings.Split(good, "\n")[0] + "\n",
		"duplicate":  good + good,
		"private":    good + "{\"Action\":\"output\",\"Package\":\"fixture\",\"Output\":\"synthetic-private-marker\"}\n",
		"unframed":   good + "not-json\n",
	} {
		t.Run(label, func(t *testing.T) {
			if _, _, err := verifyEvents(strings.NewReader(raw), []string{name}, []string{"synthetic-private-marker"}); err == nil {
				t.Fatal("invalid acceptance proof admitted")
			}
		})
	}
	if count, passed, err := verifyEvents(strings.NewReader(good), []string{name}, nil); err != nil || count != (counts{Run: 1, Pass: 1}) || len(passed) != 1 {
		t.Fatal("actual named proof rejected")
	}
}
