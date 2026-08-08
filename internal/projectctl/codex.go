// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package projectctl

import (
	"bytes"
	"context"
	"crypto/sha256"
	"encoding/base64"
	"encoding/binary"
	"encoding/hex"
	"encoding/json"
	"encoding/pem"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"regexp"
	"sort"
	"strconv"
	"strings"
	"time"

	"gopkg.in/yaml.v3"
)

const (
	readingMapSchemaVersion = 2
	defaultContextCapacity  = 131072
	contextProfileID        = "codex-utf8-bytes-v1"
	contextMeasurement      = "utf8-bytes"
	softContextRatio        = 0.55
	hardContextRatio        = 0.70
)

var (
	commitPattern = regexp.MustCompile(`^[0-9a-f]{40}$`)
	batchPattern  = regexp.MustCompile(`^(M[0-9]+)-B([0-9]{3,})$`)
	recordPattern = regexp.MustCompile(`^(\s*)- (decision_id|requirement_id|test_id):\s*["']?([^"'[:space:]]+)["']?\s*$`)
)

type codexRouteRequest struct {
	Mode                 string
	Milestone            string
	BatchID              string
	ContextCapacityBytes int
}

func defaultCodexRouteRequest(mode string) codexRouteRequest {
	return codexRouteRequest{
		Mode:                 strings.ToUpper(mode),
		Milestone:            "M0",
		BatchID:              "M0-B001",
		ContextCapacityBytes: defaultContextCapacity,
	}
}

func (a *App) codexCommand(ctx context.Context, args []string) error {
	if len(args) == 1 && args[0] == "plan" {
		return a.generateCodexRouteRequest(ctx, defaultCodexRouteRequest("PLAN"))
	}
	if len(args) == 1 && args[0] == "check" {
		return a.checkCodex(ctx)
	}
	if len(args) >= 3 && args[0] == "route" {
		request, err := parseCodexRouteRequest(args[1:])
		if err != nil {
			return err
		}
		return a.generateCodexRouteRequest(ctx, request)
	}
	return usageError("codex plan|route --mode MODE [--milestone M0] [--batch M0-B001] [--context-capacity-bytes N]|check")
}

func (a *App) generateCodexRoute(ctx context.Context, mode string) error {
	return a.generateCodexRouteRequest(ctx, defaultCodexRouteRequest(mode))
}

func parseCodexRouteRequest(args []string) (codexRouteRequest, error) {
	request := defaultCodexRouteRequest("")
	seen := map[string]bool{}
	for index := 0; index < len(args); index += 2 {
		if index+1 >= len(args) {
			return request, usageError("codex route options require values")
		}
		option, value := args[index], args[index+1]
		if seen[option] {
			return request, fmt.Errorf("duplicate Codex route option %s", option)
		}
		seen[option] = true
		switch option {
		case "--mode":
			request.Mode = strings.ToUpper(value)
		case "--milestone":
			request.Milestone = value
		case "--batch":
			request.BatchID = value
		case "--context-capacity-bytes":
			capacity, err := strconv.Atoi(value)
			if err != nil || capacity <= 0 {
				return request, fmt.Errorf("context capacity must be a positive integer, got %q", value)
			}
			request.ContextCapacityBytes = capacity
		default:
			return request, fmt.Errorf("unknown Codex route option %s", option)
		}
	}
	if request.Mode == "" {
		return request, errors.New("Codex route requires --mode")
	}
	return request, nil
}

func (a *App) generateCodexRouteRequest(ctx context.Context, request codexRouteRequest) error {
	if err := validateCodexRouteRequest(request); err != nil {
		return err
	}
	if err := a.enforceMilestonePlanMode(ctx, request.Mode); err != nil {
		return err
	}
	if err := a.requireCleanWorktree(ctx); err != nil {
		return err
	}
	if err := a.checkCodexStatic(ctx); err != nil {
		return err
	}
	commit, err := a.capture(ctx, "git", "rev-parse", "HEAD")
	if err != nil {
		return err
	}
	tree, err := a.capture(ctx, "git", "rev-parse", "HEAD^{tree}")
	if err != nil {
		return err
	}
	commit = strings.TrimSpace(commit)
	tree = strings.TrimSpace(tree)

	specs := routePaths(request.Mode)
	mapFile := readingMap{
		RouteSchemaVersion: readingMapSchemaVersion,
		Mode:               request.Mode,
		Milestone:          request.Milestone,
		BatchID:            request.BatchID,
		SourceCommit:       commit,
		SourceTree:         tree,
		GeneratedUTC:       time.Now().UTC().Format(time.RFC3339),
		MustNotBulkRead:    []string{"docs/**", "git-history", "prior-chat-transcripts"},
		ContextProfile: contextProfile{
			ProfileID:     contextProfileID,
			CapacityBytes: request.ContextCapacityBytes,
			Measurement:   contextMeasurement,
		},
	}
	if mapFile.AlwaysRead, err = a.hashRouteSections(specs.always); err != nil {
		return err
	}
	if mapFile.NormativeReferences, err = a.hashRouteSections(specs.normative); err != nil {
		return err
	}
	if mapFile.MachineContracts, err = a.hashRouteSections(specs.machine); err != nil {
		return err
	}
	if mapFile.ReadOnDemand, err = a.hashRouteSections(specs.onDemand); err != nil {
		return err
	}
	if err := applyContextBudget(&mapFile); err != nil {
		return err
	}
	mapFile.RouteBindingSHA256, err = readingMapBindingDigest(mapFile)
	if err != nil {
		return err
	}
	data, err := yaml.Marshal(mapFile)
	if err != nil {
		return fmt.Errorf("encode Codex reading map: %w", err)
	}
	runtimeDir := filepath.Join(a.root, ".codex", "runtime")
	if err := os.MkdirAll(runtimeDir, 0o755); err != nil {
		return fmt.Errorf("create Codex runtime directory: %w", err)
	}
	path := filepath.Join(runtimeDir, "READING_MAP.yaml")
	if err := atomicWrite(path, data, 0o644); err != nil {
		return fmt.Errorf("write Codex reading map: %w", err)
	}
	fmt.Fprintf(a.stdout, "[ROUTE] %s %s/%s -> .codex/runtime/READING_MAP.yaml (%s, %.4f context ratio, %s)\n",
		request.Mode, request.Milestone, request.BatchID, commit, mapFile.Budget.ActualRatio, mapFile.Budget.Status)
	return nil
}

type codexRoutePaths struct {
	always    []routeSpec
	normative []routeSpec
	machine   []routeSpec
	onDemand  []routeSpec
}

type routeSpec struct {
	path      string
	sectionID string
	kind      string
}

func route(path, sectionID, kind string) routeSpec {
	return routeSpec{path: path, sectionID: sectionID, kind: kind}
}

func routePaths(mode string) codexRoutePaths {
	routeDocumentIDs := map[string]string{
		"PLAN": "CODEX-ROUTE-PLAN", "IMPLEMENT": "CODEX-ROUTE-IMPLEMENT",
		"ACCEPT": "CODEX-ROUTE-ACCEPT", "REPAIR": "CODEX-ROUTE-REPAIR",
	}
	paths := codexRoutePaths{
		always: []routeSpec{
			route(".codex/SESSION_START.md", "CODEX-SESSION-START", "governance-entry"),
			route(".codex/routes/"+mode+".md", routeDocumentIDs[mode], "mode-policy"),
			route(".codex/state/PROJECT_SNAPSHOT.md", "CODEX-PROJECT-SNAPSHOT", "state-summary"),
			route(".codex/state/MILESTONE_STATUS.md", "CODEX-MILESTONE-STATUS", "state-summary"),
		},
		normative: []routeSpec{
			route("docs/00-governance/DOCUMENT_AUTHORITY.md", "SPEC-DOCUMENT-AUTHORITY-READING", "normative-section"),
			route("docs/00-governance/IMPLEMENTATION_GOVERNANCE.md", "SPEC-IMPLEMENTATION-GOV-MODES", "normative-section"),
			route("docs/00-governance/IMPLEMENTATION_GOVERNANCE.md", "SPEC-IMPLEMENTATION-GOV-PROGRESSIVE", "normative-section"),
			route("docs/00-governance/IMPLEMENTATION_GOVERNANCE.md", "SPEC-IMPLEMENTATION-GOV-AUTONOMY", "normative-section"),
			route("docs/00-governance/IMPLEMENTATION_GOVERNANCE.md", "SPEC-IMPLEMENTATION-GOV-BATCH", "normative-section"),
			route("docs/80-roadmap/M0_SCOPE_AND_EXIT_GATE.md", "SPEC-M0-ALLOWED", "normative-section"),
			route("docs/80-roadmap/M0_SCOPE_AND_EXIT_GATE.md", "SPEC-M0-FORBIDDEN", "normative-section"),
			route("docs/80-roadmap/M0_SCOPE_AND_EXIT_GATE.md", "SPEC-M0-EXIT", "normative-section"),
			route(".codex/state/DECISION_DIGEST.md", "CODEX-DECISION-DIGEST", "state-summary"),
		},
		machine: []routeSpec{
			route("docs/90-traceability/REQUIREMENTS.yaml", "REQ-GOV-002", "machine-contract"),
			route("docs/90-traceability/REQUIREMENTS.yaml", "REQ-GOV-003", "machine-contract"),
			route("docs/90-traceability/TEST_CATALOG.yaml", "TEST-GOV-002", "machine-contract"),
			route("docs/90-traceability/TEST_CATALOG.yaml", "TEST-GOV-003", "machine-contract"),
			route("docs/90-traceability/TRACEABILITY.yaml", "REQ-GOV-002", "machine-contract"),
			route("docs/90-traceability/TRACEABILITY.yaml", "REQ-GOV-003", "machine-contract"),
			route("schemas/codex/reading-map-v2.schema.json", "SCHEMA-CODEX-READING-MAP-V2", "machine-contract"),
			route("schemas/codex/milestone-plan-v1.schema.json", "SCHEMA-CODEX-MILESTONE-PLAN-V1", "machine-contract"),
			route(".codex/state/MILESTONE_PLAN.yaml", "M0-MILESTONE-PLAN", "machine-contract"),
		},
	}
	for _, decisionID := range []string{
		"R23-A02", "R23-A03", "R23-A05", "R23-A06", "R23-A07", "R23-A08",
		"R24-A07", "R24-A08", "R24-A10", "R24-A11", "R24-A12",
	} {
		paths.machine = append(paths.machine, route("docs/70-decisions/DECISION_REGISTER.yaml", decisionID, "machine-contract"))
	}
	switch mode {
	case "PLAN":
		paths.always = append(paths.always, route(".codex/planning/AUTONOMOUS_PLANNING_POLICY.md", "CODEX-AUTONOMOUS-PLANNING", "governance-policy"))
		paths.onDemand = []routeSpec{
			route("docs/80-roadmap/V1_MILESTONES.md", "SPEC-V1-ROADMAP-M0", "read-on-demand"),
			route("docs/00-governance/CHANGE_CONTROL.md", "SPEC-CHANGE-CONTROL-NONTRIGGERS", "read-on-demand"),
			route("docs/00-governance/CHANGE_CONTROL.md", "SPEC-CHANGE-CONTROL-FROZEN", "read-on-demand"),
		}
	case "IMPLEMENT":
		paths.onDemand = []routeSpec{
			route("docs/60-quality/TEST_STRATEGY.md", "SPEC-QUALITY-LAYERS", "read-on-demand"),
			route("docs/20-architecture/SYSTEM_ARCHITECTURE.md", "SPEC-SYSTEM-ARCH-PRINCIPLES", "read-on-demand"),
		}
	case "ACCEPT":
		paths.always = append(paths.always, route("docs/60-quality/ACCEPTANCE_POLICY.md", "SPEC-ACCEPTANCE-ORDER", "normative-section"))
		paths.onDemand = []routeSpec{
			route("docs/60-quality/ACCEPTANCE_POLICY.md", "SPEC-ACCEPTANCE-EVIDENCE", "read-on-demand"),
			route("docs/60-quality/COMPATIBILITY_AND_RELEASE_GATE.md", "SPEC-RELEASE-GATE-BLOCK", "read-on-demand"),
		}
	case "REPAIR":
		paths.onDemand = []routeSpec{
			route("docs/00-governance/CHANGE_CONTROL.md", "SPEC-CHANGE-CONTROL-TRIGGERS", "read-on-demand"),
			route("docs/00-governance/CHANGE_CONTROL.md", "SPEC-CHANGE-CONTROL-FAIL", "read-on-demand"),
			route("docs/60-quality/ACCEPTANCE_POLICY.md", "SPEC-ACCEPTANCE-FINDINGS", "read-on-demand"),
		}
	}
	return paths
}

func validateCodexRouteRequest(request codexRouteRequest) error {
	if !map[string]bool{"PLAN": true, "IMPLEMENT": true, "ACCEPT": true, "REPAIR": true}[request.Mode] {
		return fmt.Errorf("invalid Codex mode %q", request.Mode)
	}
	if request.Milestone != "M0" {
		return fmt.Errorf("M0 control plane cannot route milestone %q", request.Milestone)
	}
	if err := validateBatchID(request.Milestone, request.BatchID); err != nil {
		return err
	}
	if request.ContextCapacityBytes <= 0 {
		return errors.New("context capacity must be positive")
	}
	return nil
}

func validateBatchID(milestone, batchID string) error {
	match := batchPattern.FindStringSubmatch(batchID)
	if match == nil || match[1] != milestone {
		return fmt.Errorf("batch_id %q is not bound to milestone %s", batchID, milestone)
	}
	return nil
}

func (a *App) hashRouteSections(specs []routeSpec) ([]routeSection, error) {
	sections := make([]routeSection, 0, len(specs))
	for _, spec := range specs {
		data, err := os.ReadFile(filepath.Join(a.root, filepath.FromSlash(spec.path)))
		if err != nil {
			return nil, fmt.Errorf("route source %s: %w", spec.path, err)
		}
		material, err := sectionMaterial(data, spec.sectionID)
		if err != nil {
			return nil, fmt.Errorf("route source %s section %s: %w", spec.path, spec.sectionID, err)
		}
		fileDigest := sha256.Sum256(data)
		sectionDigest := sha256.Sum256(material)
		sections = append(sections, routeSection{
			Path: spec.path, SectionID: spec.sectionID, Kind: spec.kind,
			SHA256: hex.EncodeToString(fileDigest[:]), SectionSHA256: hex.EncodeToString(sectionDigest[:]), MaterialBytes: len(material),
		})
	}
	return sections, nil
}

type sectionMarker struct {
	id    string
	kind  string
	group string
	line  int
}

func sectionMaterial(data []byte, sectionID string) ([]byte, error) {
	normalized := bytes.ReplaceAll(data, []byte("\r\n"), []byte("\n"))
	lines := strings.SplitAfter(string(normalized), "\n")
	markers := make([]sectionMarker, 0)
	for index, withEnding := range lines {
		line := strings.TrimSpace(strings.TrimSuffix(withEnding, "\n"))
		if strings.HasPrefix(line, `<a id="`) && strings.HasSuffix(line, `"></a>`) {
			id := strings.TrimSuffix(strings.TrimPrefix(line, `<a id="`), `"></a>`)
			markers = append(markers, sectionMarker{id: id, kind: "anchor", group: "anchor", line: index})
			continue
		}
		if match := recordPattern.FindStringSubmatch(strings.TrimSuffix(withEnding, "\n")); match != nil {
			markers = append(markers, sectionMarker{id: match[3], kind: "record", group: match[1] + match[2], line: index})
			continue
		}
		for _, key := range []string{"document_id:", "plan_id:", "x-section-id:"} {
			if strings.HasPrefix(line, key) {
				value := strings.Trim(strings.TrimSpace(strings.TrimPrefix(line, key)), `"'`)
				markers = append(markers, sectionMarker{id: value, kind: "root", group: key, line: index})
			}
		}
		if strings.HasPrefix(line, `"x-section-id"`) {
			parts := strings.SplitN(line, ":", 2)
			if len(parts) == 2 {
				value := strings.Trim(strings.TrimSuffix(strings.TrimSpace(parts[1]), ","), `"'`)
				markers = append(markers, sectionMarker{id: value, kind: "root", group: "x-section-id", line: index})
			}
		}
	}
	var matches []sectionMarker
	for _, marker := range markers {
		if marker.id == sectionID {
			matches = append(matches, marker)
		}
	}
	if len(matches) != 1 {
		return nil, fmt.Errorf("stable Section ID must exist exactly once, found %d", len(matches))
	}
	target := matches[0]
	if target.kind == "root" {
		return normalized, nil
	}
	end := len(lines)
	for _, marker := range markers {
		if marker.line <= target.line {
			continue
		}
		if target.kind == "anchor" && marker.kind == "anchor" || target.kind == "record" && marker.kind == "record" && marker.group == target.group {
			end = marker.line
			break
		}
	}
	material := []byte(strings.Join(lines[target.line:end], ""))
	if len(bytes.TrimSpace(material)) == 0 {
		return nil, errors.New("stable Section ID resolves to empty material")
	}
	return material, nil
}

func applyContextBudget(route *readingMap) error {
	capacity := route.ContextProfile.CapacityBytes
	if capacity <= 0 {
		return errors.New("context capacity must be positive")
	}
	initial := measureRouteSections(route.AlwaysRead, route.NormativeReferences, route.MachineContracts, route.ReadOnDemand)
	initialRatio := float64(initial) / float64(capacity)
	if initialRatio > hardContextRatio {
		return fmt.Errorf("Codex route context hard limit exceeded: %.4f > %.2f; hard stop, return to PLAN and split the batch", initialRatio, hardContextRatio)
	}
	route.Budget = contextBudget{
		InitialMaterialBytes: initial,
		MaterialBytes:        initial,
		CapacityBytes:        capacity,
		InitialRatio:         initialRatio,
		ActualRatio:          initialRatio,
		SoftLimitRatio:       softContextRatio,
		HardLimitRatio:       hardContextRatio,
		Status:               "WITHIN_BUDGET",
	}
	if initialRatio <= softContextRatio {
		return nil
	}
	for len(route.ReadOnDemand) > 0 && route.Budget.ActualRatio > softContextRatio {
		last := len(route.ReadOnDemand) - 1
		omitted := route.ReadOnDemand[last]
		route.ReadOnDemand = route.ReadOnDemand[:last]
		route.SoftLimitOmissions = append(route.SoftLimitOmissions, omitted)
		route.Budget.MaterialBytes -= omitted.MaterialBytes
		route.Budget.ActualRatio = float64(route.Budget.MaterialBytes) / float64(capacity)
	}
	route.Budget.ReductionApplied = len(route.SoftLimitOmissions) > 0
	if route.Budget.ActualRatio <= softContextRatio {
		route.Budget.Status = "SOFT_LIMIT_REDUCED"
	} else {
		route.Budget.Status = "SOFT_LIMIT_EXCEEDED_REVIEW_REQUIRED"
	}
	return nil
}

func measureRouteSections(groups ...[]routeSection) int {
	total := 0
	for _, group := range groups {
		for _, section := range group {
			total += section.MaterialBytes
		}
	}
	return total
}

func readingMapBindingDigest(route readingMap) (string, error) {
	payload := struct {
		RouteSchemaVersion  int            `json:"route_schema_version"`
		Mode                string         `json:"mode"`
		Milestone           string         `json:"milestone"`
		BatchID             string         `json:"batch_id"`
		SourceCommit        string         `json:"source_commit"`
		SourceTree          string         `json:"source_tree"`
		ContextProfile      contextProfile `json:"context_profile"`
		Budget              contextBudget  `json:"budget"`
		AlwaysRead          []routeSection `json:"always_read"`
		NormativeReferences []routeSection `json:"normative_references"`
		MachineContracts    []routeSection `json:"machine_contracts"`
		ReadOnDemand        []routeSection `json:"read_on_demand"`
		SoftLimitOmissions  []routeSection `json:"soft_limit_omissions"`
		MustNotBulkRead     []string       `json:"must_not_bulk_read"`
	}{
		RouteSchemaVersion: route.RouteSchemaVersion, Mode: route.Mode, Milestone: route.Milestone, BatchID: route.BatchID,
		SourceCommit: route.SourceCommit, SourceTree: route.SourceTree, ContextProfile: route.ContextProfile, Budget: route.Budget,
		AlwaysRead: route.AlwaysRead, NormativeReferences: route.NormativeReferences, MachineContracts: route.MachineContracts,
		ReadOnDemand: route.ReadOnDemand, SoftLimitOmissions: route.SoftLimitOmissions, MustNotBulkRead: route.MustNotBulkRead,
	}
	data, err := json.Marshal(payload)
	if err != nil {
		return "", fmt.Errorf("encode reading-map binding: %w", err)
	}
	digest := sha256.Sum256(data)
	return hex.EncodeToString(digest[:]), nil
}

func (a *App) requireCleanWorktree(ctx context.Context) error {
	output, err := a.capture(ctx, "git", "status", "--porcelain", "--untracked-files=all")
	if err != nil {
		return err
	}
	var dirty []string
	for _, line := range strings.Split(strings.TrimSpace(output), "\n") {
		if line == "" {
			continue
		}
		path := strings.TrimSpace(line[2:])
		if path == ".codex/runtime" || strings.HasPrefix(filepath.ToSlash(path), ".codex/runtime/") {
			continue
		}
		dirty = append(dirty, line)
	}
	if len(dirty) != 0 {
		return fmt.Errorf("Codex routes require a clean worktree outside .codex/runtime:\n%s", strings.Join(dirty, "\n"))
	}
	return nil
}

func (a *App) checkCodex(ctx context.Context) error {
	if err := a.checkCodexStatic(ctx); err != nil {
		return err
	}
	runtimePath := filepath.Join(a.root, ".codex", "runtime", "READING_MAP.yaml")
	if _, err := os.Stat(runtimePath); errors.Is(err, os.ErrNotExist) {
		fmt.Fprintln(a.stdout, "[PASS] Codex governance (no runtime route present)")
		return nil
	} else if err != nil {
		return fmt.Errorf("stat Codex runtime route: %w", err)
	}
	route, err := loadYAML[readingMap](a.root, ".codex/runtime/READING_MAP.yaml")
	if err != nil {
		return err
	}
	if err := validateReadingMapMetadata(route); err != nil {
		return err
	}
	commit, err := a.capture(ctx, "git", "rev-parse", "HEAD")
	if err != nil {
		return err
	}
	if route.SourceCommit != strings.TrimSpace(commit) {
		return errors.New("Codex runtime route is stale: source commit differs from HEAD; regenerate it")
	}
	tree, err := a.capture(ctx, "git", "rev-parse", "HEAD^{tree}")
	if err != nil {
		return err
	}
	if route.SourceTree != strings.TrimSpace(tree) {
		return errors.New("Codex runtime route is stale: source tree differs from HEAD; regenerate it")
	}
	seen := map[string]bool{}
	for _, section := range allReadingMapSections(route) {
		if err := validateBoundedRoutePath(section.Path); err != nil {
			return err
		}
		key := section.Path + "#" + section.SectionID
		if seen[key] {
			return fmt.Errorf("Codex runtime route repeats stable Section ID reference %s", key)
		}
		seen[key] = true
		data, err := os.ReadFile(filepath.Join(a.root, filepath.FromSlash(section.Path)))
		if err != nil {
			return fmt.Errorf("Codex route source %s: %w", section.Path, err)
		}
		digest := sha256.Sum256(data)
		if hex.EncodeToString(digest[:]) != section.SHA256 {
			return fmt.Errorf("Codex runtime route is stale: %s changed; regenerate it", section.Path)
		}
		material, err := sectionMaterial(data, section.SectionID)
		if err != nil {
			return fmt.Errorf("Codex route Section ID %s#%s: %w", section.Path, section.SectionID, err)
		}
		sectionDigest := sha256.Sum256(material)
		if hex.EncodeToString(sectionDigest[:]) != section.SectionSHA256 || len(material) != section.MaterialBytes {
			return fmt.Errorf("Codex runtime route is stale: section %s#%s changed; regenerate it", section.Path, section.SectionID)
		}
	}
	fmt.Fprintf(a.stdout, "[PASS] Codex %s route for %s is current at %s (%.4f context ratio, %s)\n",
		route.Mode, route.BatchID, route.SourceCommit, route.Budget.ActualRatio, route.Budget.Status)
	return nil
}

func validateReadingMapMetadata(route readingMap) error {
	if route.RouteSchemaVersion != readingMapSchemaVersion {
		return errors.New("Codex runtime route schema version is invalid")
	}
	if err := validateCodexRouteRequest(codexRouteRequest{
		Mode: route.Mode, Milestone: route.Milestone, BatchID: route.BatchID, ContextCapacityBytes: route.ContextProfile.CapacityBytes,
	}); err != nil {
		return err
	}
	if route.ContextProfile.ProfileID != contextProfileID || route.ContextProfile.Measurement != contextMeasurement {
		return errors.New("Codex runtime route context profile is invalid")
	}
	for _, forbidden := range []string{"docs/**", "git-history", "prior-chat-transcripts"} {
		found := false
		for _, value := range route.MustNotBulkRead {
			if value == forbidden {
				found = true
			}
		}
		if !found {
			return fmt.Errorf("Codex runtime route lacks forbidden bulk-read rule %q", forbidden)
		}
	}
	if len(route.AlwaysRead) == 0 || len(route.NormativeReferences) == 0 || len(route.MachineContracts) == 0 {
		return errors.New("Codex runtime route has an empty required disclosure layer")
	}
	for _, section := range route.MachineContracts {
		if section.Kind != "machine-contract" {
			return fmt.Errorf("machine contract %s#%s has kind %q", section.Path, section.SectionID, section.Kind)
		}
	}
	binding, err := readingMapBindingDigest(route)
	if err != nil {
		return err
	}
	if route.RouteBindingSHA256 != binding {
		return errors.New("Codex runtime route batch/source/section binding hash is invalid")
	}
	return validateContextBudget(route)
}

func validateContextBudget(route readingMap) error {
	current := measureRouteSections(route.AlwaysRead, route.NormativeReferences, route.MachineContracts, route.ReadOnDemand)
	initial := current + measureRouteSections(route.SoftLimitOmissions)
	capacity := route.ContextProfile.CapacityBytes
	if capacity <= 0 {
		return errors.New("Codex runtime route context capacity is invalid")
	}
	initialRatio := float64(initial) / float64(capacity)
	actualRatio := float64(current) / float64(capacity)
	budget := route.Budget
	if budget.InitialMaterialBytes != initial || budget.MaterialBytes != current || budget.CapacityBytes != capacity ||
		budget.InitialRatio != initialRatio || budget.ActualRatio != actualRatio || budget.SoftLimitRatio != softContextRatio || budget.HardLimitRatio != hardContextRatio {
		return errors.New("Codex runtime route context budget measurement is inconsistent")
	}
	if initialRatio > hardContextRatio || budget.PlanSplitRequired {
		return errors.New("Codex runtime route exceeds the hard context limit; hard stop and return PLAN split required")
	}
	switch {
	case initialRatio <= softContextRatio:
		if budget.Status != "WITHIN_BUDGET" || budget.ReductionApplied || len(route.SoftLimitOmissions) != 0 {
			return errors.New("Codex runtime route has invalid within-budget handling")
		}
	case actualRatio <= softContextRatio && len(route.SoftLimitOmissions) > 0:
		if budget.Status != "SOFT_LIMIT_REDUCED" || !budget.ReductionApplied {
			return errors.New("Codex runtime route has invalid soft-limit reduction handling")
		}
	default:
		if budget.Status != "SOFT_LIMIT_EXCEEDED_REVIEW_REQUIRED" {
			return errors.New("Codex runtime route lacks explicit soft-limit handling")
		}
	}
	return nil
}

func allReadingMapSections(route readingMap) []routeSection {
	var sections []routeSection
	sections = append(sections, route.AlwaysRead...)
	sections = append(sections, route.NormativeReferences...)
	sections = append(sections, route.MachineContracts...)
	sections = append(sections, route.ReadOnDemand...)
	sections = append(sections, route.SoftLimitOmissions...)
	return sections
}

func validateBoundedRoutePath(path string) error {
	normalized := filepath.ToSlash(path)
	if normalized == "" || filepath.IsAbs(path) || normalized == "docs" || strings.HasSuffix(normalized, "/") ||
		strings.ContainsAny(normalized, "*?[") || normalized == ".git" || strings.HasPrefix(normalized, ".git/") ||
		normalized == "git-history" || strings.Contains(normalized, "chat-transcript") ||
		normalized == ".." || strings.HasPrefix(normalized, "../") {
		return fmt.Errorf("Codex route contains forbidden unbounded read path %q", path)
	}
	return nil
}

func (a *App) checkCodexStatic(ctx context.Context) error {
	required := []string{
		".codex/README.md", ".codex/SESSION_START.md", ".codex/planning/AUTONOMOUS_PLANNING_POLICY.md",
		".codex/routes/PLAN.md", ".codex/routes/IMPLEMENT.md", ".codex/routes/ACCEPT.md", ".codex/routes/REPAIR.md",
		".codex/state/DECISION_DIGEST.md", ".codex/state/MILESTONE_PLAN.yaml", ".codex/state/MILESTONE_STATUS.md", ".codex/state/PROJECT_SNAPSHOT.md",
		".codex/templates/ACCEPT.template.md", ".codex/templates/BATCH_CONTRACT.template.md", ".codex/templates/HANDOFF.template.md",
		".codex/templates/IMPLEMENT.template.md", ".codex/templates/READING_MAP.template.yaml", ".codex/templates/REPAIR.template.md",
		"schemas/codex/reading-map-v2.schema.json", "schemas/codex/milestone-plan-v1.schema.json",
	}
	problems := &validationErrors{}
	verifiedCommits := map[string]bool{}
	for _, relative := range required {
		data, err := os.ReadFile(filepath.Join(a.root, filepath.FromSlash(relative)))
		if err != nil {
			problems.add("missing %s", relative)
			continue
		}
		if strings.HasSuffix(relative, ".md") && relative != ".codex/README.md" {
			text := string(data)
			if !strings.Contains(text, "document_id:") || !strings.Contains(text, "authority:") || !strings.Contains(text, "status:") {
				problems.add("incomplete front matter: %s", relative)
			}
			if !strings.Contains(relative, "/templates/") {
				commit := frontMatterValue(text, "source_commit")
				if !commitPattern.MatchString(commit) {
					problems.add("%s source_commit must be a full immutable SHA", relative)
				} else if !verifiedCommits[commit] {
					if err := a.verifyCodexSourceCommit(ctx, commit); err != nil {
						problems.add("%s has invalid source_commit %s: %v", relative, commit, err)
					} else {
						verifiedCommits[commit] = true
					}
				}
				if strings.Contains(text, "{{") {
					problems.add("active Codex file contains template placeholder: %s", relative)
				}
			}
		}
	}
	plan, err := loadYAML[milestonePlan](a.root, ".codex/state/MILESTONE_PLAN.yaml")
	if err != nil {
		problems.add("%v", err)
	} else if err := validateMilestonePlan(plan); err != nil {
		problems.add("invalid MILESTONE_PLAN.yaml: %v", err)
	}
	for _, relative := range []string{"schemas/codex/reading-map-v2.schema.json", "schemas/codex/milestone-plan-v1.schema.json"} {
		data, err := os.ReadFile(filepath.Join(a.root, filepath.FromSlash(relative)))
		if err == nil && !json.Valid(data) {
			problems.add("invalid JSON schema: %s", relative)
		}
	}
	return problems.err("Codex governance")
}

func (a *App) enforceMilestonePlanMode(ctx context.Context, mode string) error {
	current, err := os.ReadFile(filepath.Join(a.root, ".codex", "state", "MILESTONE_PLAN.yaml"))
	if err != nil {
		return fmt.Errorf("read MILESTONE_PLAN.yaml: %w", err)
	}
	committed, err := a.capture(ctx, "git", "show", "HEAD:.codex/state/MILESTONE_PLAN.yaml")
	if err != nil {
		return fmt.Errorf("read committed MILESTONE_PLAN.yaml: %w", err)
	}
	currentPlan, err := decodeMilestonePlan(current)
	if err != nil {
		return err
	}
	committedPlan, err := decodeMilestonePlan([]byte(committed))
	if err != nil {
		return err
	}
	return validateMilestonePlanChange(committedPlan, currentPlan, mode)
}

func decodeMilestonePlan(data []byte) (milestonePlan, error) {
	var plan milestonePlan
	decoder := yaml.NewDecoder(bytes.NewReader(data))
	decoder.KnownFields(true)
	if err := decoder.Decode(&plan); err != nil {
		return plan, fmt.Errorf("parse MILESTONE_PLAN.yaml: %w", err)
	}
	return plan, nil
}

func validateMilestonePlan(plan milestonePlan) error {
	if plan.SchemaVersion != 1 || plan.PlanID != plan.Milestone+"-MILESTONE-PLAN" || plan.ModifiableOnlyInMode != "PLAN" {
		return errors.New("plan metadata is invalid")
	}
	if plan.Milestone != "M0" {
		return errors.New("M0 control plane cannot contain an M1 plan")
	}
	if plan.NextBatchSequence < 1 {
		return errors.New("next_batch_sequence must be positive")
	}
	if plan.Status == "NOT_GENERATED" {
		if plan.PlanVersion != 0 || plan.NextBatchSequence != 1 || len(plan.Batches) != 0 || len(plan.Tombstones) != 0 {
			return errors.New("NOT_GENERATED plan must remain version 0 with no batches or tombstones")
		}
		return nil
	}
	if plan.Status != "ACTIVE" && plan.Status != "COMPLETE" {
		return fmt.Errorf("invalid plan status %q", plan.Status)
	}
	if plan.PlanVersion < 1 {
		return errors.New("generated plan must have a positive plan_version")
	}

	active := map[string]milestoneBatch{}
	allocated := map[string]bool{}
	lastSequence := 0
	for _, batch := range plan.Batches {
		sequence, err := batchSequence(plan.Milestone, batch.BatchID)
		if err != nil {
			return err
		}
		if batch.Sequence != sequence || sequence <= lastSequence || sequence >= plan.NextBatchSequence {
			return fmt.Errorf("batch %s violates monotonic sequence allocation", batch.BatchID)
		}
		lastSequence = sequence
		if allocated[batch.BatchID] {
			return fmt.Errorf("batch ID %s is reused", batch.BatchID)
		}
		allocated[batch.BatchID], active[batch.BatchID] = true, batch
		if err := validateMilestoneBatch(batch); err != nil {
			return fmt.Errorf("batch %s: %w", batch.BatchID, err)
		}
	}
	lastSequence = 0
	for _, tombstone := range plan.Tombstones {
		sequence, err := batchSequence(plan.Milestone, tombstone.BatchID)
		if err != nil {
			return err
		}
		if tombstone.Sequence != sequence || sequence <= lastSequence || sequence >= plan.NextBatchSequence {
			return fmt.Errorf("tombstone %s violates monotonic sequence allocation", tombstone.BatchID)
		}
		lastSequence = sequence
		if allocated[tombstone.BatchID] {
			return fmt.Errorf("batch ID %s is reused after tombstoning", tombstone.BatchID)
		}
		allocated[tombstone.BatchID] = true
		if err := validateTombstone(tombstone, active); err != nil {
			return fmt.Errorf("tombstone %s: %w", tombstone.BatchID, err)
		}
	}
	for _, batch := range plan.Batches {
		for _, dependency := range batch.DependsOn {
			if dependency == batch.BatchID || active[dependency].BatchID == "" {
				return fmt.Errorf("batch %s has invalid dependency %s", batch.BatchID, dependency)
			}
		}
	}
	for leftIndex, left := range plan.Batches {
		if !left.ParallelSafe {
			continue
		}
		for _, right := range plan.Batches[leftIndex+1:] {
			if !right.ParallelSafe {
				continue
			}
			if slicesOverlap(left.ParallelScopeKeys, right.ParallelScopeKeys) {
				return fmt.Errorf("parallel-safe batches %s and %s have overlapping scope keys", left.BatchID, right.BatchID)
			}
			if contains(left.DependsOn, right.BatchID) || contains(right.DependsOn, left.BatchID) {
				return fmt.Errorf("parallel-safe batches %s and %s are dependency-coupled", left.BatchID, right.BatchID)
			}
		}
	}
	return nil
}

func batchSequence(milestone, batchID string) (int, error) {
	match := batchPattern.FindStringSubmatch(batchID)
	if match == nil || match[1] != milestone {
		return 0, fmt.Errorf("batch ID %q is not allocated in milestone %s", batchID, milestone)
	}
	sequence, err := strconv.Atoi(match[2])
	if err != nil || sequence < 1 {
		return 0, fmt.Errorf("batch ID %q has invalid sequence", batchID)
	}
	return sequence, nil
}

func validateMilestoneBatch(batch milestoneBatch) error {
	validStates := map[string]bool{
		"DRAFT": true, "PLANNED": true, "FROZEN": true, "IMPLEMENTING": true,
		"VERIFYING": true, "BLOCKED": true, "COMPLETED": true,
	}
	if !validStates[batch.State] {
		return fmt.Errorf("invalid state %q", batch.State)
	}
	if strings.TrimSpace(batch.Objective) == "" {
		return errors.New("objective is required")
	}
	if batch.ParallelSafe {
		if !batch.DependencyIndependent || len(batch.ParallelScopeKeys) == 0 {
			return errors.New("parallel_safe requires dependency_independent and non-empty parallel_scope_keys")
		}
	} else if batch.DependencyIndependent || len(batch.ParallelScopeKeys) != 0 {
		return errors.New("parallel execution evidence is forbidden when parallel_safe is false")
	}
	frozen := map[string]bool{"FROZEN": true, "IMPLEMENTING": true, "VERIFYING": true, "BLOCKED": true, "COMPLETED": true}[batch.State]
	if frozen && (len(batch.Requirements) == 0 || len(batch.AllowedScope) == 0 || len(batch.ForbiddenScope) == 0 ||
		len(batch.MachineContracts) == 0 || len(batch.ReadingMapSections) == 0 || len(batch.Acceptance) == 0 ||
		len(batch.Tests) == 0 || len(batch.StopConditions) == 0) {
		return errors.New("frozen contract is missing requirements, scope, machine contracts, reading map, acceptance, tests, or stop conditions")
	}
	digest, err := batchContractDigest(batch)
	if err != nil {
		return err
	}
	if frozen && batch.FrozenContractSHA256 != digest {
		return errors.New("frozen contract hash is missing or differs from objective/scope/acceptance contract")
	}
	if !frozen && batch.FrozenContractSHA256 != "" {
		return errors.New("unfrozen batch must not claim a frozen contract hash")
	}
	return nil
}

func batchContractDigest(batch milestoneBatch) (string, error) {
	contract := struct {
		Objective             string   `json:"objective"`
		NonGoals              []string `json:"non_goals"`
		Requirements          []string `json:"requirements"`
		AllowedScope          []string `json:"allowed_scope"`
		ForbiddenScope        []string `json:"forbidden_scope"`
		MachineContracts      []string `json:"machine_contracts"`
		ReadingMapSections    []string `json:"reading_map_sections"`
		Acceptance            []string `json:"acceptance"`
		Tests                 []string `json:"tests"`
		StopConditions        []string `json:"stop_conditions"`
		DependsOn             []string `json:"depends_on"`
		ParallelSafe          bool     `json:"parallel_safe"`
		DependencyIndependent bool     `json:"dependency_independent"`
		ParallelScopeKeys     []string `json:"parallel_scope_keys"`
	}{
		Objective: batch.Objective, NonGoals: batch.NonGoals, Requirements: batch.Requirements,
		AllowedScope: batch.AllowedScope, ForbiddenScope: batch.ForbiddenScope, MachineContracts: batch.MachineContracts,
		ReadingMapSections: batch.ReadingMapSections, Acceptance: batch.Acceptance, Tests: batch.Tests, StopConditions: batch.StopConditions,
		DependsOn: batch.DependsOn, ParallelSafe: batch.ParallelSafe, DependencyIndependent: batch.DependencyIndependent,
		ParallelScopeKeys: batch.ParallelScopeKeys,
	}
	data, err := json.Marshal(contract)
	if err != nil {
		return "", err
	}
	digest := sha256.Sum256(data)
	return hex.EncodeToString(digest[:]), nil
}

func validateTombstone(tombstone batchTombstone, active map[string]milestoneBatch) error {
	if strings.TrimSpace(tombstone.Reason) == "" {
		return errors.New("reason is required")
	}
	switch tombstone.Action {
	case "CANCELLED":
		if len(tombstone.ReplacementIDs) != 0 {
			return errors.New("cancelled tombstone cannot have replacements")
		}
	case "SPLIT":
		if len(tombstone.ReplacementIDs) < 2 {
			return errors.New("split tombstone requires at least two replacements")
		}
	case "MERGED":
		if len(tombstone.ReplacementIDs) != 1 {
			return errors.New("merged tombstone requires exactly one replacement")
		}
	default:
		return fmt.Errorf("invalid action %q", tombstone.Action)
	}
	seen := map[string]bool{}
	for _, replacement := range tombstone.ReplacementIDs {
		if seen[replacement] || active[replacement].BatchID == "" {
			return fmt.Errorf("invalid replacement batch %s", replacement)
		}
		seen[replacement] = true
	}
	return nil
}

func validateMilestonePlanChange(previous, proposed milestonePlan, mode string) error {
	if err := validateMilestonePlan(previous); err != nil {
		return fmt.Errorf("previous plan: %w", err)
	}
	if err := validateMilestonePlan(proposed); err != nil {
		return fmt.Errorf("proposed plan: %w", err)
	}
	previousJSON, _ := json.Marshal(previous)
	proposedJSON, _ := json.Marshal(proposed)
	if bytes.Equal(previousJSON, proposedJSON) {
		return nil
	}
	if mode != "PLAN" {
		return fmt.Errorf("MILESTONE_PLAN.yaml is read-only in %s mode; only PLAN may modify it", mode)
	}
	if previous.Milestone != proposed.Milestone || proposed.PlanVersion != previous.PlanVersion+1 || proposed.NextBatchSequence < previous.NextBatchSequence {
		return errors.New("plan change violates milestone, version, or sequence monotonicity")
	}

	previousTombstones := make(map[string]batchTombstone, len(previous.Tombstones))
	for _, tombstone := range previous.Tombstones {
		previousTombstones[tombstone.BatchID] = tombstone
	}
	proposedTombstones := make(map[string]batchTombstone, len(proposed.Tombstones))
	for _, tombstone := range proposed.Tombstones {
		proposedTombstones[tombstone.BatchID] = tombstone
		if old, exists := previousTombstones[tombstone.BatchID]; exists {
			oldJSON, _ := json.Marshal(old)
			newJSON, _ := json.Marshal(tombstone)
			if !bytes.Equal(oldJSON, newJSON) {
				return fmt.Errorf("tombstone %s is immutable", tombstone.BatchID)
			}
		}
	}
	for id := range previousTombstones {
		if _, exists := proposedTombstones[id]; !exists {
			return fmt.Errorf("tombstone %s cannot be deleted", id)
		}
	}

	previousBatches := make(map[string]milestoneBatch, len(previous.Batches))
	for _, batch := range previous.Batches {
		previousBatches[batch.BatchID] = batch
	}
	proposedBatches := make(map[string]milestoneBatch, len(proposed.Batches))
	var newSequences []int
	for _, batch := range proposed.Batches {
		proposedBatches[batch.BatchID] = batch
		old, existed := previousBatches[batch.BatchID]
		if !existed {
			if batch.Sequence < previous.NextBatchSequence {
				return fmt.Errorf("batch ID %s reuses an allocated sequence", batch.BatchID)
			}
			newSequences = append(newSequences, batch.Sequence)
			continue
		}
		if batch.Sequence != old.Sequence || !validBatchStateTransition(old.State, batch.State) {
			return fmt.Errorf("batch %s has illegal state transition %s -> %s", batch.BatchID, old.State, batch.State)
		}
		if old.FrozenContractSHA256 != "" && batch.FrozenContractSHA256 != old.FrozenContractSHA256 {
			return fmt.Errorf("batch %s silently changed a frozen objective/scope/acceptance contract", batch.BatchID)
		}
	}
	for id, batch := range previousBatches {
		if _, retained := proposedBatches[id]; retained {
			continue
		}
		if batch.State != "DRAFT" && batch.State != "PLANNED" {
			return fmt.Errorf("started batch %s cannot be removed", id)
		}
		if _, tombstoned := proposedTombstones[id]; !tombstoned {
			return fmt.Errorf("removed batch %s requires a split/merge/cancel tombstone", id)
		}
	}
	for id := range proposedTombstones {
		if _, existed := previousTombstones[id]; existed {
			continue
		}
		if _, removed := previousBatches[id]; !removed || proposedBatches[id].BatchID != "" {
			return fmt.Errorf("new tombstone %s does not correspond to a removed batch", id)
		}
	}
	sort.Ints(newSequences)
	for index, sequence := range newSequences {
		if sequence != previous.NextBatchSequence+index {
			return errors.New("new batch IDs must be allocated contiguously from next_batch_sequence")
		}
	}
	if proposed.NextBatchSequence != previous.NextBatchSequence+len(newSequences) {
		return errors.New("next_batch_sequence does not match newly allocated batch IDs")
	}
	return nil
}

func validBatchStateTransition(from, to string) bool {
	if from == to {
		return true
	}
	allowed := map[string]map[string]bool{
		"DRAFT":        {"PLANNED": true},
		"PLANNED":      {"FROZEN": true},
		"FROZEN":       {"IMPLEMENTING": true},
		"IMPLEMENTING": {"VERIFYING": true, "BLOCKED": true},
		"BLOCKED":      {"IMPLEMENTING": true},
		"VERIFYING":    {"COMPLETED": true, "IMPLEMENTING": true, "BLOCKED": true},
		"COMPLETED":    {},
	}
	return allowed[from][to]
}

func contains(values []string, target string) bool {
	for _, value := range values {
		if value == target {
			return true
		}
	}
	return false
}

func slicesOverlap(left, right []string) bool {
	seen := make(map[string]bool, len(left))
	for _, value := range left {
		seen[value] = true
	}
	for _, value := range right {
		if seen[value] {
			return true
		}
	}
	return false
}

func (a *App) verifyCodexSourceCommit(ctx context.Context, commit string) error {
	if !commitPattern.MatchString(commit) {
		return errors.New("source commit is not a full lowercase SHA-1")
	}
	if _, err := a.capture(ctx, "git", "cat-file", "-e", commit+"^{commit}"); err != nil {
		return fmt.Errorf("source commit does not exist: %w", err)
	}
	if _, err := a.capture(ctx, "git", "merge-base", "--is-ancestor", commit, "HEAD"); err != nil {
		return errors.New("source commit is not an ancestor of the current candidate HEAD")
	}
	return a.verifyTrustedSSHCommit(ctx, commit)
}

func (a *App) verifyTrustedSSHCommit(ctx context.Context, commit string) error {
	raw, err := a.capture(ctx, "git", "cat-file", "-p", commit)
	if err != nil {
		return err
	}
	publicKey, algorithm, err := commitSSHPublicKey(raw)
	if err != nil {
		return err
	}
	digest := sha256.Sum256(publicKey)
	fingerprint := "SHA256:" + base64.RawStdEncoding.EncodeToString(digest[:])
	if fingerprint != governanceSignerFingerprint {
		return fmt.Errorf("SSH signer fingerprint %s is not the trusted governance signer", fingerprint)
	}

	allowedSigners, err := os.CreateTemp("", "trpg-codex-allowed-signers-*")
	if err != nil {
		return fmt.Errorf("create temporary allowed-signers file: %w", err)
	}
	allowedSignersPath := allowedSigners.Name()
	defer os.Remove(allowedSignersPath)
	line := fmt.Sprintf("codex-governance %s %s\n", algorithm, base64.StdEncoding.EncodeToString(publicKey))
	if _, err := allowedSigners.WriteString(line); err != nil {
		allowedSigners.Close()
		return fmt.Errorf("write temporary allowed-signers file: %w", err)
	}
	if err := allowedSigners.Close(); err != nil {
		return fmt.Errorf("close temporary allowed-signers file: %w", err)
	}
	if _, err := a.capture(ctx, "git", "-c", "gpg.ssh.allowedSignersFile="+allowedSignersPath, "verify-commit", commit); err != nil {
		return fmt.Errorf("SSH signature verification failed: %w", err)
	}
	return nil
}

func commitSSHPublicKey(rawCommit string) ([]byte, string, error) {
	lines := strings.Split(rawCommit, "\n")
	var signature strings.Builder
	collecting := false
	for _, line := range lines {
		if strings.HasPrefix(line, "gpgsig ") {
			collecting = true
			signature.WriteString(strings.TrimPrefix(line, "gpgsig "))
			signature.WriteByte('\n')
			continue
		}
		if collecting && strings.HasPrefix(line, " ") {
			signature.WriteString(strings.TrimPrefix(line, " "))
			signature.WriteByte('\n')
			continue
		}
		if collecting {
			break
		}
	}
	block, _ := pem.Decode([]byte(signature.String()))
	if block == nil || block.Type != "SSH SIGNATURE" {
		return nil, "", errors.New("source commit does not contain an SSH signature")
	}
	if len(block.Bytes) < 10 || string(block.Bytes[:6]) != "SSHSIG" || binary.BigEndian.Uint32(block.Bytes[6:10]) != 1 {
		return nil, "", errors.New("source commit contains an unsupported SSH signature envelope")
	}
	publicKey, _, err := readSSHString(block.Bytes, 10)
	if err != nil {
		return nil, "", fmt.Errorf("read SSH signature public key: %w", err)
	}
	algorithmBytes, _, err := readSSHString(publicKey, 0)
	if err != nil {
		return nil, "", fmt.Errorf("read SSH public key algorithm: %w", err)
	}
	algorithm := string(algorithmBytes)
	if algorithm != "ssh-ed25519" {
		return nil, "", fmt.Errorf("unsupported governance signing algorithm %q", algorithm)
	}
	return publicKey, algorithm, nil
}

func readSSHString(data []byte, offset int) ([]byte, int, error) {
	if offset < 0 || len(data)-offset < 4 {
		return nil, offset, errors.New("truncated SSH string length")
	}
	length := int(binary.BigEndian.Uint32(data[offset : offset+4]))
	start := offset + 4
	if length < 0 || length > len(data)-start {
		return nil, offset, errors.New("truncated SSH string payload")
	}
	return data[start : start+length], start + length, nil
}

func frontMatterValue(text, key string) string {
	for _, line := range strings.Split(text, "\n") {
		line = strings.TrimSpace(line)
		if strings.HasPrefix(line, key+":") {
			return strings.Trim(strings.TrimSpace(strings.TrimPrefix(line, key+":")), `"'`)
		}
	}
	return ""
}
