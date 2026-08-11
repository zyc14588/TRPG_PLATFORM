// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package projectctl

// x-section-id: PROJECTCTL-CODEX-NATIVE-ACCEPTANCE
import (
	"bytes"
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"regexp"
	"sort"
	"strings"
	"time"

	"gopkg.in/yaml.v3"
)

const (
	nativeFindingPayloadSchemaVersion = 1
	nativeFindingPayloadType          = "native-finding-payload"
	nativeFindingOutputRelative       = ".codex/runtime/NATIVE_FINDING_PAYLOAD.json"
)

var writeNativeAcceptanceAtomically = atomicWrite

var (
	nativeFindingIDPattern  = regexp.MustCompile(`^ACC-(M(?:0|[1-9][0-9]*)-B[0-9]{3,})-[0-9]{3,}$`)
	nativeSeverityPattern   = regexp.MustCompile(`^(CRITICAL|HIGH|MEDIUM|LOW|P[0-3])$`)
	nativeEvidenceIDPattern = regexp.MustCompile(`^[A-Z0-9][A-Z0-9._:-]{1,127}$`)
)

type nativeAcceptanceRecordInput struct {
	SchemaVersion     int                        `json:"schema_version"`
	Verdict           string                     `json:"verdict"`
	BlockingCommitSHA string                     `json:"blocking_commit_sha"`
	Findings          []nativeFindingInput       `json:"findings"`
	Evidence          []nativeAcceptanceEvidence `json:"evidence"`
}

type nativeFindingInput struct {
	FindingID           string   `json:"finding_id"`
	Severity            string   `json:"severity"`
	Location            string   `json:"location"`
	Reproduction        string   `json:"reproduction"`
	ViolatedRequirement string   `json:"violated_requirement"`
	Expected            string   `json:"expected"`
	Actual              string   `json:"actual"`
	RepairBoundary      string   `json:"repair_boundary"`
	EvidenceRefs        []string `json:"evidence_refs"`
}

type nativeAcceptanceEvidence struct {
	EvidenceID string `json:"evidence_id"`
	Kind       string `json:"kind"`
	Reference  string `json:"reference"`
	Command    string `json:"command,omitempty"`
	ExitCode   *int   `json:"exit_code,omitempty"`
	Result     string `json:"result"`
	SHA256     string `json:"sha256,omitempty"`
}

type nativeFindingPayload struct {
	FindingID              string   `json:"finding_id"`
	Severity               string   `json:"severity"`
	Location               string   `json:"location"`
	Reproduction           string   `json:"reproduction"`
	ViolatedRequirement    string   `json:"violated_requirement"`
	Expected               string   `json:"expected"`
	Actual                 string   `json:"actual"`
	RepairBoundary         string   `json:"repair_boundary"`
	EvidenceRefs           []string `json:"evidence_refs"`
	AcceptanceCandidateSHA string   `json:"acceptance_candidate_sha"`
	AcceptanceIdentity     string   `json:"acceptance_identity"`
	CreatedAt              string   `json:"created_at"`
}

type nativeAcceptancePayload struct {
	SchemaVersion            int                        `json:"schema_version"`
	PayloadType              string                     `json:"payload_type"`
	ProjectID                string                     `json:"project_id"`
	Milestone                string                     `json:"milestone"`
	BatchID                  string                     `json:"batch_id"`
	Verdict                  string                     `json:"verdict"`
	AcceptanceCandidateSHA   string                     `json:"acceptance_candidate_sha"`
	AcceptanceCandidateTree  string                     `json:"acceptance_candidate_tree"`
	BlockingCommitSHA        string                     `json:"blocking_commit_sha"`
	FrozenContractSHA256     string                     `json:"frozen_contract_sha256"`
	RouteBindingSHA256       string                     `json:"route_binding_sha256"`
	FindingSetDigest         string                     `json:"finding_set_digest"`
	EvidenceDigest           string                     `json:"evidence_digest"`
	Findings                 []nativeFindingPayload     `json:"findings"`
	Evidence                 []nativeAcceptanceEvidence `json:"evidence"`
	NativeAcceptanceIdentity string                     `json:"native_acceptance_identity"`
	CreatedAt                string                     `json:"created_at"`
}

type nativeAcceptanceIdentityMaterial struct {
	SchemaVersion           int    `json:"schema_version"`
	PayloadType             string `json:"payload_type"`
	ProjectID               string `json:"project_id"`
	Milestone               string `json:"milestone"`
	BatchID                 string `json:"batch_id"`
	Verdict                 string `json:"verdict"`
	AcceptanceCandidateSHA  string `json:"acceptance_candidate_sha"`
	AcceptanceCandidateTree string `json:"acceptance_candidate_tree"`
	BlockingCommitSHA       string `json:"blocking_commit_sha"`
	FrozenContractSHA256    string `json:"frozen_contract_sha256"`
	RouteBindingSHA256      string `json:"route_binding_sha256"`
	FindingSetDigest        string `json:"finding_set_digest"`
	EvidenceDigest          string `json:"evidence_digest"`
	CreatedAt               string `json:"created_at"`
}

func (a *App) codexAcceptanceCommand(ctx context.Context, args []string) error {
	if len(args) == 3 && args[0] == "record" && args[1] == "--input" && strings.TrimSpace(args[2]) != "" {
		return a.recordNativeAcceptance(ctx, args[2], time.Now())
	}
	if len(args) == 3 && args[0] == "verify" && args[1] == "--input" && strings.TrimSpace(args[2]) != "" {
		payload, err := readNativeAcceptancePayload(args[2])
		if err != nil {
			return err
		}
		if err := a.verifyNativeAcceptance(ctx, payload); err != nil {
			return err
		}
		fmt.Fprintf(a.stdout, "[PASS] native acceptance payload %s is valid and bound to current native ACCEPT authority\n", payload.NativeAcceptanceIdentity)
		return nil
	}
	return usageError("acceptance record|verify --input FILE")
}

func (a *App) recordNativeAcceptance(ctx context.Context, inputPath string, now time.Time) error {
	if err := a.requireCleanWorktree(ctx); err != nil {
		return err
	}
	if err := a.checkCodex(ctx); err != nil {
		return fmt.Errorf("native ACCEPT route validation: %w", err)
	}
	route, err := loadYAML[readingMap](a.root, ".codex/runtime/READING_MAP.yaml")
	if err != nil {
		return err
	}
	if route.Mode != "ACCEPT" || route.RouteScope != batchRouteScope || route.Milestone == "" || route.BatchID == "" || route.MaintenanceID != "" {
		return errors.New("NATIVE_ACCEPTANCE_ROUTE_REQUIRED: a current ACCEPT BATCH route is required")
	}
	head, err := a.capture(ctx, "git", "rev-parse", "HEAD")
	if err != nil {
		return err
	}
	head = strings.TrimSpace(head)
	if route.SourceCommit != head {
		return fmt.Errorf("NATIVE_ACCEPTANCE_ROUTE_STALE: route source %s differs from HEAD %s", route.SourceCommit, head)
	}
	tree, err := a.capture(ctx, "git", "rev-parse", "HEAD^{tree}")
	if err != nil {
		return err
	}
	tree = strings.TrimSpace(tree)
	if route.SourceTree != tree {
		return fmt.Errorf("NATIVE_ACCEPTANCE_ROUTE_STALE: route tree %s differs from HEAD tree %s", route.SourceTree, tree)
	}

	plan, err := loadYAML[milestonePlan](a.root, ".codex/state/MILESTONE_PLAN.yaml")
	if err != nil {
		return err
	}
	catalog, err := a.loadV1MilestoneCatalog()
	if err != nil {
		return err
	}
	if err := validateMilestonePlan(plan, catalog); err != nil {
		return fmt.Errorf("native acceptance milestone plan: %w", err)
	}
	batch, err := nativeAcceptanceBatch(plan, route.Milestone, route.BatchID, "VERIFYING")
	if err != nil {
		return err
	}
	input, err := readNativeAcceptanceRecordInput(inputPath)
	if err != nil {
		return err
	}
	if err := validateNativeAcceptanceInput(input, route.BatchID); err != nil {
		return err
	}
	if err := a.validateNativeBlockingCommit(ctx, head, input.BlockingCommitSHA, route.Milestone, route.BatchID, batch.FrozenContractSHA256, catalog); err != nil {
		return err
	}
	projectID, err := nativeProjectID(a.root)
	if err != nil {
		return err
	}
	payload, err := buildNativeAcceptancePayload(input, projectID, route, tree, batch.FrozenContractSHA256, now)
	if err != nil {
		return err
	}
	if err := validateNativeAcceptancePayload(payload); err != nil {
		return err
	}
	data, err := json.MarshalIndent(payload, "", "  ")
	if err != nil {
		return fmt.Errorf("encode native acceptance payload: %w", err)
	}
	data = append(data, '\n')
	outputPath := filepath.Join(a.root, filepath.FromSlash(nativeFindingOutputRelative))
	if existing, readErr := os.ReadFile(outputPath); readErr == nil {
		if !bytes.Equal(existing, data) {
			return errors.New("NATIVE_ACCEPTANCE_OUTPUT_CONFLICT: existing immutable payload differs")
		}
		fmt.Fprintf(a.stdout, "[PASS] native acceptance payload already persisted at %s (%s)\n", nativeFindingOutputRelative, payload.NativeAcceptanceIdentity)
		return nil
	} else if !errors.Is(readErr, os.ErrNotExist) {
		return fmt.Errorf("read native acceptance output: %w", readErr)
	}
	if err := writeNativeAcceptanceAtomically(outputPath, data, 0o600); err != nil {
		return fmt.Errorf("persist native acceptance payload: %w", err)
	}
	fmt.Fprintf(a.stdout, "[NATIVE ACCEPTANCE] %s -> %s\n", payload.NativeAcceptanceIdentity, nativeFindingOutputRelative)
	return nil
}

func (a *App) verifyNativeAcceptance(ctx context.Context, payload nativeAcceptancePayload) error {
	if err := validateNativeAcceptancePayload(payload); err != nil {
		return err
	}
	if err := a.requireCleanWorktree(ctx); err != nil {
		return err
	}
	if err := a.checkCodex(ctx); err != nil {
		return fmt.Errorf("native ACCEPT route validation: %w", err)
	}
	route, err := loadYAML[readingMap](a.root, ".codex/runtime/READING_MAP.yaml")
	if err != nil {
		return err
	}
	if route.Mode != "ACCEPT" || route.RouteScope != batchRouteScope || route.Milestone == "" || route.BatchID == "" || route.MaintenanceID != "" {
		return errors.New("NATIVE_ACCEPTANCE_ROUTE_REQUIRED: a current ACCEPT BATCH route is required")
	}
	head, err := a.capture(ctx, "git", "rev-parse", "HEAD")
	if err != nil {
		return err
	}
	head = strings.TrimSpace(head)
	tree, err := a.capture(ctx, "git", "rev-parse", "HEAD^{tree}")
	if err != nil {
		return err
	}
	tree = strings.TrimSpace(tree)
	plan, err := loadYAML[milestonePlan](a.root, ".codex/state/MILESTONE_PLAN.yaml")
	if err != nil {
		return err
	}
	catalog, err := a.loadV1MilestoneCatalog()
	if err != nil {
		return err
	}
	if err := validateMilestonePlan(plan, catalog); err != nil {
		return fmt.Errorf("native acceptance milestone plan: %w", err)
	}
	batch, err := nativeAcceptanceBatch(plan, route.Milestone, route.BatchID, "VERIFYING")
	if err != nil {
		return err
	}
	projectID, err := nativeProjectID(a.root)
	if err != nil {
		return err
	}
	bindings := []struct {
		name string
		got  string
		want string
	}{
		{name: "project_id", got: payload.ProjectID, want: projectID},
		{name: "milestone", got: payload.Milestone, want: route.Milestone},
		{name: "batch_id", got: payload.BatchID, want: route.BatchID},
		{name: "acceptance_candidate_sha", got: payload.AcceptanceCandidateSHA, want: head},
		{name: "acceptance_candidate_tree", got: payload.AcceptanceCandidateTree, want: tree},
		{name: "frozen_contract_sha256", got: payload.FrozenContractSHA256, want: batch.FrozenContractSHA256},
		{name: "route_binding_sha256", got: payload.RouteBindingSHA256, want: route.RouteBindingSHA256},
	}
	for _, binding := range bindings {
		if binding.got != binding.want {
			return fmt.Errorf("NATIVE_ACCEPTANCE_BINDING_MISMATCH: %s=%s, current native authority requires %s", binding.name, binding.got, binding.want)
		}
	}
	if err := a.validateNativeBlockingCommit(ctx, head, payload.BlockingCommitSHA, route.Milestone, route.BatchID, batch.FrozenContractSHA256, catalog); err != nil {
		return fmt.Errorf("NATIVE_ACCEPTANCE_BLOCKING_BINDING_INVALID: %w", err)
	}
	return nil
}

func readNativeAcceptanceRecordInput(path string) (nativeAcceptanceRecordInput, error) {
	var input nativeAcceptanceRecordInput
	if err := decodeStrictJSONFile(path, &input); err != nil {
		return input, fmt.Errorf("read native acceptance input: %w", err)
	}
	return input, nil
}

func readNativeAcceptancePayload(path string) (nativeAcceptancePayload, error) {
	var payload nativeAcceptancePayload
	if err := decodeStrictJSONFile(path, &payload); err != nil {
		return payload, fmt.Errorf("read native acceptance payload: %w", err)
	}
	return payload, nil
}

func decodeStrictJSONFile(path string, target any) error {
	file, err := os.Open(path)
	if err != nil {
		return err
	}
	defer file.Close()
	decoder := json.NewDecoder(file)
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(target); err != nil {
		return err
	}
	var trailing any
	if err := decoder.Decode(&trailing); !errors.Is(err, io.EOF) {
		if err == nil {
			return errors.New("multiple JSON values are not allowed")
		}
		return err
	}
	return nil
}

func validateNativeAcceptanceInput(input nativeAcceptanceRecordInput, batchID string) error {
	if input.SchemaVersion != nativeFindingPayloadSchemaVersion {
		return fmt.Errorf("native acceptance input schema_version=%d, want %d", input.SchemaVersion, nativeFindingPayloadSchemaVersion)
	}
	if !commitPattern.MatchString(input.BlockingCommitSHA) {
		return errors.New("native acceptance input requires a 40-character lowercase blocking_commit_sha")
	}
	if input.Verdict != "PASS" && input.Verdict != "FAIL" && input.Verdict != "BLOCKED" {
		return fmt.Errorf("invalid native acceptance verdict %q", input.Verdict)
	}
	if input.Verdict == "PASS" && len(input.Findings) != 0 {
		return errors.New("PASS native acceptance must not contain findings")
	}
	if input.Verdict == "FAIL" && len(input.Findings) == 0 {
		return errors.New("FAIL native acceptance requires at least one finding")
	}
	if input.Findings == nil {
		return errors.New("native acceptance findings must be an array")
	}
	if len(input.Evidence) == 0 {
		return errors.New("native acceptance evidence must be a non-empty array")
	}
	evidenceIDs := map[string]bool{}
	for index, evidence := range input.Evidence {
		if err := validateNativeEvidence(evidence); err != nil {
			return fmt.Errorf("evidence[%d]: %w", index, err)
		}
		if evidenceIDs[evidence.EvidenceID] {
			return fmt.Errorf("duplicate evidence_id %s", evidence.EvidenceID)
		}
		evidenceIDs[evidence.EvidenceID] = true
	}
	findingIDs := map[string]bool{}
	for index, finding := range input.Findings {
		if err := validateNativeFindingInput(finding, batchID, evidenceIDs); err != nil {
			return fmt.Errorf("REPAIR_CONTEXT_INCOMPLETE: findings[%d]: %w", index, err)
		}
		if findingIDs[finding.FindingID] {
			return fmt.Errorf("duplicate finding_id %s", finding.FindingID)
		}
		findingIDs[finding.FindingID] = true
	}
	return nil
}

func validateNativeEvidence(evidence nativeAcceptanceEvidence) error {
	if !nativeEvidenceIDPattern.MatchString(evidence.EvidenceID) {
		return fmt.Errorf("invalid evidence_id %q", evidence.EvidenceID)
	}
	if strings.TrimSpace(evidence.Kind) == "" || strings.TrimSpace(evidence.Reference) == "" || strings.TrimSpace(evidence.Result) == "" {
		return errors.New("kind, reference, and result are required")
	}
	if evidence.Kind == "command" && (strings.TrimSpace(evidence.Command) == "" || evidence.ExitCode == nil) {
		return errors.New("command evidence requires command and exit_code")
	}
	if evidence.SHA256 != "" && !isSHA256(evidence.SHA256) {
		return errors.New("evidence sha256 must be 64 lowercase hexadecimal characters")
	}
	return nil
}

func validateNativeFindingInput(finding nativeFindingInput, batchID string, evidenceIDs map[string]bool) error {
	match := nativeFindingIDPattern.FindStringSubmatch(finding.FindingID)
	if match == nil || match[1] != batchID {
		return fmt.Errorf("finding_id %q is not bound to batch %s", finding.FindingID, batchID)
	}
	if !nativeSeverityPattern.MatchString(finding.Severity) {
		return fmt.Errorf("invalid severity %q", finding.Severity)
	}
	required := map[string]string{
		"location":             finding.Location,
		"reproduction":         finding.Reproduction,
		"violated_requirement": finding.ViolatedRequirement,
		"expected":             finding.Expected,
		"actual":               finding.Actual,
		"repair_boundary":      finding.RepairBoundary,
	}
	for field, value := range required {
		if strings.TrimSpace(value) == "" {
			return fmt.Errorf("%s is required", field)
		}
	}
	if len(finding.EvidenceRefs) == 0 {
		return errors.New("evidence_refs must be non-empty")
	}
	seen := map[string]bool{}
	for _, reference := range finding.EvidenceRefs {
		if !evidenceIDs[reference] {
			return fmt.Errorf("evidence_ref %q has no matching evidence", reference)
		}
		if seen[reference] {
			return fmt.Errorf("duplicate evidence_ref %q", reference)
		}
		seen[reference] = true
	}
	return nil
}

func nativeAcceptanceBatch(plan milestonePlan, milestone, batchID, requiredState string) (milestoneBatch, error) {
	if plan.Milestone != milestone {
		return milestoneBatch{}, fmt.Errorf("native route milestone %s differs from plan milestone %s", milestone, plan.Milestone)
	}
	for _, batch := range plan.Batches {
		if batch.BatchID != batchID {
			continue
		}
		if batch.State != requiredState {
			return milestoneBatch{}, fmt.Errorf("native acceptance batch %s state=%s, require %s", batchID, batch.State, requiredState)
		}
		if !isSHA256(batch.FrozenContractSHA256) {
			return milestoneBatch{}, fmt.Errorf("native acceptance batch %s has no valid frozen contract", batchID)
		}
		return batch, nil
	}
	return milestoneBatch{}, fmt.Errorf("native acceptance batch %s is not allocated", batchID)
}

func (a *App) validateNativeBlockingCommit(ctx context.Context, candidate, blocking, milestone, batchID, frozenContract string, catalog v1MilestoneCatalog) error {
	if candidate == blocking {
		return errors.New("blocking commit must differ from the VERIFYING candidate")
	}
	if _, err := a.capture(ctx, "git", "cat-file", "-e", blocking+"^{commit}"); err != nil {
		return fmt.Errorf("blocking commit is unavailable: %w", err)
	}
	if _, err := a.capture(ctx, "git", "merge-base", "--is-ancestor", candidate, blocking); err != nil {
		return fmt.Errorf("blocking commit %s is not a descendant of candidate %s", blocking, candidate)
	}
	data, err := a.capture(ctx, "git", "show", blocking+":.codex/state/MILESTONE_PLAN.yaml")
	if err != nil {
		return fmt.Errorf("read blocking milestone plan: %w", err)
	}
	var plan milestonePlan
	decoder := yaml.NewDecoder(strings.NewReader(data))
	decoder.KnownFields(true)
	if err := decoder.Decode(&plan); err != nil {
		return fmt.Errorf("parse blocking milestone plan: %w", err)
	}
	if err := validateMilestonePlan(plan, catalog); err != nil {
		return fmt.Errorf("blocking milestone plan: %w", err)
	}
	batch, err := nativeAcceptanceBatch(plan, milestone, batchID, "BLOCKED")
	if err != nil {
		return err
	}
	if batch.FrozenContractSHA256 != frozenContract {
		return errors.New("blocking commit frozen contract differs from the acceptance candidate")
	}
	return nil
}

func nativeProjectID(root string) (string, error) {
	data, err := os.ReadFile(filepath.Join(root, "go.mod"))
	if err != nil {
		return "", fmt.Errorf("read go.mod project identity: %w", err)
	}
	for _, line := range strings.Split(string(data), "\n") {
		fields := strings.Fields(line)
		if len(fields) == 2 && fields[0] == "module" && strings.TrimSpace(fields[1]) != "" {
			return fields[1], nil
		}
	}
	return "", errors.New("go.mod has no module project identity")
}

func buildNativeAcceptancePayload(input nativeAcceptanceRecordInput, projectID string, route readingMap, tree, frozenContract string, now time.Time) (nativeAcceptancePayload, error) {
	createdAt := now.UTC().Format(time.RFC3339Nano)
	evidence := make([]nativeAcceptanceEvidence, len(input.Evidence))
	copy(evidence, input.Evidence)
	sort.Slice(evidence, func(i, j int) bool { return evidence[i].EvidenceID < evidence[j].EvidenceID })
	findings := make([]nativeFindingPayload, 0, len(input.Findings))
	for _, finding := range input.Findings {
		refs := append([]string(nil), finding.EvidenceRefs...)
		sort.Strings(refs)
		findings = append(findings, nativeFindingPayload{
			FindingID: finding.FindingID, Severity: finding.Severity, Location: finding.Location,
			Reproduction: finding.Reproduction, ViolatedRequirement: finding.ViolatedRequirement,
			Expected: finding.Expected, Actual: finding.Actual, RepairBoundary: finding.RepairBoundary,
			EvidenceRefs: refs, AcceptanceCandidateSHA: route.SourceCommit, CreatedAt: createdAt,
		})
	}
	sort.Slice(findings, func(i, j int) bool { return findings[i].FindingID < findings[j].FindingID })
	findingDigest, err := digestJSON(findings)
	if err != nil {
		return nativeAcceptancePayload{}, err
	}
	evidenceDigest, err := digestJSON(evidence)
	if err != nil {
		return nativeAcceptancePayload{}, err
	}
	payload := nativeAcceptancePayload{
		SchemaVersion: nativeFindingPayloadSchemaVersion, PayloadType: nativeFindingPayloadType,
		ProjectID: projectID, Milestone: route.Milestone, BatchID: route.BatchID, Verdict: input.Verdict,
		AcceptanceCandidateSHA: route.SourceCommit, AcceptanceCandidateTree: tree, BlockingCommitSHA: input.BlockingCommitSHA,
		FrozenContractSHA256: frozenContract, RouteBindingSHA256: route.RouteBindingSHA256,
		FindingSetDigest: findingDigest, EvidenceDigest: evidenceDigest, Findings: findings, Evidence: evidence, CreatedAt: createdAt,
	}
	identity, err := nativeAcceptanceIdentity(payload)
	if err != nil {
		return nativeAcceptancePayload{}, err
	}
	payload.NativeAcceptanceIdentity = identity
	for index := range payload.Findings {
		payload.Findings[index].AcceptanceIdentity = identity
	}
	return payload, nil
}

func validateNativeAcceptancePayload(payload nativeAcceptancePayload) error {
	if payload.SchemaVersion != nativeFindingPayloadSchemaVersion || payload.PayloadType != nativeFindingPayloadType {
		return errors.New("native finding payload schema identity is invalid")
	}
	if strings.TrimSpace(payload.ProjectID) == "" || !milestonePattern.MatchString(payload.Milestone) || !batchPattern.MatchString(payload.BatchID) {
		return errors.New("native finding payload project/milestone/batch identity is invalid")
	}
	if payload.Verdict != "PASS" && payload.Verdict != "FAIL" && payload.Verdict != "BLOCKED" {
		return errors.New("native finding payload verdict is invalid")
	}
	if !commitPattern.MatchString(payload.AcceptanceCandidateSHA) || !commitPattern.MatchString(payload.AcceptanceCandidateTree) || !commitPattern.MatchString(payload.BlockingCommitSHA) || !isSHA256(payload.FrozenContractSHA256) || !isSHA256(payload.RouteBindingSHA256) {
		return errors.New("native finding payload commit, tree, contract, or route binding is invalid")
	}
	if _, err := time.Parse(time.RFC3339Nano, payload.CreatedAt); err != nil {
		return errors.New("native finding payload created_at is invalid")
	}
	if payload.Findings == nil {
		return errors.New("native finding payload findings must be an array")
	}
	if len(payload.Evidence) == 0 {
		return errors.New("native finding payload evidence must be a non-empty array")
	}
	evidenceIDs := map[string]bool{}
	for index, evidence := range payload.Evidence {
		if index > 0 && payload.Evidence[index-1].EvidenceID >= evidence.EvidenceID {
			return errors.New("native finding payload evidence must be uniquely sorted")
		}
		if err := validateNativeEvidence(evidence); err != nil {
			return err
		}
		evidenceIDs[evidence.EvidenceID] = true
	}
	for index, finding := range payload.Findings {
		if index > 0 && payload.Findings[index-1].FindingID >= finding.FindingID {
			return errors.New("native finding payload findings must be uniquely sorted")
		}
		input := nativeFindingInput{
			FindingID: finding.FindingID, Severity: finding.Severity, Location: finding.Location,
			Reproduction: finding.Reproduction, ViolatedRequirement: finding.ViolatedRequirement,
			Expected: finding.Expected, Actual: finding.Actual, RepairBoundary: finding.RepairBoundary,
			EvidenceRefs: finding.EvidenceRefs,
		}
		if err := validateNativeFindingInput(input, payload.BatchID, evidenceIDs); err != nil {
			return fmt.Errorf("REPAIR_CONTEXT_INCOMPLETE: %w", err)
		}
		if finding.AcceptanceCandidateSHA != payload.AcceptanceCandidateSHA || finding.AcceptanceIdentity != payload.NativeAcceptanceIdentity || finding.CreatedAt != payload.CreatedAt {
			return errors.New("finding candidate, acceptance identity, or created_at binding mismatch")
		}
	}
	if payload.Verdict == "PASS" && len(payload.Findings) != 0 || payload.Verdict == "FAIL" && len(payload.Findings) == 0 {
		return errors.New("native finding payload verdict/finding cardinality mismatch")
	}
	findingsForDigest := make([]nativeFindingPayload, len(payload.Findings))
	copy(findingsForDigest, payload.Findings)
	for index := range findingsForDigest {
		findingsForDigest[index].AcceptanceIdentity = ""
	}
	wantFindingDigest, err := digestJSON(findingsForDigest)
	if err != nil {
		return err
	}
	if payload.FindingSetDigest != wantFindingDigest {
		return errors.New("native finding payload finding_set_digest mismatch")
	}
	wantEvidenceDigest, err := digestJSON(payload.Evidence)
	if err != nil {
		return err
	}
	if payload.EvidenceDigest != wantEvidenceDigest {
		return errors.New("native finding payload evidence_digest mismatch")
	}
	wantIdentity, err := nativeAcceptanceIdentity(payload)
	if err != nil {
		return err
	}
	if payload.NativeAcceptanceIdentity != wantIdentity {
		return errors.New("native finding payload acceptance identity mismatch")
	}
	return nil
}

func nativeAcceptanceIdentity(payload nativeAcceptancePayload) (string, error) {
	material := nativeAcceptanceIdentityMaterial{
		SchemaVersion: payload.SchemaVersion, PayloadType: payload.PayloadType, ProjectID: payload.ProjectID,
		Milestone: payload.Milestone, BatchID: payload.BatchID, Verdict: payload.Verdict,
		AcceptanceCandidateSHA: payload.AcceptanceCandidateSHA, AcceptanceCandidateTree: payload.AcceptanceCandidateTree,
		BlockingCommitSHA: payload.BlockingCommitSHA, FrozenContractSHA256: payload.FrozenContractSHA256,
		RouteBindingSHA256: payload.RouteBindingSHA256, FindingSetDigest: payload.FindingSetDigest,
		EvidenceDigest: payload.EvidenceDigest, CreatedAt: payload.CreatedAt,
	}
	digest, err := digestJSON(material)
	if err != nil {
		return "", err
	}
	return "native-acceptance:sha256:" + digest, nil
}

func digestJSON(value any) (string, error) {
	data, err := json.Marshal(value)
	if err != nil {
		return "", fmt.Errorf("encode digest material: %w", err)
	}
	digest := sha256.Sum256(data)
	return hex.EncodeToString(digest[:]), nil
}

func isSHA256(value string) bool {
	if len(value) != sha256.Size*2 || value != strings.ToLower(value) {
		return false
	}
	_, err := hex.DecodeString(value)
	return err == nil
}
