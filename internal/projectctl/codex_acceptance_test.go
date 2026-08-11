// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package projectctl

// x-section-id: PROJECTCTL-CODEX-NATIVE-ACCEPTANCE-TESTS
import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/santhosh-tekuri/jsonschema/v6"
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
	tamperedEvidence := payload
	tamperedEvidence.EvidenceDigest = strings.Repeat("0", 64)
	if err := validateNativeAcceptancePayload(tamperedEvidence); err == nil || !strings.Contains(err.Error(), "evidence_digest mismatch") {
		t.Fatalf("evidence digest tampering error=%v", err)
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
	input = completeNativeAcceptanceInput(strings.Repeat("d", 40))
	input.Findings[0].Severity = "URGENT"
	if err := validateNativeAcceptanceInput(input, "M1-B002"); err == nil || !strings.Contains(err.Error(), "invalid severity") {
		t.Fatalf("invalid severity error=%v", err)
	}
	input = completeNativeAcceptanceInput(strings.Repeat("d", 40))
	input.Evidence = nil
	if err := validateNativeAcceptanceInput(input, "M1-B002"); err == nil || !strings.Contains(err.Error(), "non-empty array") {
		t.Fatalf("empty evidence error=%v", err)
	}
	input = completeNativeAcceptanceInput(strings.Repeat("d", 40))
	input.Evidence = append(input.Evidence, input.Evidence[0])
	if err := validateNativeAcceptanceInput(input, "M1-B002"); err == nil || !strings.Contains(err.Error(), "duplicate evidence_id") {
		t.Fatalf("duplicate evidence error=%v", err)
	}
	input = completeNativeAcceptanceInput(strings.Repeat("d", 40))
	input.Findings[0].EvidenceRefs = []string{"CMD-VM-POISON", "CMD-VM-POISON"}
	if err := validateNativeAcceptanceInput(input, "M1-B002"); err == nil || !strings.Contains(err.Error(), "duplicate evidence_ref") {
		t.Fatalf("duplicate evidence ref error=%v", err)
	}
}

func TestNativeAcceptanceCanonicalCollectionsMatchSchema(t *testing.T) {
	route := readingMap{
		Mode: "ACCEPT", Milestone: "M1", RouteScope: batchRouteScope, BatchID: "M1-B002",
		SourceCommit: strings.Repeat("a", 40), SourceTree: strings.Repeat("b", 40), RouteBindingSHA256: strings.Repeat("c", 64),
	}
	for _, verdict := range []string{"PASS", "FAIL", "BLOCKED"} {
		t.Run(verdict, func(t *testing.T) {
			input := completeNativeAcceptanceInput(strings.Repeat("d", 40))
			input.Verdict = verdict
			if verdict != "FAIL" {
				input.Findings = []nativeFindingInput{}
			}
			payload, err := buildNativeAcceptancePayload(input, "example.invalid/project", route, route.SourceTree, strings.Repeat("e", 64), time.Date(2026, time.August, 11, 4, 30, 0, 0, time.UTC))
			if err != nil {
				t.Fatal(err)
			}
			if err := validateNativeAcceptancePayload(payload); err != nil {
				t.Fatal(err)
			}
			if payload.Findings == nil || payload.Evidence == nil {
				t.Fatal("canonical payload collapsed an array to null")
			}
			if err := validateNativeAcceptanceSchema(t, payload); err != nil {
				t.Fatalf("schema parity: %v", err)
			}
		})
	}
	input := completeNativeAcceptanceInput(strings.Repeat("d", 40))
	payload, err := buildNativeAcceptancePayload(input, "example.invalid/project", route, route.SourceTree, strings.Repeat("e", 64), time.Now())
	if err != nil {
		t.Fatal(err)
	}
	payload.Findings = nil
	if err := validateNativeAcceptancePayload(payload); err == nil || !strings.Contains(err.Error(), "findings must be an array") {
		t.Fatalf("null findings parser error=%v", err)
	}
	if err := validateNativeAcceptanceSchema(t, payload); err == nil {
		t.Fatal("schema accepted null findings")
	}
	payload, err = buildNativeAcceptancePayload(input, "example.invalid/project", route, route.SourceTree, strings.Repeat("e", 64), time.Now())
	if err != nil {
		t.Fatal(err)
	}
	payload.Evidence = nil
	if err := validateNativeAcceptancePayload(payload); err == nil || !strings.Contains(err.Error(), "evidence must be a non-empty array") {
		t.Fatalf("null evidence parser error=%v", err)
	}
	if err := validateNativeAcceptanceSchema(t, payload); err == nil {
		t.Fatal("schema accepted null evidence")
	}
}

func TestNativeAcceptanceVerifyRebindsCurrentRepository(t *testing.T) {
	a, candidate, blocking := newNativeAcceptanceFixture(t)
	inputPath := writeNativeAcceptanceInput(t, a.root, completeNativeAcceptanceInput(blocking))
	if err := a.recordNativeAcceptance(context.Background(), inputPath, time.Date(2026, time.August, 11, 4, 30, 0, 0, time.UTC)); err != nil {
		t.Fatal(err)
	}
	payload, err := readNativeAcceptancePayload(filepath.Join(a.root, filepath.FromSlash(nativeFindingOutputRelative)))
	if err != nil {
		t.Fatal(err)
	}
	if err := a.verifyNativeAcceptance(context.Background(), payload); err != nil {
		t.Fatalf("exact native repository binding: %v", err)
	}
	cases := []struct {
		name   string
		mutate func(*nativeAcceptancePayload)
	}{
		{name: "project", mutate: func(value *nativeAcceptancePayload) { value.ProjectID = "foreign.invalid/project" }},
		{name: "candidate", mutate: func(value *nativeAcceptancePayload) {
			value.AcceptanceCandidateSHA = blocking
			for index := range value.Findings {
				value.Findings[index].AcceptanceCandidateSHA = blocking
			}
		}},
		{name: "tree", mutate: func(value *nativeAcceptancePayload) { value.AcceptanceCandidateTree = strings.Repeat("f", 40) }},
		{name: "contract", mutate: func(value *nativeAcceptancePayload) { value.FrozenContractSHA256 = strings.Repeat("f", 64) }},
		{name: "route", mutate: func(value *nativeAcceptancePayload) { value.RouteBindingSHA256 = strings.Repeat("f", 64) }},
		{name: "blocking", mutate: func(value *nativeAcceptancePayload) { value.BlockingCommitSHA = candidate }},
	}
	for _, test := range cases {
		t.Run(test.name, func(t *testing.T) {
			tampered := cloneNativeAcceptancePayload(t, payload)
			test.mutate(&tampered)
			rebindNativeAcceptancePayload(t, &tampered)
			if err := validateNativeAcceptancePayload(tampered); err != nil {
				t.Fatalf("tampered fixture is not internally self-consistent: %v", err)
			}
			if err := a.verifyNativeAcceptance(context.Background(), tampered); err == nil || !strings.Contains(err.Error(), "NATIVE_ACCEPTANCE_") {
				t.Fatalf("foreign %s binding error=%v", test.name, err)
			}
		})
	}
}

func TestNativeAcceptanceRejectsInvalidBlockingBindings(t *testing.T) {
	t.Run("non-descendant", func(t *testing.T) {
		a, candidate, _ := newNativeAcceptanceFixture(t)
		parent := fixtureGitOutput(t, a.root, "rev-parse", candidate+"^")
		tree := fixtureGitOutput(t, a.root, "rev-parse", candidate+"^{tree}")
		blocking := fixtureGitOutput(t, a.root, "-c", "user.name=Codex Fixture", "-c", "user.email=fixture@example.invalid", "commit-tree", tree, "-p", parent, "-m", "sibling blocking commit")
		err := a.recordNativeAcceptance(context.Background(), writeNativeAcceptanceInput(t, a.root, completeNativeAcceptanceInput(blocking)), time.Now())
		if err == nil || !strings.Contains(err.Error(), "not a descendant") {
			t.Fatalf("non-descendant blocking error=%v", err)
		}
	})
	t.Run("state", func(t *testing.T) {
		a, candidate, _ := newNativeAcceptanceFixture(t)
		tree := fixtureGitOutput(t, a.root, "rev-parse", candidate+"^{tree}")
		blocking := fixtureGitOutput(t, a.root, "-c", "user.name=Codex Fixture", "-c", "user.email=fixture@example.invalid", "commit-tree", tree, "-p", candidate, "-m", "still verifying")
		err := a.recordNativeAcceptance(context.Background(), writeNativeAcceptanceInput(t, a.root, completeNativeAcceptanceInput(blocking)), time.Now())
		if err == nil || !strings.Contains(err.Error(), "require BLOCKED") {
			t.Fatalf("blocking state error=%v", err)
		}
	})
	t.Run("frozen-contract", func(t *testing.T) {
		a, candidate, _ := newNativeAcceptanceFixture(t)
		plan, err := loadYAML[milestonePlan](a.root, ".codex/state/MILESTONE_PLAN.yaml")
		if err != nil {
			t.Fatal(err)
		}
		for index := range plan.Batches {
			if plan.Batches[index].BatchID == "M1-B002" {
				plan.Batches[index].State = "BLOCKED"
				plan.Batches[index].Objective += " with a foreign contract"
				plan.Batches[index].FrozenContractSHA256 = mustBatchContractDigest(t, plan.Batches[index])
			}
		}
		plan.PlanVersion++
		writeFixtureMilestonePlan(t, a, plan)
		commitFixturePaths(t, a, "blocking with a foreign frozen contract", ".codex/state/MILESTONE_PLAN.yaml")
		blocking := fixtureGitOutput(t, a.root, "rev-parse", "HEAD")
		runFixtureGit(t, a.root, "checkout", "--quiet", "--detach", candidate)
		err = a.recordNativeAcceptance(context.Background(), writeNativeAcceptanceInput(t, a.root, completeNativeAcceptanceInput(blocking)), time.Now())
		if err == nil || !strings.Contains(err.Error(), "frozen contract differs") {
			t.Fatalf("blocking frozen contract error=%v", err)
		}
	})
}

func TestNativeAcceptancePersistenceFailureLeavesNoOutput(t *testing.T) {
	a, _, blocking := newNativeAcceptanceFixture(t)
	inputPath := writeNativeAcceptanceInput(t, a.root, completeNativeAcceptanceInput(blocking))
	original := writeNativeAcceptanceAtomically
	writeNativeAcceptanceAtomically = func(string, []byte, os.FileMode) error { return errors.New("injected atomic persistence failure") }
	t.Cleanup(func() { writeNativeAcceptanceAtomically = original })
	err := a.recordNativeAcceptance(context.Background(), inputPath, time.Now())
	if err == nil || !strings.Contains(err.Error(), "injected atomic persistence failure") {
		t.Fatalf("atomic persistence error=%v", err)
	}
	if _, statErr := os.Stat(filepath.Join(a.root, filepath.FromSlash(nativeFindingOutputRelative))); !os.IsNotExist(statErr) {
		t.Fatalf("failed persistence left output: %v", statErr)
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

func validateNativeAcceptanceSchema(t *testing.T, payload nativeAcceptancePayload) error {
	t.Helper()
	root, err := findRoot()
	if err != nil {
		return err
	}
	schemaData, err := os.ReadFile(filepath.Join(root, "schemas", "codex", "native-finding-payload-v1.schema.json"))
	if err != nil {
		return err
	}
	compiler := jsonschema.NewCompiler()
	compiler.DefaultDraft(jsonschema.Draft2020)
	document, err := jsonschema.UnmarshalJSON(bytes.NewReader(schemaData))
	if err != nil {
		return err
	}
	const schemaID = "https://github.com/zyc14588/TRPG_PLATFORM/schemas/codex/native-finding-payload-v1.schema.json"
	if err := compiler.AddResource(schemaID, document); err != nil {
		return err
	}
	schema, err := compiler.Compile(schemaID)
	if err != nil {
		return err
	}
	data, err := json.Marshal(payload)
	if err != nil {
		return err
	}
	instance, err := jsonschema.UnmarshalJSON(bytes.NewReader(data))
	if err != nil {
		return err
	}
	return schema.Validate(instance)
}

func cloneNativeAcceptancePayload(t *testing.T, payload nativeAcceptancePayload) nativeAcceptancePayload {
	t.Helper()
	data, err := json.Marshal(payload)
	if err != nil {
		t.Fatal(err)
	}
	var clone nativeAcceptancePayload
	if err := json.Unmarshal(data, &clone); err != nil {
		t.Fatal(err)
	}
	return clone
}

func rebindNativeAcceptancePayload(t *testing.T, payload *nativeAcceptancePayload) {
	t.Helper()
	findings := make([]nativeFindingPayload, len(payload.Findings))
	copy(findings, payload.Findings)
	for index := range findings {
		findings[index].AcceptanceIdentity = ""
	}
	digest, err := digestJSON(findings)
	if err != nil {
		t.Fatal(err)
	}
	payload.FindingSetDigest = digest
	identity, err := nativeAcceptanceIdentity(*payload)
	if err != nil {
		t.Fatal(err)
	}
	payload.NativeAcceptanceIdentity = identity
	for index := range payload.Findings {
		payload.Findings[index].AcceptanceIdentity = identity
	}
}
