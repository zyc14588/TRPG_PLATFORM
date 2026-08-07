// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package projectctl

import (
	"fmt"
	"os"
	"path/filepath"

	"gopkg.in/yaml.v3"
)

type decisionRegister struct {
	RegisterSchemaVersion int            `yaml:"register_schema_version"`
	BaselineID            string         `yaml:"baseline_id"`
	BaselineStatus        string         `yaml:"baseline_status"`
	AuthoritativeLanguage string         `yaml:"authoritative_language"`
	GeneratedAt           string         `yaml:"generated_at"`
	DecisionCount         int            `yaml:"decision_count"`
	StatusValues          []string       `yaml:"status_values"`
	Decisions             []decision     `yaml:"decisions"`
	Supersessions         []supersession `yaml:"supersessions"`
	Notes                 []string       `yaml:"notes"`
}

type decision struct {
	ID                   string   `yaml:"decision_id"`
	Status               string   `yaml:"status"`
	Title                string   `yaml:"title"`
	Statement            string   `yaml:"statement"`
	SourceStage          string   `yaml:"source_stage"`
	Scope                string   `yaml:"scope"`
	ImplementationStatus string   `yaml:"implementation_status"`
	Specifications       []string `yaml:"specifications"`
	Exceptions           []string `yaml:"exceptions"`
	SupersededBy         []string `yaml:"superseded_by"`
}

type supersession struct {
	Superseded       string `yaml:"superseded"`
	SupersededBy     string `yaml:"superseded_by"`
	Effect           string `yaml:"effect"`
	RelationshipType string `yaml:"relationship_type"`
}

type requirementsDocument struct {
	SchemaVersion    int           `yaml:"schema_version"`
	Baseline         string        `yaml:"baseline"`
	RequirementCount int           `yaml:"requirement_count"`
	Requirements     []requirement `yaml:"requirements"`
}

type requirement struct {
	ID              string   `yaml:"requirement_id"`
	Title           string   `yaml:"title"`
	Statement       string   `yaml:"statement"`
	SourceDecisions []string `yaml:"source_decisions"`
	OwningSpec      string   `yaml:"owning_spec"`
	Milestone       string   `yaml:"milestone"`
	Priority        string   `yaml:"priority"`
	V1Status        string   `yaml:"v1_status"`
}

type testCatalog struct {
	SchemaVersion int        `yaml:"schema_version"`
	Baseline      string     `yaml:"baseline"`
	TestCount     int        `yaml:"test_count"`
	Tests         []testCase `yaml:"tests"`
}

type testCase struct {
	ID               string   `yaml:"test_id"`
	Title            string   `yaml:"title"`
	Level            string   `yaml:"level"`
	Milestone        string   `yaml:"milestone"`
	Status           string   `yaml:"status"`
	Objective        string   `yaml:"objective"`
	RequiredEvidence []string `yaml:"required_evidence"`
}

type traceabilityDocument struct {
	SchemaVersion int            `yaml:"schema_version"`
	Baseline      string         `yaml:"baseline"`
	MappingCount  int            `yaml:"mapping_count"`
	Mappings      []traceMapping `yaml:"mappings"`
}

type traceMapping struct {
	RequirementID        string   `yaml:"requirement_id"`
	SourceDecisions      []string `yaml:"source_decisions"`
	OwningSpec           string   `yaml:"owning_spec"`
	Milestone            string   `yaml:"milestone"`
	TestIDs              []string `yaml:"test_ids"`
	ImplementationStatus string   `yaml:"implementation_status"`
}

func loadYAML[T any](root, relative string) (T, error) {
	var result T
	path := filepath.Join(root, filepath.FromSlash(relative))
	file, err := os.Open(path)
	if err != nil {
		return result, fmt.Errorf("open %s: %w", relative, err)
	}
	defer file.Close()
	decoder := yaml.NewDecoder(file)
	decoder.KnownFields(true)
	if err := decoder.Decode(&result); err != nil {
		return result, fmt.Errorf("parse %s: %w", relative, err)
	}
	return result, nil
}

func (a *App) loadAuthority() (decisionRegister, requirementsDocument, testCatalog, traceabilityDocument, error) {
	decisions, err := loadYAML[decisionRegister](a.root, "docs/70-decisions/DECISION_REGISTER.yaml")
	if err != nil {
		return decisionRegister{}, requirementsDocument{}, testCatalog{}, traceabilityDocument{}, err
	}
	requirements, err := loadYAML[requirementsDocument](a.root, "docs/90-traceability/REQUIREMENTS.yaml")
	if err != nil {
		return decisionRegister{}, requirementsDocument{}, testCatalog{}, traceabilityDocument{}, err
	}
	tests, err := loadYAML[testCatalog](a.root, "docs/90-traceability/TEST_CATALOG.yaml")
	if err != nil {
		return decisionRegister{}, requirementsDocument{}, testCatalog{}, traceabilityDocument{}, err
	}
	trace, err := loadYAML[traceabilityDocument](a.root, "docs/90-traceability/TRACEABILITY.yaml")
	if err != nil {
		return decisionRegister{}, requirementsDocument{}, testCatalog{}, traceabilityDocument{}, err
	}
	return decisions, requirements, tests, trace, nil
}
