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

// x-section-id: PROJECTCTL-CODEX-ROUTING
const (
	readingMapSchemaVersion    = 4
	milestonePlanSchemaVersion = 2
	codexExecutor              = "codex"
	codexContextProfileID      = "codex-default"
	openCodeExecutor           = "opencode"
	openCodeContextProfileID   = "opencode-deepseek-v4-flash"
	contextMeasurement         = "utf8-bytes"
	softContextRatio           = 0.55
	hardContextRatio           = 0.70
	milestoneRouteScope        = "MILESTONE"
	batchRouteScope            = "BATCH"
	maintenanceRouteScope      = "GOVERNANCE_MAINTENANCE"
)

var (
	commitPattern    = regexp.MustCompile(`^[0-9a-f]{40}$`)
	milestonePattern = regexp.MustCompile(`^M(0|[1-9][0-9]*)$`)
	batchPattern     = regexp.MustCompile(`^(M(?:0|[1-9][0-9]*))-B([0-9]{3,})$`)
	recordPattern    = regexp.MustCompile(`^(\s*)- (decision_id|requirement_id|test_id):\s*["']?([^"'[:space:]]+)["']?\s*$`)
)

type codexRouteRequest struct {
	Mode                 string
	Milestone            string
	BatchID              string
	MaintenanceID        string
	Executor             string
	ProfileID            string
	ContextCapacityBytes int
}

func newCodexRouteRequest(mode string) codexRouteRequest {
	return codexRouteRequest{
		Mode:      strings.ToUpper(mode),
		Executor:  codexExecutor,
		ProfileID: codexContextProfileID,
	}
}

func (a *App) codexCommand(ctx context.Context, args []string) error {
	if len(args) >= 1 && args[0] == "plan" {
		request, err := parseCodexPlanRequest(args[1:])
		if err != nil {
			return err
		}
		return a.generateCodexRouteRequest(ctx, request)
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
	return usageError("codex plan --milestone M? [profile options]|route --mode MODE (--milestone M? [--batch M?-B???] | --maintenance GOV-ID) [profile options]|check")
}

func parseCodexPlanRequest(args []string) (codexRouteRequest, error) {
	request := newCodexRouteRequest("PLAN")
	seen := map[string]bool{}
	for index := 0; index < len(args); index += 2 {
		if index+1 >= len(args) {
			return request, usageError("codex plan options require values")
		}
		option, value := args[index], args[index+1]
		if seen[option] {
			return request, fmt.Errorf("duplicate Codex plan option %s", option)
		}
		seen[option] = true
		switch option {
		case "--milestone":
			request.Milestone = value
		case "--executor":
			request.Executor = strings.ToLower(value)
		case "--profile":
			request.ProfileID = value
		case "--context-capacity-bytes":
			capacity, err := strconv.Atoi(value)
			if err != nil || capacity <= 0 {
				return request, fmt.Errorf("context capacity must be a positive integer, got %q", value)
			}
			request.ContextCapacityBytes = capacity
		default:
			return request, fmt.Errorf("unknown Codex plan option %s", option)
		}
	}
	if request.Milestone == "" {
		return request, errors.New("Codex plan requires --milestone")
	}
	return request, nil
}

func parseCodexRouteRequest(args []string) (codexRouteRequest, error) {
	request := newCodexRouteRequest("")
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
		case "--maintenance":
			request.MaintenanceID = value
		case "--executor":
			request.Executor = strings.ToLower(value)
		case "--profile":
			request.ProfileID = value
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
	if request.Milestone == "" && request.MaintenanceID == "" {
		return request, errors.New("Codex route requires a milestone/batch target or --maintenance")
	}
	return request, nil
}

func (a *App) generateCodexRouteRequest(ctx context.Context, request codexRouteRequest) error {
	catalog, err := a.loadV1MilestoneCatalog()
	if err != nil {
		return err
	}
	if err := validateCodexRouteRequest(request, catalog); err != nil {
		return err
	}
	profile, err := contextProfileForRequest(request)
	if err != nil {
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
	if err := a.validateRouteAgainstCurrentPlan(request, catalog); err != nil {
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

	specs, err := a.routePaths(request)
	if err != nil {
		return err
	}
	mapFile := readingMap{
		RouteSchemaVersion: readingMapSchemaVersion,
		Mode:               request.Mode,
		Milestone:          request.Milestone,
		RouteScope:         routeScopeForRequest(request),
		BatchID:            request.BatchID,
		MaintenanceID:      request.MaintenanceID,
		SourceCommit:       commit,
		SourceTree:         tree,
		GeneratedUTC:       time.Now().UTC().Format(time.RFC3339),
		MustNotBulkRead:    mustNotBulkReadForRequest(request),
		ContextProfile:     profile,
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
	fmt.Fprintf(a.stdout, "[ROUTE] %s %s -> .codex/runtime/READING_MAP.yaml (%s, %s, %s)\n",
		request.Mode, routeRequestTarget(request), commit, contextTelemetrySummary(mapFile), mapFile.Budget.Status)
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

func routeScopeForRequest(request codexRouteRequest) string {
	if request.MaintenanceID != "" {
		return maintenanceRouteScope
	}
	if request.Mode == "PLAN" {
		return milestoneRouteScope
	}
	return batchRouteScope
}

func validateCodexRouteRequest(request codexRouteRequest, catalog v1MilestoneCatalog) error {
	if !map[string]bool{"PLAN": true, "IMPLEMENT": true, "ACCEPT": true, "REPAIR": true}[request.Mode] {
		return fmt.Errorf("invalid Codex mode %q", request.Mode)
	}
	if request.BatchID != "" && request.MaintenanceID != "" {
		return errors.New("--batch and --maintenance are mutually exclusive")
	}
	if request.MaintenanceID != "" {
		if request.Milestone != "" {
			return errors.New("governance maintenance routes must not claim a milestone")
		}
		if !maintenanceIDPattern.MatchString(request.MaintenanceID) {
			return fmt.Errorf("invalid governance maintenance ID %q; expected GOV-[A-Z0-9-]+", request.MaintenanceID)
		}
		if request.Mode == "PLAN" || request.Mode == "IMPLEMENT" {
			return fmt.Errorf("%s cannot target governance maintenance", request.Mode)
		}
	} else {
		if request.Milestone == "" {
			return errors.New("milestone target is required when --maintenance is absent")
		}
		if err := validateKnownMilestone(request.Milestone, catalog); err != nil {
			return err
		}
		if request.Mode == "PLAN" {
			if request.BatchID != "" {
				return errors.New("PLAN routes are milestone-level and must not allocate or accept a batch_id")
			}
		} else {
			if request.BatchID == "" {
				return fmt.Errorf("%s routes require a real batch_id or governance maintenance target", request.Mode)
			}
			if err := validateBatchID(request.Milestone, request.BatchID); err != nil {
				return err
			}
		}
	}
	if _, err := contextProfileForRequest(request); err != nil {
		return err
	}
	return nil
}

func contextProfileForRequest(request codexRouteRequest) (contextProfile, error) {
	switch {
	case request.Executor == codexExecutor && request.ProfileID == codexContextProfileID:
		if request.ContextCapacityBytes < 0 {
			return contextProfile{}, errors.New("Codex telemetry capacity cannot be negative")
		}
		return contextProfile{
			Executor: codexExecutor, ProfileID: codexContextProfileID, Enforcement: false,
			CapacityBytes: request.ContextCapacityBytes, Measurement: contextMeasurement,
		}, nil
	case request.Executor == openCodeExecutor && request.ProfileID == openCodeContextProfileID:
		if request.ContextCapacityBytes <= 0 {
			return contextProfile{}, errors.New("PROFILE_REQUIRED: opencode-deepseek-v4-flash requires explicit --context-capacity-bytes")
		}
		return contextProfile{
			Executor: openCodeExecutor, ProfileID: openCodeContextProfileID, Enforcement: true,
			CapacityBytes: request.ContextCapacityBytes, Measurement: contextMeasurement,
		}, nil
	default:
		return contextProfile{}, fmt.Errorf("PROFILE_REQUIRED: unknown executor/profile %q/%q", request.Executor, request.ProfileID)
	}
}

func validateBatchID(milestone, batchID string) error {
	match := batchPattern.FindStringSubmatch(batchID)
	if match == nil || match[1] != milestone {
		return fmt.Errorf("batch_id %q is not bound to milestone %s", batchID, milestone)
	}
	return nil
}

func (a *App) validateRouteAgainstCurrentPlan(request codexRouteRequest, catalog v1MilestoneCatalog) error {
	if request.MaintenanceID != "" {
		_, err := a.loadGovernanceMaintenanceContract(request.MaintenanceID)
		return err
	}
	plan, err := loadYAML[milestonePlan](a.root, ".codex/state/MILESTONE_PLAN.yaml")
	if err != nil {
		return err
	}
	return validateRouteAgainstPlan(request, plan, catalog)
}

func validateRouteAgainstPlan(request codexRouteRequest, plan milestonePlan, catalog v1MilestoneCatalog) error {
	if err := validateMilestonePlan(plan, catalog); err != nil {
		return fmt.Errorf("current milestone plan: %w", err)
	}
	if request.Milestone != plan.Milestone {
		return fmt.Errorf("milestone %s is outside the current %s planning boundary", request.Milestone, plan.Milestone)
	}
	if request.Mode == "PLAN" {
		if plan.Status == "COMPLETE" {
			return fmt.Errorf("milestone %s is complete and cannot be replanned", plan.Milestone)
		}
		return nil
	}
	if plan.Status == "NOT_GENERATED" {
		return fmt.Errorf("%s has no generated plan or real batch IDs", plan.Milestone)
	}
	for _, batch := range plan.Batches {
		if batch.BatchID == request.BatchID {
			return nil
		}
	}
	return fmt.Errorf("batch_id %s is not allocated by the current milestone plan", request.BatchID)
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
		for _, prefix := range []string{"// x-section-id:", "# x-section-id:"} {
			if strings.HasPrefix(line, prefix) {
				id := strings.Trim(strings.TrimSpace(strings.TrimPrefix(line, prefix)), `"'`)
				markers = append(markers, sectionMarker{id: id, kind: "anchor", group: prefix, line: index})
			}
		}
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

// x-section-id: PROJECTCTL-CONTEXT-POLICY
func applyContextBudget(route *readingMap) error {
	if err := validateContextProfile(route.ContextProfile); err != nil {
		return err
	}
	capacity := route.ContextProfile.CapacityBytes
	initial := measureRouteSections(route.AlwaysRead, route.NormativeReferences, route.MachineContracts, route.ReadOnDemand)
	route.Budget = contextBudget{
		InitialMaterialBytes: initial,
		MaterialBytes:        initial,
		CapacityBytes:        capacity,
		Status:               "TELEMETRY_ONLY",
	}
	if capacity > 0 {
		route.Budget.InitialRatio = float64(initial) / float64(capacity)
		route.Budget.ActualRatio = route.Budget.InitialRatio
	}
	if !route.ContextProfile.Enforcement {
		return nil
	}
	initialRatio := route.Budget.InitialRatio
	route.Budget.SoftLimitRatio = softContextRatio
	route.Budget.HardLimitRatio = hardContextRatio
	route.Budget.Status = "WITHIN_BUDGET"
	if initialRatio > hardContextRatio {
		return fmt.Errorf("%s context hard limit exceeded: %.4f > %.2f; hard stop, shrink the route or split the batch", route.ContextProfile.ProfileID, initialRatio, hardContextRatio)
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

func validateContextProfile(profile contextProfile) error {
	if profile.Measurement != contextMeasurement {
		return fmt.Errorf("context profile measurement %q is unsupported", profile.Measurement)
	}
	switch {
	case profile.Executor == codexExecutor && profile.ProfileID == codexContextProfileID && !profile.Enforcement:
		if profile.CapacityBytes < 0 {
			return errors.New("Codex telemetry capacity cannot be negative")
		}
		return nil
	case profile.Executor == openCodeExecutor && profile.ProfileID == openCodeContextProfileID && profile.Enforcement:
		if profile.CapacityBytes <= 0 {
			return errors.New("PROFILE_REQUIRED: opencode-deepseek-v4-flash requires explicit capacity")
		}
		return nil
	default:
		return fmt.Errorf("PROFILE_REQUIRED: unknown or mismatched executor/profile %q/%q", profile.Executor, profile.ProfileID)
	}
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
		RouteScope          string         `json:"route_scope"`
		BatchID             string         `json:"batch_id,omitempty"`
		MaintenanceID       string         `json:"maintenance_id,omitempty"`
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
		RouteSchemaVersion: route.RouteSchemaVersion, Mode: route.Mode, Milestone: route.Milestone,
		RouteScope: route.RouteScope, BatchID: route.BatchID, MaintenanceID: route.MaintenanceID,
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

// x-section-id: PROJECTCTL-CODEX-VALIDATION
func (a *App) checkCodex(ctx context.Context) error {
	if err := a.checkCodexStatic(ctx); err != nil {
		return err
	}
	catalog, err := a.loadV1MilestoneCatalog()
	if err != nil {
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
	if err := validateReadingMapMetadata(route, catalog); err != nil {
		return err
	}
	if err := a.validateRouteAgainstCurrentPlan(codexRouteRequest{
		Mode: route.Mode, Milestone: route.Milestone, BatchID: route.BatchID, MaintenanceID: route.MaintenanceID,
		Executor: route.ContextProfile.Executor, ProfileID: route.ContextProfile.ProfileID,
		ContextCapacityBytes: route.ContextProfile.CapacityBytes,
	}, catalog); err != nil {
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
	if err := a.validateCanonicalReadingMap(route); err != nil {
		return err
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
	fmt.Fprintf(a.stdout, "[PASS] Codex %s route for %s is current at %s (%s, %s)\n",
		route.Mode, routeTarget(route), route.SourceCommit, contextTelemetrySummary(route), route.Budget.Status)
	return nil
}

func (a *App) validateCanonicalReadingMap(actual readingMap) error {
	specs, err := a.routePaths(codexRouteRequest{
		Mode: actual.Mode, Milestone: actual.Milestone, BatchID: actual.BatchID, MaintenanceID: actual.MaintenanceID,
		Executor: actual.ContextProfile.Executor, ProfileID: actual.ContextProfile.ProfileID,
		ContextCapacityBytes: actual.ContextProfile.CapacityBytes,
	})
	if err != nil {
		return err
	}
	expected := readingMap{
		ContextProfile:  actual.ContextProfile,
		MustNotBulkRead: mustNotBulkReadForRequest(codexRouteRequest{MaintenanceID: actual.MaintenanceID}),
	}
	if expected.AlwaysRead, err = a.hashRouteSections(specs.always); err != nil {
		return err
	}
	if expected.NormativeReferences, err = a.hashRouteSections(specs.normative); err != nil {
		return err
	}
	if expected.MachineContracts, err = a.hashRouteSections(specs.machine); err != nil {
		return err
	}
	if expected.ReadOnDemand, err = a.hashRouteSections(specs.onDemand); err != nil {
		return err
	}
	if err := applyContextBudget(&expected); err != nil {
		return err
	}
	actualCanonical, err := json.Marshal(struct {
		Budget              contextBudget
		AlwaysRead          []routeSection
		NormativeReferences []routeSection
		MachineContracts    []routeSection
		ReadOnDemand        []routeSection
		SoftLimitOmissions  []routeSection
		MustNotBulkRead     []string
	}{
		actual.Budget, actual.AlwaysRead, actual.NormativeReferences, actual.MachineContracts,
		actual.ReadOnDemand, actual.SoftLimitOmissions, actual.MustNotBulkRead,
	})
	if err != nil {
		return err
	}
	expectedCanonical, err := json.Marshal(struct {
		Budget              contextBudget
		AlwaysRead          []routeSection
		NormativeReferences []routeSection
		MachineContracts    []routeSection
		ReadOnDemand        []routeSection
		SoftLimitOmissions  []routeSection
		MustNotBulkRead     []string
	}{
		expected.Budget, expected.AlwaysRead, expected.NormativeReferences, expected.MachineContracts,
		expected.ReadOnDemand, expected.SoftLimitOmissions, expected.MustNotBulkRead,
	})
	if err != nil {
		return err
	}
	if !bytes.Equal(actualCanonical, expectedCanonical) {
		return errors.New("Codex runtime route does not match the canonical Section set for its mode and context profile; regenerate it")
	}
	return nil
}

func validateReadingMapMetadata(route readingMap, catalog v1MilestoneCatalog) error {
	if route.RouteSchemaVersion != readingMapSchemaVersion {
		return errors.New("Codex runtime route schema version is invalid")
	}
	if err := validateCodexRouteRequest(codexRouteRequest{
		Mode: route.Mode, Milestone: route.Milestone, BatchID: route.BatchID, MaintenanceID: route.MaintenanceID,
		Executor: route.ContextProfile.Executor, ProfileID: route.ContextProfile.ProfileID,
		ContextCapacityBytes: route.ContextProfile.CapacityBytes,
	}, catalog); err != nil {
		return err
	}
	if route.RouteScope != routeScopeForRequest(codexRouteRequest{
		Mode: route.Mode, Milestone: route.Milestone, BatchID: route.BatchID, MaintenanceID: route.MaintenanceID,
	}) {
		return fmt.Errorf("Codex runtime route scope %q is invalid for %s mode", route.RouteScope, route.Mode)
	}
	if !commitPattern.MatchString(route.SourceCommit) || !commitPattern.MatchString(route.SourceTree) {
		return errors.New("Codex runtime route source commit/tree binding is invalid")
	}
	if _, err := time.Parse(time.RFC3339, route.GeneratedUTC); err != nil {
		return errors.New("Codex runtime route generation timestamp is invalid")
	}
	if err := validateContextProfile(route.ContextProfile); err != nil {
		return fmt.Errorf("Codex runtime route context profile is invalid: %w", err)
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
	if err := validateContextProfile(route.ContextProfile); err != nil {
		return err
	}
	current := measureRouteSections(route.AlwaysRead, route.NormativeReferences, route.MachineContracts, route.ReadOnDemand)
	initial := current + measureRouteSections(route.SoftLimitOmissions)
	capacity := route.ContextProfile.CapacityBytes
	budget := route.Budget
	if budget.InitialMaterialBytes != initial || budget.MaterialBytes != current || budget.CapacityBytes != capacity {
		return errors.New("Codex runtime route context budget measurement is inconsistent")
	}
	initialRatio, actualRatio := 0.0, 0.0
	if capacity > 0 {
		initialRatio = float64(initial) / float64(capacity)
		actualRatio = float64(current) / float64(capacity)
	}
	if budget.InitialRatio != initialRatio || budget.ActualRatio != actualRatio {
		return errors.New("Codex runtime route context ratio telemetry is inconsistent")
	}
	if !route.ContextProfile.Enforcement {
		if budget.Status != "TELEMETRY_ONLY" || budget.SoftLimitRatio != 0 || budget.HardLimitRatio != 0 ||
			budget.ReductionApplied || budget.PlanSplitRequired || len(route.SoftLimitOmissions) != 0 || current != initial {
			return errors.New("Codex unrestricted profile applied a project-level context gate")
		}
		return nil
	}
	if budget.SoftLimitRatio != softContextRatio || budget.HardLimitRatio != hardContextRatio {
		return errors.New("enforcement-enabled context thresholds are inconsistent")
	}
	if initialRatio > hardContextRatio || budget.PlanSplitRequired {
		return errors.New("enforcement-enabled runtime route exceeds the hard context limit; shrink route or split required")
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

func routeTarget(route readingMap) string {
	if route.MaintenanceID != "" {
		return route.MaintenanceID
	}
	if route.BatchID == "" {
		return route.Milestone + "/MILESTONE"
	}
	return route.Milestone + "/" + route.BatchID
}

func routeRequestTarget(request codexRouteRequest) string {
	if request.MaintenanceID != "" {
		return request.MaintenanceID
	}
	if request.BatchID == "" {
		return request.Milestone + "/MILESTONE"
	}
	return request.Milestone + "/" + request.BatchID
}

func mustNotBulkReadForRequest(request codexRouteRequest) []string {
	forbidden := []string{"docs/**", "git-history", "prior-chat-transcripts"}
	if request.MaintenanceID != "" {
		forbidden = append(forbidden, "unrelated-product-specs", "M1-business-implementation")
	}
	return forbidden
}

func contextTelemetrySummary(route readingMap) string {
	if route.ContextProfile.CapacityBytes == 0 {
		return fmt.Sprintf("%d context bytes measured", route.Budget.MaterialBytes)
	}
	return fmt.Sprintf("%.4f context ratio", route.Budget.ActualRatio)
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
		".codex/maintenance/GOV-M1-PLANNING-RUNTIME/CONTRACT.yaml",
		"schemas/codex/reading-map-v2.schema.json", "schemas/codex/reading-map-v3.schema.json", "schemas/codex/reading-map-v4.schema.json",
		"schemas/codex/governance-maintenance-contract-v1.schema.json",
		"schemas/codex/milestone-plan-v1.schema.json", "schemas/codex/milestone-plan-v2.schema.json",
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
	catalog, err := a.loadV1MilestoneCatalog()
	if err != nil {
		problems.add("%v", err)
	}
	plan, err := loadYAML[milestonePlan](a.root, ".codex/state/MILESTONE_PLAN.yaml")
	if err != nil {
		problems.add("%v", err)
	} else if err := validateMilestonePlan(plan, catalog); err != nil {
		problems.add("invalid MILESTONE_PLAN.yaml: %v", err)
	}
	maintenanceContracts, globErr := filepath.Glob(filepath.Join(a.root, ".codex", "maintenance", "*", "CONTRACT.yaml"))
	if globErr != nil {
		problems.add("discover governance maintenance contracts: %v", globErr)
	}
	for _, path := range maintenanceContracts {
		maintenanceID := filepath.Base(filepath.Dir(path))
		if _, err := a.loadGovernanceMaintenanceContract(maintenanceID); err != nil {
			problems.add("invalid governance maintenance contract %s: %v", maintenanceID, err)
		}
	}
	for _, relative := range []string{
		"schemas/codex/reading-map-v2.schema.json", "schemas/codex/reading-map-v3.schema.json", "schemas/codex/reading-map-v4.schema.json",
		"schemas/codex/governance-maintenance-contract-v1.schema.json",
		"schemas/codex/milestone-plan-v1.schema.json", "schemas/codex/milestone-plan-v2.schema.json",
	} {
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
	catalog, err := a.loadV1MilestoneCatalog()
	if err != nil {
		return err
	}
	return validateMilestonePlanChange(committedPlan, currentPlan, mode, catalog)
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

// x-section-id: PROJECTCTL-MILESTONE-LIFECYCLE
func validateMilestonePlan(plan milestonePlan, catalog v1MilestoneCatalog) error {
	if plan.SchemaVersion != milestonePlanSchemaVersion || plan.PlanID != plan.Milestone+"-MILESTONE-PLAN" || plan.ModifiableOnlyInMode != "PLAN" {
		return errors.New("plan metadata is invalid")
	}
	if err := validateKnownMilestone(plan.Milestone, catalog); err != nil {
		return err
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
	allocatedSequences := map[int]string{}
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
		if previous := allocatedSequences[sequence]; previous != "" {
			return fmt.Errorf("batch sequence %d is reused by %s and %s", sequence, previous, batch.BatchID)
		}
		allocated[batch.BatchID], active[batch.BatchID] = true, batch
		allocatedSequences[sequence] = batch.BatchID
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
		if previous := allocatedSequences[sequence]; previous != "" {
			return fmt.Errorf("batch sequence %d is reused by %s and %s", sequence, previous, tombstone.BatchID)
		}
		allocated[tombstone.BatchID] = true
		allocatedSequences[sequence] = tombstone.BatchID
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
	if len(allocatedSequences) != plan.NextBatchSequence-1 {
		return errors.New("allocated batch and tombstone sequences must be contiguous below next_batch_sequence")
	}
	for sequence := 1; sequence < plan.NextBatchSequence; sequence++ {
		if allocatedSequences[sequence] == "" {
			return fmt.Errorf("allocated batch sequence %d is missing", sequence)
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

func validateMilestonePlanChange(previous, proposed milestonePlan, mode string, catalog v1MilestoneCatalog) error {
	if err := validateMilestonePlan(previous, catalog); err != nil {
		return fmt.Errorf("previous plan: %w", err)
	}
	if err := validateMilestonePlan(proposed, catalog); err != nil {
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
	if previous.Milestone != proposed.Milestone {
		if previous.Status != "COMPLETE" || proposed.Status != "NOT_GENERATED" ||
			!consecutiveMilestones(previous.Milestone, proposed.Milestone, catalog) {
			return errors.New("milestone transition requires the completed current milestone and the next V1 NOT_GENERATED baseline")
		}
		return nil
	}
	if proposed.PlanVersion != previous.PlanVersion+1 || proposed.NextBatchSequence < previous.NextBatchSequence {
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
