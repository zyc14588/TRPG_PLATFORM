// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package projectctl

import (
	"fmt"
	"os"
	"path/filepath"
	"regexp"
	"sort"
	"strconv"
	"strings"
)

var roadmapMilestoneAnchorPattern = regexp.MustCompile(`<a id="SPEC-V1-ROADMAP-(M(?:0|[1-9][0-9]*))"></a>`)

// normativeSpecificationPaths is the bounded authoritative inventory used to
// resolve stable Section IDs. Route generation must never discover normative
// material by walking docs/**.
var normativeSpecificationPaths = []string{
	"docs/00-governance/CHANGE_CONTROL.md",
	"docs/00-governance/CODEX_PROGRESSIVE_DISCLOSURE.md",
	"docs/00-governance/DOCUMENT_AUTHORITY.md",
	"docs/00-governance/IMPLEMENTATION_GOVERNANCE.md",
	"docs/00-governance/LICENSE_AND_RIGHTS_POLICY.md",
	"docs/00-governance/V1_SCOPE.md",
	"docs/10-product/CREATOR_STUDIO.md",
	"docs/10-product/OFFICIAL_BOARDGAME.md",
	"docs/10-product/OFFICIAL_TRPG.md",
	"docs/10-product/PLAYER_EXPERIENCE.md",
	"docs/10-product/PRODUCT_DEFINITION.md",
	"docs/20-architecture/AI_ARCHITECTURE.md",
	"docs/20-architecture/DATA_AND_STORAGE.md",
	"docs/20-architecture/DEPLOYMENT_ARCHITECTURE.md",
	"docs/20-architecture/LUA_RUNTIME.md",
	"docs/20-architecture/SESSION_RUNTIME.md",
	"docs/20-architecture/SYSTEM_ARCHITECTURE.md",
	"docs/30-package-spec/DEPENDENCY_SIGNING_TRUST.md",
	"docs/30-package-spec/HOST_API_AND_CALLBACKS.md",
	"docs/30-package-spec/MIGRATION_COMPATIBILITY.md",
	"docs/30-package-spec/PACKAGE_MODEL.md",
	"docs/40-security/INCIDENT_RESPONSE.md",
	"docs/40-security/SECURITY_MODEL.md",
	"docs/40-security/SUPPLY_CHAIN_SECURITY.md",
	"docs/50-operations/BACKUP_RESTORE_OBSERVABILITY.md",
	"docs/50-operations/SELF_HOSTING_AND_UPGRADE.md",
	"docs/60-quality/ACCEPTANCE_POLICY.md",
	"docs/60-quality/COMPATIBILITY_AND_RELEASE_GATE.md",
	"docs/60-quality/TEST_STRATEGY.md",
	"docs/80-roadmap/M0_SCOPE_AND_EXIT_GATE.md",
	"docs/80-roadmap/M1_SCOPE_AND_EXIT_GATE.md",
	"docs/80-roadmap/V1_MILESTONES.md",
}

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

// x-section-id: PROJECTCTL-CODEX-CATALOG
func (a *App) routePaths(request codexRouteRequest) (codexRoutePaths, error) {
	if request.MaintenanceID != "" {
		return a.maintenanceRoutePaths(request)
	}
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
	paths.normative = appendUniqueRouteSpecs(paths.normative, inputs.normative...)
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
		route("schemas/codex/reading-map-v4.schema.json", "SCHEMA-CODEX-READING-MAP-V4", "machine-contract"),
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
	return deduplicateRoutePaths(paths), nil
}

func (a *App) loadMilestoneRouteInputs(request codexRouteRequest) (milestoneRouteInputs, error) {
	decisions, requirements, tests, trace, err := a.loadAuthority()
	if err != nil {
		return milestoneRouteInputs{}, err
	}
	selectedIDs := map[string]bool{"REQ-GOV-002": true, "REQ-GOV-003": true}
	var frozenReadingSectionIDs []string
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
				frozenReadingSectionIDs = append(frozenReadingSectionIDs, batch.ReadingMapSections...)
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

	sectionCatalog, err := a.normativeSectionCatalog()
	if err != nil {
		return milestoneRouteInputs{}, err
	}
	for _, requirement := range inputs.requirements {
		if !ownerIDs[requirement.OwningSpec] {
			continue
		}
		spec, err := resolveNormativeSection(sectionCatalog, requirement.OwningSpec)
		if err != nil {
			return milestoneRouteInputs{}, fmt.Errorf("owning specification %s: %w", requirement.OwningSpec, err)
		}
		inputs.normative = appendUniqueRouteSpecs(inputs.normative, spec)
	}
	for _, sectionID := range frozenReadingSectionIDs {
		spec, err := resolveNormativeSection(sectionCatalog, sectionID)
		if err != nil {
			return milestoneRouteInputs{}, fmt.Errorf("frozen reading_map_sections entry %s: %w", sectionID, err)
		}
		inputs.normative = appendUniqueRouteSpecs(inputs.normative, spec)
	}
	return inputs, nil
}

func appendUniqueRouteSpecs(target []routeSpec, specs ...routeSpec) []routeSpec {
	seen := make(map[string]bool, len(target)+len(specs))
	for _, spec := range target {
		seen[spec.path+"#"+spec.sectionID] = true
	}
	for _, spec := range specs {
		key := spec.path + "#" + spec.sectionID
		if seen[key] {
			continue
		}
		seen[key] = true
		target = append(target, spec)
	}
	return target
}

func deduplicateRoutePaths(paths codexRoutePaths) codexRoutePaths {
	seen := map[string]bool{}
	unique := func(specs []routeSpec) []routeSpec {
		result := make([]routeSpec, 0, len(specs))
		for _, spec := range specs {
			key := spec.path + "#" + spec.sectionID
			if seen[key] {
				continue
			}
			seen[key] = true
			result = append(result, spec)
		}
		return result
	}
	paths.always = unique(paths.always)
	paths.normative = unique(paths.normative)
	paths.machine = unique(paths.machine)
	paths.onDemand = unique(paths.onDemand)
	return paths
}

type normativeSectionCatalog map[string][]string

func (a *App) normativeSectionCatalog() (normativeSectionCatalog, error) {
	return loadNormativeSectionCatalog(a.root, normativeSpecificationPaths)
}

func loadNormativeSectionCatalog(root string, paths []string) (normativeSectionCatalog, error) {
	catalog := normativeSectionCatalog{}
	for _, relative := range paths {
		if err := validateBoundedRoutePath(relative); err != nil {
			return nil, fmt.Errorf("normative Section catalog: %w", err)
		}
		data, err := os.ReadFile(filepath.Join(root, filepath.FromSlash(relative)))
		if err != nil {
			return nil, fmt.Errorf("read bounded normative document %s: %w", relative, err)
		}
		sectionIDs, err := normativeStableSectionIDs(data)
		if err != nil {
			return nil, fmt.Errorf("index bounded normative document %s: %w", relative, err)
		}
		for _, sectionID := range sectionIDs {
			catalog[sectionID] = append(catalog[sectionID], relative)
		}
	}
	return catalog, nil
}

func normativeStableSectionIDs(data []byte) ([]string, error) {
	lines := strings.Split(strings.ReplaceAll(string(data), "\r\n", "\n"), "\n")
	if len(lines) == 0 || strings.TrimSpace(lines[0]) != "---" {
		return nil, fmt.Errorf("normative document lacks YAML front matter")
	}
	frontMatter := true
	frontMatterClosed := false
	rootIDs := 0
	var sectionIDs []string
	for index, raw := range lines {
		line := strings.TrimSpace(raw)
		if index == 0 {
			continue
		}
		if frontMatter && line == "---" {
			frontMatter = false
			frontMatterClosed = true
			continue
		}
		if frontMatter && strings.HasPrefix(line, "document_id:") {
			sectionID := strings.Trim(strings.TrimSpace(strings.TrimPrefix(line, "document_id:")), `"'`)
			if sectionID != "" {
				sectionIDs = append(sectionIDs, sectionID)
				rootIDs++
			}
		}
		for _, prefix := range []string{"// x-section-id:", "# x-section-id:"} {
			if strings.HasPrefix(line, prefix) {
				sectionID := strings.Trim(strings.TrimSpace(strings.TrimPrefix(line, prefix)), `"'`)
				if sectionID != "" {
					sectionIDs = append(sectionIDs, sectionID)
				}
			}
		}
		if strings.HasPrefix(line, `<a id="`) && strings.HasSuffix(line, `"></a>`) {
			sectionID := strings.TrimSuffix(strings.TrimPrefix(line, `<a id="`), `"></a>`)
			if sectionID != "" {
				sectionIDs = append(sectionIDs, sectionID)
			}
		}
	}
	if !frontMatterClosed {
		return nil, fmt.Errorf("normative document has unterminated YAML front matter")
	}
	if rootIDs == 0 {
		return nil, fmt.Errorf("normative document has no document_id")
	}
	return sectionIDs, nil
}

func resolveNormativeSection(catalog normativeSectionCatalog, sectionID string) (routeSpec, error) {
	candidates := catalog[sectionID]
	if len(candidates) != 1 {
		locations := append([]string(nil), candidates...)
		sort.Strings(locations)
		if len(locations) == 0 {
			return routeSpec{}, fmt.Errorf("stable Section ID must resolve exactly once in bounded normative catalog, found 0")
		}
		return routeSpec{}, fmt.Errorf("stable Section ID must resolve exactly once in bounded normative catalog, found %d in %s", len(locations), strings.Join(locations, ", "))
	}
	return route(candidates[0], sectionID, "normative-section"), nil
}
