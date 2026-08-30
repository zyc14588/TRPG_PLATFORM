// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package projectctl

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"regexp"
	"strings"
)

const maintenanceContractSchemaVersion = 1

// x-section-id: PROJECTCTL-CODEX-MAINTENANCE
var (
	maintenanceIDPattern   = regexp.MustCompile(`^GOV-[A-Z0-9-]+$`)
	maintenanceBootPattern = regexp.MustCompile(`^GOVERNANCE-MAINTENANCE-BOOTSTRAP-[0-9]{3}$`)
)

func maintenanceContractPath(maintenanceID string) string {
	return ".codex/maintenance/" + maintenanceID + "/CONTRACT.yaml"
}

func (a *App) loadGovernanceMaintenanceContract(maintenanceID string) (governanceMaintenanceContract, error) {
	var contract governanceMaintenanceContract
	if !maintenanceIDPattern.MatchString(maintenanceID) {
		return contract, fmt.Errorf("invalid governance maintenance ID %q; expected GOV-[A-Z0-9-]+", maintenanceID)
	}
	relative := maintenanceContractPath(maintenanceID)
	loaded, err := loadYAML[governanceMaintenanceContract](a.root, relative)
	if err != nil {
		if errors.Is(rootCause(err), os.ErrNotExist) {
			return contract, fmt.Errorf("governance maintenance contract %s does not exist", relative)
		}
		return contract, err
	}
	if err := validateGovernanceMaintenanceContract(loaded, maintenanceID); err != nil {
		return contract, fmt.Errorf("governance maintenance contract %s: %w", relative, err)
	}
	for _, reference := range append(append([]maintenanceRouteRef(nil), loaded.NormativeReferences...), loaded.AllowedScope...) {
		data, err := os.ReadFile(filepath.Join(a.root, filepath.FromSlash(reference.Path)))
		if err != nil {
			return contract, fmt.Errorf("governance maintenance reference %s: %w", reference.Path, err)
		}
		if _, err := sectionMaterial(data, reference.SectionID); err != nil {
			return contract, fmt.Errorf("governance maintenance reference %s#%s: %w", reference.Path, reference.SectionID, err)
		}
	}
	return loaded, nil
}

func rootCause(err error) error {
	for {
		unwrapped := errors.Unwrap(err)
		if unwrapped == nil {
			return err
		}
		err = unwrapped
	}
}

// x-section-id: PROJECTCTL-MAINTENANCE-REFERENCE-SCOPE
func validateGovernanceMaintenanceContract(contract governanceMaintenanceContract, requestedID string) error {
	if contract.SchemaVersion != maintenanceContractSchemaVersion {
		return fmt.Errorf("schema_version=%d, want %d", contract.SchemaVersion, maintenanceContractSchemaVersion)
	}
	if contract.MaintenanceID != requestedID || contract.SectionID != requestedID {
		return errors.New("maintenance_id and x-section-id must match the requested target")
	}
	if !maintenanceIDPattern.MatchString(contract.MaintenanceID) {
		return fmt.Errorf("invalid maintenance_id %q", contract.MaintenanceID)
	}
	if contract.Status != "ACTIVE" && contract.Status != "READY_FOR_ACCEPTANCE" {
		return fmt.Errorf("maintenance status %q is not routeable", contract.Status)
	}
	if len(contract.Objective) == 0 || strings.TrimSpace(contract.Reason) == "" || strings.TrimSpace(contract.SourceBlocker) == "" {
		return errors.New("objective, reason, and source_blocker are required")
	}
	if len(contract.AllowedScope) == 0 || len(contract.ForbiddenScope) == 0 || len(contract.NormativeReferences) == 0 ||
		len(contract.Acceptance) == 0 || len(contract.Tests) == 0 || len(contract.StopConditions) == 0 {
		return errors.New("scope, normative references, acceptance, tests, and stop conditions must be non-empty")
	}
	seen := map[string]bool{}
	recordReference := func(reference maintenanceRouteRef) error {
		key := reference.Path + "#" + reference.SectionID
		if seen[key] {
			return fmt.Errorf("maintenance contract repeats route reference %s", key)
		}
		seen[key] = true
		return nil
	}
	for _, reference := range contract.NormativeReferences {
		if err := validateMaintenanceNormativeReference(reference); err != nil {
			return err
		}
		if err := recordReference(reference); err != nil {
			return err
		}
	}
	for _, reference := range contract.AllowedScope {
		if err := validateMaintenanceAllowedScopeReference(reference); err != nil {
			return err
		}
		if err := recordReference(reference); err != nil {
			return err
		}
	}
	bootstrap := contract.Bootstrap
	if !maintenanceBootPattern.MatchString(bootstrap.BootstrapID) || strings.TrimSpace(bootstrap.Authorization) == "" ||
		len(bootstrap.AllowedScope) == 0 || strings.TrimSpace(bootstrap.RetirementCondition) == "" {
		return errors.New("bootstrap audit record is incomplete")
	}
	switch bootstrap.Status {
	case "AUTHORIZED_ONCE":
		if len(bootstrap.RetirementEvidence) != 0 {
			return errors.New("active one-time bootstrap must not claim retirement evidence")
		}
	case "RETIRED":
		if len(bootstrap.RetirementEvidence) == 0 {
			return errors.New("retired bootstrap requires retirement evidence")
		}
	default:
		return fmt.Errorf("invalid bootstrap status %q", bootstrap.Status)
	}
	return nil
}

func validateMaintenanceReferenceShape(reference maintenanceRouteRef) error {
	if err := validateBoundedRoutePath(reference.Path); err != nil {
		return err
	}
	if strings.TrimSpace(reference.SectionID) == "" {
		return fmt.Errorf("maintenance route reference %s has no stable Section ID", reference.Path)
	}
	allowedKinds := map[string]bool{
		"governance-entry": true, "governance-policy": true, "normative-section": true,
		"machine-contract": true, "state-summary": true, "production-code": true,
		"test": true, "generated-reference": true,
	}
	if !allowedKinds[reference.Kind] {
		return fmt.Errorf("maintenance route reference %s#%s has invalid kind %q", reference.Path, reference.SectionID, reference.Kind)
	}
	return nil
}

func isGovernanceMaintenanceWritablePath(path string) bool {
	allowedPath := path == "Justfile"
	for _, prefix := range []string{".codex/", "docs/00-governance/", "docs/70-decisions/", "internal/projectctl/", "cmd/projectctl/", "schemas/codex/"} {
		if strings.HasPrefix(path, prefix) {
			allowedPath = true
		}
	}
	return allowedPath
}

func validateMaintenanceAllowedScopeReference(reference maintenanceRouteRef) error {
	if err := validateMaintenanceReferenceShape(reference); err != nil {
		return err
	}
	if !isGovernanceMaintenanceWritablePath(reference.Path) {
		return fmt.Errorf("governance maintenance scope cannot include product path %q", reference.Path)
	}
	return nil
}

func validateMaintenanceNormativeReference(reference maintenanceRouteRef) error {
	if err := validateMaintenanceReferenceShape(reference); err != nil {
		return err
	}
	if isGovernanceMaintenanceWritablePath(reference.Path) {
		return nil
	}
	if strings.HasPrefix(reference.Path, "docs/") && reference.Kind == "normative-section" {
		return nil
	}
	if strings.HasPrefix(reference.Path, "docs/") {
		return fmt.Errorf("governance maintenance product documentation reference %q must use kind normative-section", reference.Path)
	}
	return fmt.Errorf("governance maintenance normative references cannot include product path %q", reference.Path)
}

func (a *App) maintenanceRoutePaths(request codexRouteRequest) (codexRoutePaths, error) {
	contract, err := a.loadGovernanceMaintenanceContract(request.MaintenanceID)
	if err != nil {
		return codexRoutePaths{}, err
	}
	modeDocumentID := map[string]string{"ACCEPT": "CODEX-ROUTE-ACCEPT", "REPAIR": "CODEX-ROUTE-REPAIR"}[request.Mode]
	paths := codexRoutePaths{}
	seen := map[string]bool{}
	appendUnique := func(target *[]routeSpec, specs ...routeSpec) {
		for _, spec := range specs {
			key := spec.path + "#" + spec.sectionID
			if seen[key] {
				continue
			}
			seen[key] = true
			*target = append(*target, spec)
		}
	}
	appendUnique(&paths.always,
		route(".codex/SESSION_START.md", "CODEX-SESSION-START", "governance-entry"),
		route(".codex/routes/"+request.Mode+".md", modeDocumentID, "mode-policy"),
		route(".codex/state/PROJECT_SNAPSHOT.md", "CODEX-PROJECT-SNAPSHOT", "state-summary"),
		route(".codex/state/MILESTONE_STATUS.md", "CODEX-MILESTONE-STATUS", "state-summary"),
		route(maintenanceContractPath(request.MaintenanceID), request.MaintenanceID, "machine-contract"),
	)
	appendUnique(&paths.normative,
		route("docs/00-governance/DOCUMENT_AUTHORITY.md", "SPEC-DOCUMENT-AUTHORITY-READING", "normative-section"),
		route("docs/00-governance/CODEX_PROGRESSIVE_DISCLOSURE.md", "SPEC-CODEX-ENTRY", "normative-section"),
		route("docs/00-governance/CODEX_PROGRESSIVE_DISCLOSURE.md", "SPEC-CODEX-ROUTE", "normative-section"),
		route("docs/00-governance/CODEX_PROGRESSIVE_DISCLOSURE.md", "SPEC-CODEX-BUDGET", "normative-section"),
		route("docs/00-governance/CODEX_PROGRESSIVE_DISCLOSURE.md", "SPEC-CODEX-MAINTENANCE", "normative-section"),
	)
	for _, reference := range contract.NormativeReferences {
		appendUnique(&paths.normative, route(reference.Path, reference.SectionID, reference.Kind))
	}
	appendUnique(&paths.machine,
		route("schemas/codex/reading-map-v4.schema.json", "SCHEMA-CODEX-READING-MAP-V4", "machine-contract"),
		route("schemas/codex/governance-maintenance-contract-v1.schema.json", "SCHEMA-CODEX-GOVERNANCE-MAINTENANCE-V1", "machine-contract"),
		route("schemas/codex/milestone-plan-v2.schema.json", "SCHEMA-CODEX-MILESTONE-PLAN-V2", "machine-contract"),
		route(".codex/state/MILESTONE_PLAN.yaml", "M1-MILESTONE-PLAN", "machine-contract"),
	)
	for _, decisionID := range []string{"R25-A01", "R25-A02", "R25-A03", "R25-A04", "R26-A01", "R26-A02", "R26-A03", "R26-A04"} {
		appendUnique(&paths.machine, route("docs/70-decisions/DECISION_REGISTER.yaml", decisionID, "machine-contract"))
	}
	for _, reference := range contract.AllowedScope {
		appendUnique(&paths.onDemand, route(reference.Path, reference.SectionID, reference.Kind))
	}
	return paths, nil
}
