// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package projectctl

// x-section-id: PROJECTCTL-CODEX-NATIVE-ACCEPTANCE-TESTS
import (
	"bytes"
	"context"
	"encoding/json"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"gopkg.in/yaml.v3"
)

func TestNativeAcceptanceRecordBindsExactRouteCandidateAndBlockingCommit(t *testing.T) {
	a, candidate, blocking := newNativeAcceptanceFixture(t)
	inputPath := writeNativeAcceptanceInput(t, a.root, completeNativeAcceptanceInput(blocking))
	fixedTime := time.Date(2026, time.August, 11, 4, 30, 0, 123456000, time.UTC)
	if err := a.recordNativeAcceptance(context.Background(), inputPath, fixedTime); err != nil {
		t.Fatal(err)
	}
	outputPath := filepath.Join(a.root, filepath.FromSlash(nativeFindingOutputRelative))
	payload, err := readNativeAcceptancePayload(outputPath)
	if err != nil {
		t.Fatal(err)
	}
	if err := validateNativeAcceptancePayload(payload); err != nil {
		t.Fatal(err)
	}
	if payload.AcceptanceCandidateSHA != candidate || payload.BlockingCommitSHA != blocking {
		t.Fatalf("payload candidate/blocking = %s/%s, want %s/%s", payload.AcceptanceCandidateSHA, payload.BlockingCommitSHA, candidate, blocking)
	}
	if payload.Milestone != "M1" || payload.BatchID != "M1-B002" || payload.Verdict != "FAIL" {
		t.Fatalf("unexpected native acceptance lifecycle: %+v", payload)
	}
	if len(payload.Findings) != 1 || payload.Findings[0].FindingID != "ACC-M1-B002-008" {
		t.Fatalf("unexpected findings: %+v", payload.Findings)
	}
	if payload.Findings[0].AcceptanceCandidateSHA != candidate || payload.Findings[0].AcceptanceIdentity != payload.NativeAcceptanceIdentity || payload.Findings[0].CreatedAt != payload.CreatedAt {
		t.Fatal("finding does not inherit immutable candidate/identity/time binding")
	}
	if !strings.HasPrefix(payload.NativeAcceptanceIdentity, "native-acceptance:sha256:") || len(payload.NativeAcceptanceIdentity) != len("native-acceptance:sha256:")+64 {
		t.Fatalf("invalid native acceptance identity %q", payload.NativeAcceptanceIdentity)
	}
	if payload.CreatedAt != "2026-08-11T04:30:00.123456Z" {
		t.Fatalf("created_at=%q", payload.CreatedAt)
	}

	first, err := os.ReadFile(outputPath)
	if err != nil {
		t.Fatal(err)
	}
	if err := a.recordNativeAcceptance(context.Background(), inputPath, fixedTime); err != nil {
		t.Fatalf("idempotent record: %v", err)
	}
	second, err := os.ReadFile(outputPath)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(first, second) {
		t.Fatal("idempotent record changed immutable output")
	}
}

func TestNativeAcceptanceRejectsIncompleteFindingWithoutOutput(t *testing.T) {
	a, _, blocking := newNativeAcceptanceFixture(t)
	input := completeNativeAcceptanceInput(blocking)
	input.Findings[0].RepairBoundary = ""
	inputPath := writeNativeAcceptanceInput(t, a.root, input)
	err := a.recordNativeAcceptance(context.Background(), inputPath, time.Now())
	if err == nil || !strings.Contains(err.Error(), "REPAIR_CONTEXT_INCOMPLETE") || !strings.Contains(err.Error(), "repair_boundary") {
		t.Fatalf("incomplete finding error=%v", err)
	}
	if _, statErr := os.Stat(filepath.Join(a.root, filepath.FromSlash(nativeFindingOutputRelative))); !os.IsNotExist(statErr) {
		t.Fatalf("invalid input created output: %v", statErr)
	}
}

func TestNativeAcceptanceRejectsCandidateAsBlockingCommit(t *testing.T) {
	a, candidate, _ := newNativeAcceptanceFixture(t)
	inputPath := writeNativeAcceptanceInput(t, a.root, completeNativeAcceptanceInput(candidate))
	err := a.recordNativeAcceptance(context.Background(), inputPath, time.Now())
	if err == nil || !strings.Contains(err.Error(), "blocking commit must differ") {
		t.Fatalf("candidate/blocking mismatch error=%v", err)
	}
}

func TestNativeAcceptanceRejectsStaleRoute(t *testing.T) {
	a, _, blocking := newNativeAcceptanceFixture(t)
	route, err := loadYAML[readingMap](a.root, ".codex/runtime/READING_MAP.yaml")
	if err != nil {
		t.Fatal(err)
	}
	route.SourceCommit = strings.Repeat("a", 40)
	data, err := yaml.Marshal(route)
	if err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(a.root, ".codex", "runtime", "READING_MAP.yaml"), data, 0o644); err != nil {
		t.Fatal(err)
	}
	inputPath := writeNativeAcceptanceInput(t, a.root, completeNativeAcceptanceInput(blocking))
	err = a.recordNativeAcceptance(context.Background(), inputPath, time.Now())
	if err == nil || !strings.Contains(err.Error(), "native ACCEPT route validation") {
		t.Fatalf("stale route error=%v", err)
	}
}

func TestNativeAcceptancePayloadRejectsDigestAndIdentityTampering(t *testing.T) {
	route := readingMap{
		Mode: "ACCEPT", Milestone: "M1", RouteScope: batchRouteScope, BatchID: "M1-B002",
		SourceCommit: strings.Repeat("a", 40), SourceTree: strings.Repeat("b", 40), RouteBindingSHA256: strings.Repeat("c", 64),
	}
	payload, err := buildNativeAcceptancePayload(
		completeNativeAcceptanceInput(strings.Repeat("d", 40)),
		"example.invalid/project", route, route.SourceTree, strings.Repeat("e", 64),
		time.Date(2026, time.August, 11, 4, 30, 0, 0, time.UTC),
	)
	if err != nil {
		t.Fatal(err)
	}
	if err := validateNativeAcceptancePayload(payload); err != nil {
		t.Fatal(err)
	}
	tamperedDigest := payload
	tamperedDigest.FindingSetDigest = strings.Repeat("f", 64)
	if err := validateNativeAcceptancePayload(tamperedDigest); err == nil || !strings.Contains(err.Error(), "finding_set_digest mismatch") {
		t.Fatalf("finding digest tampering error=%v", err)
	}
	tamperedCandidate := payload
	tamperedCandidate.Findings = append([]nativeFindingPayload(nil), payload.Findings...)
	tamperedCandidate.Findings[0].AcceptanceCandidateSHA = strings.Repeat("f", 40)
	if err := validateNativeAcceptancePayload(tamperedCandidate); err == nil || !strings.Contains(err.Error(), "binding mismatch") {
		t.Fatalf("candidate tampering error=%v", err)
	}
	tamperedIdentity := payload
	tamperedIdentity.NativeAcceptanceIdentity = "native-acceptance:sha256:" + strings.Repeat("0", 64)
	if err := validateNativeAcceptancePayload(tamperedIdentity); err == nil {
		t.Fatal("acceptance identity tampering passed")
	}
}

func TestNativeAcceptanceInputRejectsDuplicateAndForeignBindings(t *testing.T) {
	input := completeNativeAcceptanceInput(strings.Repeat("d", 40))
	input.Findings = append(input.Findings, input.Findings[0])
	if err := validateNativeAcceptanceInput(input, "M1-B002"); err == nil || !strings.Contains(err.Error(), "duplicate finding_id") {
		t.Fatalf("duplicate finding error=%v", err)
	}
	input = completeNativeAcceptanceInput(strings.Repeat("d", 40))
	input.Findings[0].FindingID = "ACC-M1-B003-008"
	if err := validateNativeAcceptanceInput(input, "M1-B002"); err == nil || !strings.Contains(err.Error(), "not bound to batch") {
		t.Fatalf("foreign finding error=%v", err)
	}
	input = completeNativeAcceptanceInput(strings.Repeat("d", 40))
	input.Findings[0].EvidenceRefs = []string{"EVIDENCE-MISSING"}
	if err := validateNativeAcceptanceInput(input, "M1-B002"); err == nil || !strings.Contains(err.Error(), "no matching evidence") {
		t.Fatalf("foreign evidence error=%v", err)
	}
}

func completeNativeAcceptanceInput(blocking string) nativeAcceptanceRecordInput {
	exitCode := 1
	return nativeAcceptanceRecordInput{
		SchemaVersion:     nativeFindingPayloadSchemaVersion,
		Verdict:           "FAIL",
		BlockingCommitSHA: blocking,
		Findings: []nativeFindingInput{
			{
				FindingID: "ACC-M1-B002-008", Severity: "HIGH", Location: "internal/example/runtime.go:42",
				Reproduction: "run the isolated native acceptance regression", ViolatedRequirement: "REQ-LUA-002",
				Expected: "a failed value conversion poisons the VM", Actual: "the VM remains reusable",
				RepairBoundary: "limit repair to the runtime failure boundary and its regression tests",
				EvidenceRefs:   []string{"CMD-VM-POISON"},
			},
		},
		Evidence: []nativeAcceptanceEvidence{
			{EvidenceID: "CMD-VM-POISON", Kind: "command", Reference: "isolated acceptance command", Command: "go test ./internal/example -run TestPoison", ExitCode: &exitCode, Result: "REPRODUCED"},
		},
	}
}

func newNativeAcceptanceFixture(t *testing.T) (*App, string, string) {
	t.Helper()
	source := testApp(t)
	root := filepath.Join(t.TempDir(), "repo")
	command := exec.Command("git", "clone", "--quiet", "--shared", source.root, root)
	if output, err := command.CombinedOutput(); err != nil {
		t.Fatalf("clone native acceptance fixture: %v\n%s", err, output)
	}
	a := &App{root: root, stdout: &bytes.Buffer{}, stderr: &bytes.Buffer{}}
	setNativeAcceptanceBatchState(t, a, "VERIFYING", "native acceptance verifying candidate")
	candidate := fixtureGitOutput(t, root, "rev-parse", "HEAD")
	request := testCodexRequest("ACCEPT", "M1", "M1-B002")
	if err := a.generateCodexRouteRequest(context.Background(), request); err != nil {
		t.Fatalf("generate fixture ACCEPT route: %v", err)
	}
	setNativeAcceptanceBatchState(t, a, "BLOCKED", "native acceptance blocking commit")
	blocking := fixtureGitOutput(t, root, "rev-parse", "HEAD")
	runFixtureGit(t, root, "checkout", "--quiet", "--detach", candidate)
	if err := a.checkCodex(context.Background()); err != nil {
		t.Fatalf("fixture ACCEPT route after detached checkout: %v", err)
	}
	return a, candidate, blocking
}

func setNativeAcceptanceBatchState(t *testing.T, a *App, state, message string) {
	t.Helper()
	plan, err := loadYAML[milestonePlan](a.root, ".codex/state/MILESTONE_PLAN.yaml")
	if err != nil {
		t.Fatal(err)
	}
	found := false
	for index := range plan.Batches {
		if plan.Batches[index].BatchID == "M1-B002" {
			plan.Batches[index].State = state
			found = true
			break
		}
	}
	if !found {
		t.Fatal("fixture has no M1-B002")
	}
	plan.PlanVersion++
	writeFixtureMilestonePlan(t, a, plan)
	commitFixturePaths(t, a, message, ".codex/state/MILESTONE_PLAN.yaml")
}

func writeNativeAcceptanceInput(t *testing.T, root string, input nativeAcceptanceRecordInput) string {
	t.Helper()
	data, err := json.Marshal(input)
	if err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(root, ".codex", "runtime", "NATIVE_ACCEPTANCE_INPUT.json")
	if err := os.WriteFile(path, append(data, '\n'), 0o600); err != nil {
		t.Fatal(err)
	}
	return path
}

func fixtureGitOutput(t *testing.T, root string, args ...string) string {
	t.Helper()
	command := exec.Command("git", append([]string{"-C", root}, args...)...)
	output, err := command.CombinedOutput()
	if err != nil {
		t.Fatalf("git %s: %v\n%s", strings.Join(args, " "), err, output)
	}
	return strings.TrimSpace(string(output))
}
