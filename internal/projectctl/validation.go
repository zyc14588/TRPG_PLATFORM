// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package projectctl

import (
	"bufio"
	"errors"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"regexp"
	"slices"
	"strings"
)

var (
	decisionIDPattern    = regexp.MustCompile(`^R[0-9]+-A[0-9]{2}$`)
	requirementIDPattern = regexp.MustCompile(`^REQ-[A-Z0-9-]+$`)
	testIDPattern        = regexp.MustCompile(`^TEST-[A-Z0-9-]+$`)
)

type validationErrors struct {
	items []string
}

func (v *validationErrors) add(format string, values ...any) {
	v.items = append(v.items, fmt.Sprintf(format, values...))
}

func (v *validationErrors) err(label string) error {
	if len(v.items) == 0 {
		return nil
	}
	return fmt.Errorf("%s validation failed:\n- %s", label, strings.Join(v.items, "\n- "))
}

// x-section-id: PROJECTCTL-DECISION-VALIDATION
func validateDecisions(register decisionRegister) error {
	problems := &validationErrors{}
	if register.RegisterSchemaVersion != 1 {
		problems.add("register_schema_version is %d, want 1", register.RegisterSchemaVersion)
	}
	if register.BaselineID != "TRPG-PLATFORM-R0-R26" {
		problems.add("unexpected baseline_id %q", register.BaselineID)
	}
	if register.BaselineStatus != "ACTIVE_GOVERNANCE" {
		problems.add("unexpected baseline_status %q", register.BaselineStatus)
	}
	if register.DecisionCount != len(register.Decisions) {
		problems.add("decision_count=%d but found %d entries", register.DecisionCount, len(register.Decisions))
	}
	if len(register.Decisions) != 463 {
		problems.add("R0-R26 decision register must contain 463 decisions, found %d", len(register.Decisions))
	}

	allowedStatus := map[string]bool{"ACTIVE": true, "SUPERSEDED": true, "DEFERRED": true, "REJECTED": true}
	decisions := make(map[string]decision, len(register.Decisions))
	for _, item := range register.Decisions {
		if !decisionIDPattern.MatchString(item.ID) {
			problems.add("invalid decision_id %q", item.ID)
		}
		if _, duplicate := decisions[item.ID]; duplicate {
			problems.add("duplicate decision_id %s", item.ID)
		}
		decisions[item.ID] = item
		if !allowedStatus[item.Status] {
			problems.add("%s has invalid status %q", item.ID, item.Status)
		}
		stage := strings.SplitN(item.ID, "-", 2)[0]
		if item.SourceStage != stage {
			problems.add("%s source_stage=%q, want %q", item.ID, item.SourceStage, stage)
		}
		if strings.TrimSpace(item.Title) == "" || strings.TrimSpace(item.Statement) == "" || strings.TrimSpace(item.Scope) == "" {
			problems.add("%s has an empty title, statement, or scope", item.ID)
		}
		if len(item.Specifications) == 0 {
			problems.add("%s has no specification mapping", item.ID)
		}
	}

	supersededTargets := map[string]bool{}
	for index, relation := range register.Supersessions {
		if _, exists := decisions[relation.SupersededBy]; !exists {
			problems.add("supersession[%d] references missing superseded_by %s", index, relation.SupersededBy)
		}
		if relation.RelationshipType == "SUPERSEDES" {
			if target, exists := decisions[relation.Superseded]; exists {
				supersededTargets[target.ID] = true
				if target.Status != "SUPERSEDED" {
					problems.add("%s is targeted by SUPERSEDES but status is %s", target.ID, target.Status)
				}
			}
		}
		if strings.TrimSpace(relation.Effect) == "" || strings.TrimSpace(relation.RelationshipType) == "" {
			problems.add("supersession[%d] is incomplete", index)
		}
	}
	for _, item := range register.Decisions {
		if item.Status == "SUPERSEDED" && !supersededTargets[item.ID] {
			problems.add("%s is SUPERSEDED without an internal SUPERSEDES relation", item.ID)
		}
	}
	return problems.err("decision register")
}

func (a *App) checkDecisions() error {
	register, err := loadYAML[decisionRegister](a.root, "docs/70-decisions/DECISION_REGISTER.yaml")
	if err != nil {
		return err
	}
	if err := validateDecisions(register); err != nil {
		return err
	}
	fmt.Fprintf(a.stdout, "[PASS] decision register: %d decisions\n", len(register.Decisions))
	return nil
}

func (a *App) validateTraceability(decisions decisionRegister, requirements requirementsDocument, tests testCatalog, trace traceabilityDocument) error {
	problems := &validationErrors{}
	if requirements.SchemaVersion != 1 || tests.SchemaVersion != 1 || trace.SchemaVersion != 1 {
		problems.add("all traceability schema versions must equal 1")
	}
	if requirements.Baseline != "R0-R24" || tests.Baseline != "R0-R24" || trace.Baseline != "R0-R24" {
		problems.add("all traceability baselines must equal R0-R24")
	}
	if requirements.RequirementCount != len(requirements.Requirements) {
		problems.add("requirement_count=%d but found %d", requirements.RequirementCount, len(requirements.Requirements))
	}
	if tests.TestCount != len(tests.Tests) {
		problems.add("test_count=%d but found %d", tests.TestCount, len(tests.Tests))
	}
	if trace.MappingCount != len(trace.Mappings) {
		problems.add("mapping_count=%d but found %d", trace.MappingCount, len(trace.Mappings))
	}
	if len(requirements.Requirements) != 77 || len(tests.Tests) != 77 || len(trace.Mappings) != 77 {
		problems.add("frozen baseline requires 77 requirements, tests, and mappings (found %d/%d/%d)", len(requirements.Requirements), len(tests.Tests), len(trace.Mappings))
	}

	decisionIDs := make(map[string]bool, len(decisions.Decisions))
	for _, item := range decisions.Decisions {
		decisionIDs[item.ID] = true
	}
	documentIDs, err := collectDocumentIDs(a.root)
	if err != nil {
		return err
	}

	requirementsByID := make(map[string]requirement, len(requirements.Requirements))
	for _, item := range requirements.Requirements {
		if !requirementIDPattern.MatchString(item.ID) {
			problems.add("invalid requirement_id %q", item.ID)
		}
		if _, duplicate := requirementsByID[item.ID]; duplicate {
			problems.add("duplicate requirement_id %s", item.ID)
		}
		requirementsByID[item.ID] = item
		if !documentIDs[item.OwningSpec] {
			problems.add("%s references missing owning_spec %s", item.ID, item.OwningSpec)
		}
		for _, decisionID := range item.SourceDecisions {
			if !decisionIDs[decisionID] {
				problems.add("%s references missing decision %s", item.ID, decisionID)
			}
		}
	}

	testsByID := make(map[string]testCase, len(tests.Tests))
	for _, item := range tests.Tests {
		if !testIDPattern.MatchString(item.ID) {
			problems.add("invalid test_id %q", item.ID)
		}
		if _, duplicate := testsByID[item.ID]; duplicate {
			problems.add("duplicate test_id %s", item.ID)
		}
		testsByID[item.ID] = item
		if item.Status != "PLANNED" {
			problems.add("%s status=%q, want PLANNED at M0", item.ID, item.Status)
		}
		if len(item.RequiredEvidence) == 0 {
			problems.add("%s has no required_evidence", item.ID)
		}
	}

	mappingsByRequirement := make(map[string]traceMapping, len(trace.Mappings))
	testUsage := map[string]int{}
	for _, mapping := range trace.Mappings {
		if _, duplicate := mappingsByRequirement[mapping.RequirementID]; duplicate {
			problems.add("duplicate mapping for %s", mapping.RequirementID)
		}
		mappingsByRequirement[mapping.RequirementID] = mapping
		requirement, exists := requirementsByID[mapping.RequirementID]
		if !exists {
			problems.add("mapping references missing requirement %s", mapping.RequirementID)
			continue
		}
		if mapping.OwningSpec != requirement.OwningSpec || mapping.Milestone != requirement.Milestone {
			problems.add("%s mapping spec/milestone differs from requirement", mapping.RequirementID)
		}
		if !slices.Equal(mapping.SourceDecisions, requirement.SourceDecisions) {
			problems.add("%s mapping source_decisions drift", mapping.RequirementID)
		}
		if mapping.ImplementationStatus != "NOT_STARTED" {
			problems.add("%s implementation_status=%q, want NOT_STARTED at M0", mapping.RequirementID, mapping.ImplementationStatus)
		}
		if len(mapping.TestIDs) == 0 {
			problems.add("%s has no test mapping", mapping.RequirementID)
		}
		for _, testID := range mapping.TestIDs {
			test, exists := testsByID[testID]
			if !exists {
				problems.add("%s references missing test %s", mapping.RequirementID, testID)
				continue
			}
			testUsage[testID]++
			if test.Milestone != mapping.Milestone {
				problems.add("%s milestone differs from %s", testID, mapping.RequirementID)
			}
		}
	}
	for id := range requirementsByID {
		if _, exists := mappingsByRequirement[id]; !exists {
			problems.add("missing mapping for %s", id)
		}
	}
	for id := range testsByID {
		if testUsage[id] == 0 {
			problems.add("unmapped test %s", id)
		}
	}
	return problems.err("traceability")
}

func (a *App) checkTraceability() error {
	decisions, requirements, tests, trace, err := a.loadAuthority()
	if err != nil {
		return err
	}
	if err := validateDecisions(decisions); err != nil {
		return err
	}
	if err := a.validateTraceability(decisions, requirements, tests, trace); err != nil {
		return err
	}
	fmt.Fprintf(a.stdout, "[PASS] traceability: %d requirements, %d tests, %d mappings\n", len(requirements.Requirements), len(tests.Tests), len(trace.Mappings))
	return nil
}

func collectDocumentIDs(root string) (map[string]bool, error) {
	ids := map[string]bool{}
	docsRoot := filepath.Join(root, "docs")
	err := filepath.WalkDir(docsRoot, func(path string, entry fs.DirEntry, walkErr error) error {
		if walkErr != nil {
			return walkErr
		}
		if entry.IsDir() || filepath.Ext(path) != ".md" {
			return nil
		}
		file, err := os.Open(path)
		if err != nil {
			return err
		}
		defer file.Close()
		scanner := bufio.NewScanner(file)
		for scanner.Scan() {
			line := strings.TrimSpace(scanner.Text())
			if strings.HasPrefix(line, "document_id:") {
				id := strings.Trim(strings.TrimSpace(strings.TrimPrefix(line, "document_id:")), `"'`)
				if id != "" {
					ids[id] = true
				}
				break
			}
		}
		return scanner.Err()
	})
	if err != nil {
		return nil, fmt.Errorf("collect document IDs: %w", err)
	}
	if len(ids) == 0 {
		return nil, errors.New("no document IDs found")
	}
	return ids, nil
}
