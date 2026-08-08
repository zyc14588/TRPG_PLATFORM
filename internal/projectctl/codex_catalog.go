// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package projectctl

import (
	"bufio"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"regexp"
	"sort"
	"strconv"
	"strings"
)

var roadmapMilestoneAnchorPattern = regexp.MustCompile(`<a id="SPEC-V1-ROADMAP-(M(?:0|[1-9][0-9]*))"></a>`)

type v1MilestoneCatalog struct {
	ordered []string
	known   map[string]bool
}

func (a *App) loadV1MilestoneCatalog() (v1MilestoneCatalog, error) {
	data, err := os.ReadFile(filepath.Join(a.root, "docs", "80-roadmap", "V1_MILESTONES.md"))
	if err != nil {
		return v1MilestoneCatalog{}, fmt.Errorf("read V1 milestone roadmap: %w", err)
	}
	matches := roadmapMilestoneAnchorPattern.FindAllStringSubmatch(string(data), -1)
	catalog := v1MilestoneCatalog{known: make(map[string]bool, len(matches))}
	for _, match := range matches {
		milestone := match[1]
		if catalog.known[milestone] {
			return v1MilestoneCatalog{}, fmt.Errorf("V1 milestone roadmap repeats %s", milestone)
		}
		catalog.known[milestone] = true
		catalog.ordered = append(catalog.ordered, milestone)
	}
	sort.Slice(catalog.ordered, func(left, right int) bool {
		leftNumber, _ := milestoneNumber(catalog.ordered[left])
		rightNumber, _ := milestoneNumber(catalog.ordered[right])
		return leftNumber < rightNumber
	})
	if len(catalog.ordered) == 0 || catalog.ordered[0] != "M0" {
		return v1MilestoneCatalog{}, fmt.Errorf("V1 milestone roadmap must start at M0")
	}
	for index, milestone := range catalog.ordered {
		number, err := milestoneNumber(milestone)
		if err != nil || number != index {
			return v1MilestoneCatalog{}, fmt.Errorf("V1 milestone roadmap is not contiguous at %s", milestone)
		}
	}
	return catalog, nil
}

func validateKnownMilestone(milestone string, catalog v1MilestoneCatalog) error {
	if !milestonePattern.MatchString(milestone) {
		return fmt.Errorf("invalid milestone %q; expected M followed by a canonical non-negative sequence", milestone)
	}
	if !catalog.known[milestone] {
		return fmt.Errorf("milestone %q is not defined by the V1 roadmap", milestone)
	}
	return nil
}

func milestoneNumber(milestone string) (int, error) {
	match := milestonePattern.FindStringSubmatch(milestone)
	if match == nil {
		return 0, fmt.Errorf("invalid milestone %q", milestone)
	}
	number, err := strconv.Atoi(match[1])
	if err != nil {
		return 0, fmt.Errorf("invalid milestone %q", milestone)
	}
	return number, nil
}

func consecutiveMilestones(previous, proposed string, catalog v1MilestoneCatalog) bool {
	previousNumber, previousErr := milestoneNumber(previous)
	proposedNumber, proposedErr := milestoneNumber(proposed)
	return previousErr == nil && proposedErr == nil && proposedNumber == previousNumber+1 &&
		catalog.known[previous] && catalog.known[proposed]
}

type milestoneRouteInputs struct {
	requirements []requirement
	tests        []testCase
	mappings     []traceMapping
	decisions    []decision
	normative    []routeSpec
}

func (a *App) routePaths(request codexRouteRequest) (codexRoutePaths, error) {
	routeDocumentIDs := map[string]string{
		"PLAN": "CODEX-ROUTE-PLAN", "IMPLEMENT": "CODEX-ROUTE-IMPLEMENT",
		"ACCEPT": "CODEX-ROUTE-ACCEPT", "REPAIR": "CODEX-ROUTE-REPAIR",
	}
	modeDocumentID, ok := routeDocumentIDs[request.Mode]
	if !ok {
		return codexRoutePaths{}, fmt.Errorf("invalid Codex mode %q", request.Mode)
	}

	milestoneScopePath := fmt.Sprintf("docs/80-roadmap/%s_SCOPE_AND_EXIT_GATE.md", request.Milestone)
	paths := codexRoutePaths{
		always: []routeSpec{
			route(".codex/SESSION_START.md", "CODEX-SESSION-START", "governance-entry"),
			route(".codex/routes/"+request.Mode+".md", modeDocumentID, "mode-policy"),
			route(".codex/state/PROJECT_SNAPSHOT.md", "CODEX-PROJECT-SNAPSHOT", "state-summary"),
			route(".codex/state/MILESTONE_STATUS.md", "CODEX-MILESTONE-STATUS", "state-summary"),
		},
		normative: []routeSpec{
			route("docs/00-governance/DOCUMENT_AUTHORITY.md", "SPEC-DOCUMENT-AUTHORITY-READING", "normative-section"),
			route("docs/00-governance/IMPLEMENTATION_GOVERNANCE.md", "SPEC-IMPLEMENTATION-GOV-MODES", "normative-section"),
			route("docs/00-governance/IMPLEMENTATION_GOVERNANCE.md", "SPEC-IMPLEMENTATION-GOV-PROGRESSIVE", "normative-section"),
			route("docs/00-governance/IMPLEMENTATION_GOVERNANCE.md", "SPEC-IMPLEMENTATION-GOV-AUTONOMY", "normative-section"),
			route("docs/00-governance/IMPLEMENTATION_GOVERNANCE.md", "SPEC-IMPLEMENTATION-GOV-BATCH", "normative-section"),
			route("docs/80-roadmap/V1_MILESTONES.md", "SPEC-V1-ROADMAP-"+request.Milestone, "normative-section"),
			route(milestoneScopePath, "SPEC-"+request.Milestone+"-ALLOWED", "normative-section"),
			route(milestoneScopePath, "SPEC-"+request.Milestone+"-FORBIDDEN", "normative-section"),
			route(milestoneScopePath, "SPEC-"+request.Milestone+"-EXIT", "normative-section"),
			route(".codex/state/DECISION_DIGEST.md", "CODEX-DECISION-DIGEST", "state-summary"),
		},
	}

	inputs, err := a.loadMilestoneRouteInputs(request)
	if err != nil {
		return codexRoutePaths{}, err
	}
	paths.normative = append(paths.normative, inputs.normative...)
	for _, requirement := range inputs.requirements {
		paths.machine = append(paths.machine, route("docs/90-traceability/REQUIREMENTS.yaml", requirement.ID, "machine-contract"))
	}
	for _, test := range inputs.tests {
		paths.machine = append(paths.machine, route("docs/90-traceability/TEST_CATALOG.yaml", test.ID, "machine-contract"))
	}
	for _, mapping := range inputs.mappings {
		paths.machine = append(paths.machine, route("docs/90-traceability/TRACEABILITY.yaml", mapping.RequirementID, "machine-contract"))
	}
	paths.machine = append(paths.machine,
		route("schemas/codex/reading-map-v3.schema.json", "SCHEMA-CODEX-READING-MAP-V3", "machine-contract"),
		route("schemas/codex/milestone-plan-v2.schema.json", "SCHEMA-CODEX-MILESTONE-PLAN-V2", "machine-contract"),
		route(".codex/state/MILESTONE_PLAN.yaml", request.Milestone+"-MILESTONE-PLAN", "machine-contract"),
	)
	for _, decision := range inputs.decisions {
		paths.machine = append(paths.machine, route("docs/70-decisions/DECISION_REGISTER.yaml", decision.ID, "machine-contract"))
	}

	switch request.Mode {
	case "PLAN":
		paths.always = append(paths.always, route(".codex/planning/AUTONOMOUS_PLANNING_POLICY.md", "CODEX-AUTONOMOUS-PLANNING", "governance-policy"))
		paths.onDemand = []routeSpec{
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
	return paths, nil
}

func (a *App) loadMilestoneRouteInputs(request codexRouteRequest) (milestoneRouteInputs, error) {
	decisions, requirements, tests, trace, err := a.loadAuthority()
	if err != nil {
		return milestoneRouteInputs{}, err
	}
	selectedIDs := map[string]bool{"REQ-GOV-002": true, "REQ-GOV-003": true}
	if request.Mode == "PLAN" {
		for _, requirement := range requirements.Requirements {
			if requirement.Milestone == request.Milestone {
				selectedIDs[requirement.ID] = true
			}
		}
	} else {
		plan, err := loadYAML[milestonePlan](a.root, ".codex/state/MILESTONE_PLAN.yaml")
		if err != nil {
			return milestoneRouteInputs{}, err
		}
		for _, batch := range plan.Batches {
			if batch.BatchID == request.BatchID {
				for _, requirementID := range batch.Requirements {
					selectedIDs[requirementID] = true
				}
				break
			}
		}
	}

	inputs := milestoneRouteInputs{}
	foundIDs := make(map[string]bool, len(selectedIDs))
	ownerIDs := map[string]bool{}
	decisionIDs := map[string]bool{}
	milestoneRequirementCount := 0
	for _, requirement := range requirements.Requirements {
		if !selectedIDs[requirement.ID] {
			continue
		}
		if requirement.Milestone != request.Milestone && requirement.ID != "REQ-GOV-002" && requirement.ID != "REQ-GOV-003" {
			return milestoneRouteInputs{}, fmt.Errorf("route requirement %s belongs to %s, not %s", requirement.ID, requirement.Milestone, request.Milestone)
		}
		inputs.requirements = append(inputs.requirements, requirement)
		foundIDs[requirement.ID] = true
		if requirement.Milestone == request.Milestone {
			milestoneRequirementCount++
			ownerIDs[requirement.OwningSpec] = true
		}
		for _, decisionID := range requirement.SourceDecisions {
			decisionIDs[decisionID] = true
		}
	}
	for requirementID := range selectedIDs {
		if !foundIDs[requirementID] {
			return milestoneRouteInputs{}, fmt.Errorf("route requirement %s does not exist", requirementID)
		}
	}
	if milestoneRequirementCount == 0 {
		return milestoneRouteInputs{}, fmt.Errorf("milestone %s has no routeable requirements", request.Milestone)
	}

	mappingByRequirement := make(map[string]traceMapping, len(trace.Mappings))
	for _, mapping := range trace.Mappings {
		mappingByRequirement[mapping.RequirementID] = mapping
	}
	testByID := make(map[string]testCase, len(tests.Tests))
	for _, test := range tests.Tests {
		testByID[test.ID] = test
	}
	selectedTestIDs := map[string]bool{}
	for _, requirement := range inputs.requirements {
		mapping, ok := mappingByRequirement[requirement.ID]
		if !ok || mapping.Milestone != requirement.Milestone || mapping.OwningSpec != requirement.OwningSpec {
			return milestoneRouteInputs{}, fmt.Errorf("route requirement %s lacks a consistent traceability row", requirement.ID)
		}
		inputs.mappings = append(inputs.mappings, mapping)
		for _, decisionID := range mapping.SourceDecisions {
			decisionIDs[decisionID] = true
		}
		for _, testID := range mapping.TestIDs {
			test, ok := testByID[testID]
			if !ok || test.Milestone != requirement.Milestone {
				return milestoneRouteInputs{}, fmt.Errorf("route requirement %s references inconsistent test %s", requirement.ID, testID)
			}
			selectedTestIDs[testID] = true
		}
	}
	for _, test := range tests.Tests {
		if selectedTestIDs[test.ID] {
			inputs.tests = append(inputs.tests, test)
		}
	}

	for _, decision := range decisions.Decisions {
		if !decisionIDs[decision.ID] {
			continue
		}
		if decision.Status != "ACTIVE" {
			return milestoneRouteInputs{}, fmt.Errorf("route source decision %s is not ACTIVE", decision.ID)
		}
		inputs.decisions = append(inputs.decisions, decision)
		delete(decisionIDs, decision.ID)
	}
	if len(decisionIDs) != 0 {
		missing := make([]string, 0, len(decisionIDs))
		for decisionID := range decisionIDs {
			missing = append(missing, decisionID)
		}
		sort.Strings(missing)
		return milestoneRouteInputs{}, fmt.Errorf("route source decisions do not exist: %s", strings.Join(missing, ", "))
	}

	documentPaths, err := a.normativeDocumentIndex()
	if err != nil {
		return milestoneRouteInputs{}, err
	}
	addedOwners := map[string]bool{}
	for _, requirement := range inputs.requirements {
		if !ownerIDs[requirement.OwningSpec] || addedOwners[requirement.OwningSpec] {
			continue
		}
		path, ok := documentPaths[requirement.OwningSpec]
		if !ok {
			return milestoneRouteInputs{}, fmt.Errorf("owning specification %s has no bounded normative document", requirement.OwningSpec)
		}
		inputs.normative = append(inputs.normative, route(path, requirement.OwningSpec, "normative-section"))
		addedOwners[requirement.OwningSpec] = true
	}
	return inputs, nil
}

func (a *App) normativeDocumentIndex() (map[string]string, error) {
	docsRoot := filepath.Join(a.root, "docs")
	index := map[string]string{}
	err := filepath.WalkDir(docsRoot, func(path string, entry fs.DirEntry, walkErr error) error {
		if walkErr != nil {
			return walkErr
		}
		if entry.IsDir() {
			name := entry.Name()
			if path != docsRoot && (name == "70-decisions" || name == "90-traceability") {
				return filepath.SkipDir
			}
			return nil
		}
		if filepath.Ext(path) != ".md" {
			return nil
		}
		documentID, err := readFrontMatterDocumentID(path)
		if err != nil {
			return err
		}
		if documentID == "" {
			return nil
		}
		relative, err := filepath.Rel(a.root, path)
		if err != nil {
			return err
		}
		relative = filepath.ToSlash(relative)
		if previous, exists := index[documentID]; exists {
			return fmt.Errorf("normative document ID %s is repeated in %s and %s", documentID, previous, relative)
		}
		index[documentID] = relative
		return nil
	})
	if err != nil {
		return nil, fmt.Errorf("index bounded normative documents: %w", err)
	}
	return index, nil
}

func readFrontMatterDocumentID(path string) (string, error) {
	file, err := os.Open(path)
	if err != nil {
		return "", err
	}
	defer file.Close()
	scanner := bufio.NewScanner(file)
	scanner.Buffer(make([]byte, 4096), 64*1024)
	lineNumber := 0
	documentID := ""
	for scanner.Scan() {
		lineNumber++
		line := strings.TrimSpace(scanner.Text())
		if lineNumber == 1 && line != "---" {
			return "", nil
		}
		if lineNumber > 1 && line == "---" {
			break
		}
		if strings.HasPrefix(line, "document_id:") {
			documentID = strings.Trim(strings.TrimSpace(strings.TrimPrefix(line, "document_id:")), `"'`)
		}
	}
	if err := scanner.Err(); err != nil {
		return "", fmt.Errorf("read normative front matter %s: %w", path, err)
	}
	return documentID, nil
}
