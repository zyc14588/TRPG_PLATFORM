// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package projectctl

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"regexp"
	"strings"
	"time"

	"gopkg.in/yaml.v3"
)

var commitPattern = regexp.MustCompile(`^[0-9a-f]{40}$`)

type readingMap struct {
	RouteSchemaVersion  int         `yaml:"route_schema_version"`
	Mode                string      `yaml:"mode"`
	Milestone           string      `yaml:"milestone"`
	SourceCommit        string      `yaml:"source_commit"`
	SourceTree          string      `yaml:"source_tree"`
	GeneratedUTC        string      `yaml:"generated_utc"`
	AlwaysRead          []routeFile `yaml:"always_read"`
	NormativeReferences []routeFile `yaml:"normative_references"`
	MachineContracts    []routeFile `yaml:"machine_contracts"`
	ReadOnDemand        []routeFile `yaml:"read_on_demand"`
	MustNotBulkRead     []string    `yaml:"must_not_bulk_read"`
	SoftContextRatio    float64     `yaml:"soft_context_ratio"`
	HardContextRatio    float64     `yaml:"hard_context_ratio"`
}

type routeFile struct {
	Path   string `yaml:"path"`
	SHA256 string `yaml:"sha256"`
}

func (a *App) codexCommand(ctx context.Context, args []string) error {
	if len(args) == 1 && args[0] == "plan" {
		return a.generateCodexRoute(ctx, "PLAN")
	}
	if len(args) == 1 && args[0] == "check" {
		return a.checkCodex(ctx)
	}
	if len(args) == 3 && args[0] == "route" && args[1] == "--mode" {
		return a.generateCodexRoute(ctx, strings.ToUpper(args[2]))
	}
	return usageError("codex plan|route --mode PLAN|IMPLEMENT|ACCEPT|REPAIR|check")
}

func (a *App) generateCodexRoute(ctx context.Context, mode string) error {
	allowed := map[string]bool{"PLAN": true, "IMPLEMENT": true, "ACCEPT": true, "REPAIR": true}
	if !allowed[mode] {
		return fmt.Errorf("invalid Codex mode %q", mode)
	}
	if err := a.requireCleanWorktree(ctx); err != nil {
		return err
	}
	if err := a.checkCodexStatic(); err != nil {
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

	paths := routePaths(mode)
	mapFile := readingMap{
		RouteSchemaVersion: 1,
		Mode:               mode,
		Milestone:          "M0",
		SourceCommit:       commit,
		SourceTree:         tree,
		GeneratedUTC:       time.Now().UTC().Format(time.RFC3339),
		MustNotBulkRead:    []string{"docs/**", "git-history", "prior-chat-transcripts"},
		SoftContextRatio:   0.55,
		HardContextRatio:   0.70,
	}
	if mapFile.AlwaysRead, err = a.hashRouteFiles(paths.always); err != nil {
		return err
	}
	if mapFile.NormativeReferences, err = a.hashRouteFiles(paths.normative); err != nil {
		return err
	}
	if mapFile.MachineContracts, err = a.hashRouteFiles(paths.machine); err != nil {
		return err
	}
	if mapFile.ReadOnDemand, err = a.hashRouteFiles(paths.onDemand); err != nil {
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
	fmt.Fprintf(a.stdout, "[ROUTE] %s -> .codex/runtime/READING_MAP.yaml (%s)\n", mode, commit)
	return nil
}

type codexRoutePaths struct {
	always    []string
	normative []string
	machine   []string
	onDemand  []string
}

func routePaths(mode string) codexRoutePaths {
	paths := codexRoutePaths{
		always: []string{
			".codex/SESSION_START.md",
			".codex/routes/" + mode + ".md",
			".codex/state/PROJECT_SNAPSHOT.md",
			".codex/state/MILESTONE_STATUS.md",
		},
		normative: []string{
			"docs/00-governance/DOCUMENT_AUTHORITY.md",
			"docs/00-governance/IMPLEMENTATION_GOVERNANCE.md",
			"docs/80-roadmap/M0_SCOPE_AND_EXIT_GATE.md",
			".codex/state/DECISION_DIGEST.md",
		},
		machine: []string{
			"docs/70-decisions/DECISION_REGISTER.yaml",
			"docs/90-traceability/REQUIREMENTS.yaml",
			"docs/90-traceability/TEST_CATALOG.yaml",
			"docs/90-traceability/TRACEABILITY.yaml",
		},
	}
	switch mode {
	case "PLAN":
		paths.always = append(paths.always, ".codex/planning/AUTONOMOUS_PLANNING_POLICY.md", ".codex/state/MILESTONE_PLAN.yaml")
		paths.onDemand = []string{"docs/80-roadmap/V1_MILESTONES.md", "docs/00-governance/CHANGE_CONTROL.md"}
	case "IMPLEMENT":
		paths.onDemand = []string{"docs/60-quality/TEST_STRATEGY.md", "docs/20-architecture/SYSTEM_ARCHITECTURE.md"}
	case "ACCEPT":
		paths.always = append(paths.always, "docs/60-quality/ACCEPTANCE_POLICY.md")
		paths.onDemand = []string{"docs/60-quality/COMPATIBILITY_AND_RELEASE_GATE.md", "docs/90-traceability/TRACEABILITY.md"}
	case "REPAIR":
		paths.onDemand = []string{"docs/00-governance/CHANGE_CONTROL.md", "docs/60-quality/ACCEPTANCE_POLICY.md"}
	}
	return paths
}

func (a *App) hashRouteFiles(paths []string) ([]routeFile, error) {
	files := make([]routeFile, 0, len(paths))
	for _, relative := range paths {
		data, err := os.ReadFile(filepath.Join(a.root, filepath.FromSlash(relative)))
		if err != nil {
			return nil, fmt.Errorf("route source %s: %w", relative, err)
		}
		digest := sha256.Sum256(data)
		files = append(files, routeFile{Path: relative, SHA256: hex.EncodeToString(digest[:])})
	}
	return files, nil
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
	if err := a.checkCodexStatic(); err != nil {
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
	if route.RouteSchemaVersion != 1 || route.Milestone != "M0" || route.SoftContextRatio != 0.55 || route.HardContextRatio != 0.70 {
		return errors.New("Codex runtime route metadata is invalid")
	}
	commit, err := a.capture(ctx, "git", "rev-parse", "HEAD")
	if err != nil {
		return err
	}
	if route.SourceCommit != strings.TrimSpace(commit) {
		return errors.New("Codex runtime route is stale: source commit differs from HEAD; regenerate it")
	}
	allFiles := append(append(append(route.AlwaysRead, route.NormativeReferences...), route.MachineContracts...), route.ReadOnDemand...)
	for _, file := range allFiles {
		data, err := os.ReadFile(filepath.Join(a.root, filepath.FromSlash(file.Path)))
		if err != nil {
			return fmt.Errorf("Codex route source %s: %w", file.Path, err)
		}
		digest := sha256.Sum256(data)
		if hex.EncodeToString(digest[:]) != file.SHA256 {
			return fmt.Errorf("Codex runtime route is stale: %s changed; regenerate it", file.Path)
		}
	}
	fmt.Fprintf(a.stdout, "[PASS] Codex %s route is current at %s\n", route.Mode, route.SourceCommit)
	return nil
}

func (a *App) checkCodexStatic() error {
	required := []string{
		".codex/README.md", ".codex/SESSION_START.md", ".codex/planning/AUTONOMOUS_PLANNING_POLICY.md",
		".codex/routes/PLAN.md", ".codex/routes/IMPLEMENT.md", ".codex/routes/ACCEPT.md", ".codex/routes/REPAIR.md",
		".codex/state/DECISION_DIGEST.md", ".codex/state/MILESTONE_PLAN.yaml", ".codex/state/MILESTONE_STATUS.md", ".codex/state/PROJECT_SNAPSHOT.md",
		".codex/templates/ACCEPT.template.md", ".codex/templates/BATCH_CONTRACT.template.md", ".codex/templates/HANDOFF.template.md",
		".codex/templates/IMPLEMENT.template.md", ".codex/templates/READING_MAP.template.yaml", ".codex/templates/REPAIR.template.md",
	}
	problems := &validationErrors{}
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
				}
				if strings.Contains(text, "{{") {
					problems.add("active Codex file contains template placeholder: %s", relative)
				}
			}
		}
	}
	return problems.err("Codex governance")
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
