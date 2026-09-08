// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package projectctl

import (
	"bytes"
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io/fs"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"runtime"
	"sort"
	"strings"

	"gopkg.in/yaml.v3"
)

const (
	spdxIdentifier     = "SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0"
	officialLicenseSHA = "ffcca38841adb694b6f380647e15f17c446a4d1656fed51a1e2041d064c94cc8"
)

type toolchainLock struct {
	SPDXLicenseIdentifier string `json:"spdx_license_identifier"`
	SchemaVersion         int    `json:"schema_version"`
	LockedAt              string `json:"locked_at"`
	Tools                 struct {
		Go    lockedTool `json:"go"`
		Node  lockedTool `json:"node"`
		PNPM  lockedTool `json:"pnpm"`
		Just  lockedTool `json:"just"`
		Wails lockedTool `json:"wails"`
	} `json:"tools"`
	Frontend   map[string]lockedTool      `json:"frontend"`
	Actions    map[string]lockedTool      `json:"actions"`
	Containers map[string]lockedContainer `json:"containers"`
}

type lockedTool struct {
	Version     string `json:"version"`
	License     string `json:"license"`
	Source      string `json:"source"`
	ReleaseLine string `json:"release_line"`
	Integrity   string `json:"integrity"`
	Stability   string `json:"stability"`
	SHA         string `json:"sha"`
}

type lockedContainer struct {
	Reference              string                     `json:"reference"`
	Role                   string                     `json:"role"`
	License                string                     `json:"license"`
	DistributionComponents []lockedContainerComponent `json:"distribution_components"`
}

type lockedContainerComponent struct {
	Name    string `json:"name"`
	Version string `json:"version"`
	License string `json:"license"`
}

func (a *App) loadToolchain() (toolchainLock, error) {
	var lock toolchainLock
	data, err := os.ReadFile(filepath.Join(a.root, "tools", "toolchain.lock.json"))
	if err != nil {
		return lock, fmt.Errorf("read toolchain lock: %w", err)
	}
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(&lock); err != nil {
		return lock, fmt.Errorf("parse toolchain lock: %w", err)
	}
	if lock.SchemaVersion != 1 || lock.SPDXLicenseIdentifier != "PolyForm-Noncommercial-1.0.0" {
		return lock, errors.New("invalid toolchain lock metadata")
	}
	if err := a.checkPinnedFiles(lock); err != nil {
		return lock, err
	}
	return lock, nil
}

func (a *App) checkPinnedFiles(lock toolchainLock) error {
	problems := &validationErrors{}
	checkTextFile := func(relative, expected string) {
		data, err := os.ReadFile(filepath.Join(a.root, filepath.FromSlash(relative)))
		if err != nil {
			problems.add("read %s: %v", relative, err)
			return
		}
		if normalizePinnedText(data) != expected {
			problems.add("%s does not match toolchain lock", relative)
		}
	}
	checkTextFile(".go-version", lock.Tools.Go.Version)
	checkTextFile(".node-version", lock.Tools.Node.Version)
	checkTextFile(".tool-versions", fmt.Sprintf("golang %s\nnodejs %s\njust %s", lock.Tools.Go.Version, lock.Tools.Node.Version, lock.Tools.Just.Version))
	checkTextFile(".gitattributes", "# SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0\n\n* text=auto eol=lf\n\n*.gif binary\n*.ico binary\n*.jpg binary\n*.jpeg binary\n*.png binary\n*.webp binary\n*.woff binary\n*.woff2 binary")

	goMod, err := os.ReadFile(filepath.Join(a.root, "go.mod"))
	if err != nil {
		problems.add("read go.mod: %v", err)
	} else {
		text := string(goMod)
		if !strings.Contains(text, "toolchain go"+lock.Tools.Go.Version) {
			problems.add("go.mod toolchain does not pin Go %s", lock.Tools.Go.Version)
		}
		if !strings.Contains(text, "github.com/wailsapp/wails/v2 v"+lock.Tools.Wails.Version) {
			problems.add("go.mod does not pin Wails %s", lock.Tools.Wails.Version)
		}
	}

	var rootPackage struct {
		PackageManager string            `json:"packageManager"`
		Engines        map[string]string `json:"engines"`
	}
	rootPackageData, err := os.ReadFile(filepath.Join(a.root, "package.json"))
	if err != nil || json.Unmarshal(rootPackageData, &rootPackage) != nil {
		problems.add("root package.json is unreadable")
	} else {
		if rootPackage.PackageManager != "pnpm@"+lock.Tools.PNPM.Version || rootPackage.Engines["node"] != lock.Tools.Node.Version || rootPackage.Engines["pnpm"] != lock.Tools.PNPM.Version {
			problems.add("root package manager/engine pins differ from toolchain lock")
		}
	}

	for _, relative := range []string{"apps/web-player/package.json", "apps/creator-studio/frontend/package.json"} {
		var manifest struct {
			Dependencies    map[string]string `json:"dependencies"`
			DevDependencies map[string]string `json:"devDependencies"`
		}
		data, readErr := os.ReadFile(filepath.Join(a.root, filepath.FromSlash(relative)))
		if readErr != nil || json.Unmarshal(data, &manifest) != nil {
			problems.add("%s is unreadable", relative)
			continue
		}
		for name, version := range manifest.Dependencies {
			locked, exists := lock.Frontend[name]
			if !exists || version != locked.Version {
				problems.add("%s dependency %s@%s differs from toolchain lock", relative, name, version)
			}
		}
		for name, version := range manifest.DevDependencies {
			locked, exists := lock.Frontend[name]
			if !exists || version != locked.Version {
				problems.add("%s dependency %s@%s differs from toolchain lock", relative, name, version)
			}
		}
	}
	if err := a.checkPinnedWorkflows(lock, problems); err != nil {
		return err
	}

	dockerfile, err := os.ReadFile(filepath.Join(a.root, "deploy", "docker", "Dockerfile"))
	if err != nil {
		problems.add("read deploy/docker/Dockerfile: %v", err)
	} else if err := validateContainerSupplyChain(lock, dockerfile); err != nil {
		problems.add("%v", err)
	}
	return problems.err("toolchain lock")
}

func validateContainerSupplyChain(lock toolchainLock, dockerfile []byte) error {
	problems := &validationErrors{}
	var finalName string
	var final lockedContainer
	for name, container := range lock.Containers {
		switch container.Role {
		case "build-only":
			if !strings.Contains(container.Reference, "@sha256:") {
				problems.add("build-only container %s lacks immutable digest", name)
			}
		case "final-runtime":
			if finalName != "" {
				problems.add("multiple final-runtime containers: %s and %s", finalName, name)
			}
			finalName, final = name, container
		default:
			problems.add("container %s has invalid role %q", name, container.Role)
		}
	}
	if finalName == "" {
		problems.add("container lock has no final-runtime entry")
		return problems.err("container supply chain")
	}

	fromReferences := dockerFromReferences(string(dockerfile))
	if len(fromReferences) == 0 {
		problems.add("Dockerfile has no FROM instruction")
	} else if got := fromReferences[len(fromReferences)-1]; got != final.Reference {
		problems.add("Dockerfile final FROM is %q, lock final-runtime %s is %q", got, finalName, final.Reference)
	}
	for name, container := range lock.Containers {
		found := false
		for _, reference := range fromReferences {
			if reference == container.Reference {
				found = true
				break
			}
		}
		if !found {
			problems.add("Dockerfile does not contain locked %s image %q", name, container.Reference)
		}
	}

	if final.Reference == "scratch" {
		if final.License != "NONE" {
			problems.add("scratch final runtime license must be NONE, got %q", final.License)
		}
		if len(final.DistributionComponents) != 0 {
			problems.add("scratch final runtime must have an empty distribution component inventory")
		}
	} else {
		if !strings.Contains(final.Reference, "@sha256:") {
			problems.add("final runtime %s lacks immutable digest", finalName)
		}
		if len(final.DistributionComponents) == 0 {
			problems.add("non-scratch final runtime requires a component-level license inventory; aggregate label %q is insufficient", final.License)
		}
	}
	for _, component := range final.DistributionComponents {
		if component.Name == "" || component.Version == "" || component.License == "" {
			problems.add("final runtime component inventory contains an incomplete record")
			continue
		}
		if prohibitedDistributionLicense(component.License) {
			problems.add("final runtime component %s@%s has prohibited license %s", component.Name, component.Version, component.License)
		}
	}
	return problems.err("container supply chain")
}

func dockerFromReferences(dockerfile string) []string {
	var references []string
	for _, line := range strings.Split(dockerfile, "\n") {
		fields := strings.Fields(strings.TrimSpace(line))
		if len(fields) >= 2 && strings.EqualFold(fields[0], "FROM") {
			references = append(references, fields[1])
		}
	}
	return references
}

func prohibitedDistributionLicense(identifier string) bool {
	upper := strings.ToUpper(strings.TrimSpace(identifier))
	return strings.HasPrefix(upper, "GPL-") || strings.HasPrefix(upper, "AGPL-") ||
		strings.HasPrefix(upper, "SSPL-") || strings.HasPrefix(upper, "BUSL-") ||
		upper == "ELASTIC-2.0" || strings.Contains(upper, "COMMONS-CLAUSE")
}

func normalizePinnedText(data []byte) string {
	return strings.TrimSpace(strings.ReplaceAll(string(data), "\r\n", "\n"))
}

func (a *App) checkPinnedWorkflows(lock toolchainLock, problems *validationErrors) error {
	workflowRoot := filepath.Join(a.root, ".github", "workflows")
	if _, err := os.Stat(workflowRoot); errors.Is(err, os.ErrNotExist) {
		return nil
	} else if err != nil {
		return err
	}
	usesPattern := regexp.MustCompile(`(?m)^\s*uses:\s*([^@\s]+)@([^\s#]+)`)
	used := map[string]bool{}
	err := filepath.WalkDir(workflowRoot, func(path string, entry fs.DirEntry, walkErr error) error {
		if walkErr != nil {
			return walkErr
		}
		if entry.IsDir() || filepath.Ext(path) != ".yml" && filepath.Ext(path) != ".yaml" {
			return nil
		}
		data, err := os.ReadFile(path)
		if err != nil {
			return err
		}
		var syntax yaml.Node
		if err := yaml.Unmarshal(data, &syntax); err != nil {
			return fmt.Errorf("parse workflow %s: %w", path, err)
		}
		for _, match := range usesPattern.FindAllStringSubmatch(string(data), -1) {
			name, revision := match[1], match[2]
			locked, exists := lock.Actions[name]
			if !exists {
				problems.add("workflow uses unregistered action %s", name)
				continue
			}
			used[name] = true
			if revision != locked.SHA || !commitPattern.MatchString(revision) {
				problems.add("workflow action %s is not pinned to locked SHA %s", name, locked.SHA)
			}
		}
		text := string(data)
		for _, forbidden := range []string{"go test ", "go build ", "go vet ", "go run ", "pnpm -r ", "pnpm run "} {
			if strings.Contains(text, forbidden) {
				problems.add("workflow duplicates project logic with %q; call a Just target", forbidden)
			}
		}
		return nil
	})
	if err != nil {
		return fmt.Errorf("check workflow pins: %w", err)
	}
	for name, action := range lock.Actions {
		if !commitPattern.MatchString(action.SHA) {
			problems.add("action %s has invalid immutable SHA %q", name, action.SHA)
		}
		if !used[name] {
			problems.add("locked action %s is not used by an M0 workflow", name)
		}
	}
	return nil
}

func (a *App) bootstrap(ctx context.Context, args []string) error {
	if len(args) != 0 {
		return usageError("bootstrap")
	}
	if err := a.runLogged(ctx, "go", "mod", "download"); err != nil {
		return err
	}
	if err := a.runLogged(ctx, "pnpm", "install", "--frozen-lockfile"); err != nil {
		return err
	}
	fmt.Fprintln(a.stdout, "[PASS] bootstrap dependencies are present")
	return nil
}

func (a *App) checkEnvironment(ctx context.Context) error {
	lock, err := a.loadToolchain()
	if err != nil {
		return err
	}
	checks := []struct {
		name     string
		command  string
		args     []string
		expected string
		trim     func(string) string
	}{
		{name: "Go", command: "go", args: []string{"env", "GOVERSION"}, expected: lock.Tools.Go.Version, trim: func(value string) string { return strings.TrimPrefix(strings.TrimSpace(value), "go") }},
		{name: "Node", command: "node", args: []string{"--version"}, expected: lock.Tools.Node.Version, trim: func(value string) string { return strings.TrimPrefix(strings.TrimSpace(value), "v") }},
		{name: "pnpm", command: "pnpm", args: []string{"--version"}, expected: lock.Tools.PNPM.Version, trim: strings.TrimSpace},
		{name: "Just", command: "just", args: []string{"--version"}, expected: lock.Tools.Just.Version, trim: func(value string) string { return strings.TrimPrefix(strings.TrimSpace(value), "just ") }},
	}
	for _, check := range checks {
		output, runErr := a.capture(ctx, check.command, check.args...)
		if runErr != nil {
			return fmt.Errorf("%s unavailable: %w", check.name, runErr)
		}
		actual := check.trim(output)
		if actual != check.expected {
			return fmt.Errorf("%s version %q, want %q", check.name, actual, check.expected)
		}
		fmt.Fprintf(a.stdout, "[PASS] %s %s\n", check.name, actual)
	}
	fmt.Fprintf(a.stdout, "[PASS] Wails %s (validated module lock)\n", lock.Tools.Wails.Version)
	return nil
}

func (a *App) checkAll(ctx context.Context) error {
	steps := []struct {
		name string
		run  func() error
	}{
		{name: "docs", run: a.checkDocuments},
		{name: "decisions", run: a.checkDecisions},
		{name: "traceability", run: a.checkTraceability},
		{name: "license", run: func() error { return a.checkLicense(ctx) }},
		{name: "scope M0", run: func() error { return a.checkScope(ctx) }},
	}
	if _, err := os.Stat(filepath.Join(a.root, ".codex", "SESSION_START.md")); err == nil {
		steps = append(steps, struct {
			name string
			run  func() error
		}{name: "Codex governance", run: func() error { return a.checkCodex(ctx) }})
	}
	for _, step := range steps {
		fmt.Fprintf(a.stdout, "[CHECK] %s\n", step.name)
		if err := step.run(); err != nil {
			return err
		}
	}
	fmt.Fprintln(a.stdout, "[PASS] repository checks")
	return nil
}

// x-section-id: PROJECTCTL-PLATFORM-CI
type platformCIProfileName string

const (
	platformProfileLinuxCore      platformCIProfileName = "linux-core"
	platformProfileWindowsProduct platformCIProfileName = "windows-product"
	platformProfileMacOSProduct   platformCIProfileName = "macos-product"
)

type nativeStudioProfile string

const (
	nativeStudioLinux   nativeStudioProfile = "linux"
	nativeStudioWindows nativeStudioProfile = "windows"
)

type platformCIStepKind string

const (
	platformStepCommand      platformCIStepKind = "command"
	platformStepGoFormat     platformCIStepKind = "gofmt"
	platformStepNativeStudio platformCIStepKind = "native-studio"
	platformStepCompose      platformCIStepKind = "compose"
)

type platformCIStep struct {
	surface      string
	kind         platformCIStepKind
	name         string
	args         []string
	nativeStudio nativeStudioProfile
}

type platformCIPlan struct {
	profile    platformCIProfileName
	policy     bool
	buildSteps []platformCIStep
	testSteps  []platformCIStep
}

func commandPlatformStep(surface, name string, args ...string) platformCIStep {
	return platformCIStep{surface: surface, kind: platformStepCommand, name: name, args: args}
}

func frontendBuildSteps() []platformCIStep {
	return []platformCIStep{
		commandPlatformStep("Web Player and Creator Studio frontend typecheck", "pnpm", "-r", "typecheck"),
		commandPlatformStep("Web Player and Creator Studio frontend build", "pnpm", "-r", "build"),
	}
}

func frontendTestStep() platformCIStep {
	return commandPlatformStep("Web Player and Creator Studio frontend tests", "pnpm", "-r", "test")
}

func platformCIPlanForGOOS(goos string) (platformCIPlan, error) {
	plan := platformCIPlan{policy: true}
	switch goos {
	case "linux":
		plan.profile = platformProfileLinuxCore
		plan.buildSteps = append([]platformCIStep{
			commandPlatformStep("Linux Core and all Go products", "go", "build", "./..."),
		}, frontendBuildSteps()...)
		plan.buildSteps = append(plan.buildSteps,
			platformCIStep{surface: "Creator Studio native Linux shell", kind: platformStepNativeStudio, nativeStudio: nativeStudioLinux},
			platformCIStep{surface: "Linux Docker Compose", kind: platformStepCompose},
		)
		plan.testSteps = []platformCIStep{
			{surface: "all Go source formatting", kind: platformStepGoFormat},
			commandPlatformStep("Linux Core and all Go package tests", "go", "test", "./..."),
			commandPlatformStep("Linux Core and all Go package vet", "go", "vet", "./..."),
			frontendTestStep(),
		}
	case "windows":
		plan.profile = platformProfileWindowsProduct
		plan.buildSteps = append([]platformCIStep{
			commandPlatformStep("projectctl Windows build", "go", "build", "./cmd/projectctl"),
			commandPlatformStep("Creator CLI Windows build", "go", "build", "./cmd/creator-cli"),
		}, frontendBuildSteps()...)
		plan.buildSteps = append(plan.buildSteps,
			platformCIStep{surface: "Creator Studio native Windows shell", kind: platformStepNativeStudio, nativeStudio: nativeStudioWindows},
		)
		plan.testSteps = []platformCIStep{
			{surface: "all Go source formatting", kind: platformStepGoFormat},
			commandPlatformStep("projectctl package tests", "go", "test", "./internal/projectctl/..."),
			commandPlatformStep("portable package tests", "go", "test", "./internal/package/..."),
			commandPlatformStep("projectctl command tests", "go", "test", "./cmd/projectctl/..."),
			commandPlatformStep("Creator CLI tests", "go", "test", "./cmd/creator-cli/..."),
			commandPlatformStep("Creator Studio Go shell tests", "go", "test", "./apps/creator-studio/..."),
			commandPlatformStep("projectctl package vet", "go", "vet", "./internal/projectctl/..."),
			commandPlatformStep("portable package vet", "go", "vet", "./internal/package/..."),
			commandPlatformStep("projectctl command vet", "go", "vet", "./cmd/projectctl/..."),
			commandPlatformStep("Creator CLI vet", "go", "vet", "./cmd/creator-cli/..."),
			commandPlatformStep("Creator Studio Go shell vet", "go", "vet", "./apps/creator-studio/..."),
			frontendTestStep(),
		}
	case "darwin":
		plan.profile = platformProfileMacOSProduct
		plan.buildSteps = append([]platformCIStep{
			commandPlatformStep("projectctl macOS build", "go", "build", "./cmd/projectctl"),
		}, frontendBuildSteps()...)
		plan.testSteps = []platformCIStep{
			{surface: "all Go source formatting", kind: platformStepGoFormat},
			commandPlatformStep("projectctl package tests", "go", "test", "./internal/projectctl/..."),
			commandPlatformStep("projectctl command tests", "go", "test", "./cmd/projectctl/..."),
			commandPlatformStep("projectctl package vet", "go", "vet", "./internal/projectctl/..."),
			commandPlatformStep("projectctl command vet", "go", "vet", "./cmd/projectctl/..."),
			frontendTestStep(),
		}
	default:
		return platformCIPlan{}, fmt.Errorf("unsupported CI platform %q; supported GOOS values are linux, windows, and darwin", goos)
	}
	return plan, nil
}

func currentPlatformCIPlan() (platformCIPlan, error) {
	return platformCIPlanForGOOS(runtime.GOOS)
}

func executePlatformSteps(ctx context.Context, steps []platformCIStep, execute func(context.Context, platformCIStep) error) error {
	for _, step := range steps {
		if err := execute(ctx, step); err != nil {
			return fmt.Errorf("%s: %w", step.surface, err)
		}
	}
	return nil
}

func (a *App) executePlatformStep(ctx context.Context, step platformCIStep) error {
	switch step.kind {
	case platformStepCommand:
		if step.name == "" {
			return errors.New("platform command has no executable")
		}
		return a.runLogged(ctx, step.name, step.args...)
	case platformStepGoFormat:
		return a.checkGoFormat(ctx)
	case platformStepNativeStudio:
		return a.buildNativeStudio(ctx, step.nativeStudio)
	case platformStepCompose:
		return a.validateCompose(ctx)
	default:
		return fmt.Errorf("unsupported platform CI step kind %q", step.kind)
	}
}

func (a *App) build(ctx context.Context) error {
	plan, err := currentPlatformCIPlan()
	if err != nil {
		return err
	}
	return a.buildWithPlan(ctx, plan)
}

func (a *App) buildWithPlan(ctx context.Context, plan platformCIPlan) error {
	if err := executePlatformSteps(ctx, plan.buildSteps, a.executePlatformStep); err != nil {
		return err
	}
	fmt.Fprintf(a.stdout, "[PASS] build matrix for %s profile\n", plan.profile)
	return nil
}

func (a *App) buildNativeStudio(ctx context.Context, profile nativeStudioProfile) error {
	if profile != nativeStudioLinux && profile != nativeStudioWindows {
		return fmt.Errorf("unsupported native Studio profile %q", profile)
	}
	temporary, err := os.MkdirTemp("", "trpg-platform-studio-build-*")
	if err != nil {
		return fmt.Errorf("create Studio build directory: %w", err)
	}
	defer os.RemoveAll(temporary)
	output := filepath.Join(temporary, "creator-studio")
	if profile == nativeStudioWindows {
		output += ".exe"
		return a.runLogged(ctx, "go", "build", "-tags", "production", "-o", output, "./apps/creator-studio")
	}
	if !pkgConfigAvailable(ctx, a.root, "gtk+-3.0", "webkit2gtk-4.1") {
		if isCI() {
			return errors.New("Linux CI requires GTK3 and WebKitGTK 4.1 development packages for the native Studio build")
		}
		fmt.Fprintln(a.stdout, "[SKIP] native Studio link: GTK3/WebKitGTK 4.1 development packages unavailable")
		return nil
	}
	return a.runLogged(ctx, "go", "build", "-tags", "production,webkit2_41", "-o", output, "./apps/creator-studio")
}

func (a *App) validateCompose(ctx context.Context) error {
	if _, err := exec.LookPath("docker"); err != nil {
		if isCI() {
			return errors.New("docker is required by the Linux CI profile")
		}
		fmt.Fprintln(a.stdout, "[SKIP] Compose validation: docker CLI unavailable")
		return nil
	}
	return a.runLogged(ctx, "docker", "compose", "-f", "deploy/compose.yaml", "config", "--quiet")
}

func pkgConfigAvailable(ctx context.Context, root string, packages ...string) bool {
	if _, err := exec.LookPath("pkg-config"); err != nil {
		return false
	}
	command := exec.CommandContext(ctx, "pkg-config", append([]string{"--exists"}, packages...)...)
	command.Dir = root
	return command.Run() == nil
}

func (a *App) checkGoFormat(ctx context.Context) error {
	goFiles, err := a.goSourceFiles()
	if err != nil {
		return err
	}
	if len(goFiles) != 0 {
		args := append([]string{"-l"}, goFiles...)
		output, err := a.capture(ctx, "gofmt", args...)
		if err != nil {
			return fmt.Errorf("gofmt check: %w", err)
		}
		if strings.TrimSpace(output) != "" {
			return fmt.Errorf("Go formatting drift:\n%s", output)
		}
	}
	return nil
}

func (a *App) test(ctx context.Context) error {
	plan, err := currentPlatformCIPlan()
	if err != nil {
		return err
	}
	return a.testWithPlan(ctx, plan)
}

func (a *App) testWithPlan(ctx context.Context, plan platformCIPlan) error {
	if err := executePlatformSteps(ctx, plan.testSteps, a.executePlatformStep); err != nil {
		return err
	}
	fmt.Fprintf(a.stdout, "[PASS] tests for %s profile\n", plan.profile)
	return nil
}

func (a *App) ci(ctx context.Context) error {
	plan, err := currentPlatformCIPlan()
	if err != nil {
		return err
	}
	if !plan.policy {
		return fmt.Errorf("platform CI profile %q does not enforce repository policy", plan.profile)
	}
	steps := []struct {
		name string
		run  func() error
	}{
		{name: "environment", run: func() error { return a.checkEnvironment(ctx) }},
		{name: "policy", run: func() error { return a.checkAll(ctx) }},
		{name: "build", run: func() error { return a.buildWithPlan(ctx, plan) }},
		{name: "test", run: func() error { return a.testWithPlan(ctx, plan) }},
	}
	for _, step := range steps {
		fmt.Fprintf(a.stdout, "[CI] %s\n", step.name)
		if err := step.run(); err != nil {
			return fmt.Errorf("CI step %s: %w", step.name, err)
		}
	}
	fmt.Fprintln(a.stdout, "[PASS] M0 CI machine gate")
	return nil
}

func (a *App) acceptM0(ctx context.Context) error {
	if err := a.ci(ctx); err != nil {
		return err
	}
	fmt.Fprintln(a.stdout, "[MACHINE PASS] M0 checks completed; independent acceptance NOT RUN")
	return nil
}

func (a *App) runLogged(ctx context.Context, name string, args ...string) error {
	fmt.Fprintf(a.stdout, "+ %s %s\n", name, strings.Join(args, " "))
	command := exec.CommandContext(ctx, name, args...)
	command.Dir = a.root
	command.Stdout = a.stdout
	command.Stderr = a.stderr
	command.Env = a.commandEnvironment()
	if err := command.Run(); err != nil {
		return fmt.Errorf("%s failed: %w", name, err)
	}
	return nil
}

func (a *App) capture(ctx context.Context, name string, args ...string) (string, error) {
	command := exec.CommandContext(ctx, name, args...)
	command.Dir = a.root
	command.Env = a.commandEnvironment()
	output, err := command.CombinedOutput()
	if err != nil {
		return string(output), fmt.Errorf("%s %s: %w: %s", name, strings.Join(args, " "), err, strings.TrimSpace(string(output)))
	}
	return string(output), nil
}

func (a *App) commandEnvironment() []string {
	environment := os.Environ()
	if os.Getenv("GOCACHE") == "" {
		environment = append(environment, "GOCACHE="+filepath.Join(os.TempDir(), "trpg-platform-m0-go-build"))
	}
	return environment
}

func isCI() bool {
	return strings.EqualFold(os.Getenv("CI"), "true") || os.Getenv("GITHUB_ACTIONS") == "true"
}

func (a *App) goSourceFiles() ([]string, error) {
	var files []string
	err := filepath.WalkDir(a.root, func(path string, entry fs.DirEntry, walkErr error) error {
		if walkErr != nil {
			return walkErr
		}
		if entry.IsDir() {
			name := entry.Name()
			if name == ".git" || name == "node_modules" || name == "dist" || name == ".codex" && filepath.Base(filepath.Dir(path)) == "runtime" {
				return filepath.SkipDir
			}
			return nil
		}
		if filepath.Ext(path) == ".go" {
			relative, err := filepath.Rel(a.root, path)
			if err != nil {
				return err
			}
			files = append(files, relative)
		}
		return nil
	})
	sort.Strings(files)
	return files, err
}

func (a *App) checkLicense(ctx context.Context) error {
	lock, err := a.loadToolchain()
	if err != nil {
		return err
	}
	license, err := os.ReadFile(filepath.Join(a.root, "LICENSE"))
	if err != nil {
		return fmt.Errorf("read LICENSE: %w", err)
	}
	copyText, err := os.ReadFile(filepath.Join(a.root, "LICENSES", "PolyForm-Noncommercial-1.0.0.txt"))
	if err != nil {
		return fmt.Errorf("read PolyForm license copy: %w", err)
	}
	if !bytes.Equal(license, copyText) {
		return errors.New("LICENSE and LICENSES/PolyForm-Noncommercial-1.0.0.txt differ")
	}
	digest := sha256.Sum256(license)
	if hex.EncodeToString(digest[:]) != officialLicenseSHA {
		return fmt.Errorf("PolyForm license hash differs from official text: %x", digest)
	}

	files, err := a.repositoryFiles(ctx)
	if err != nil {
		return err
	}
	problems := &validationErrors{}
	for _, relative := range files {
		normalized := filepath.ToSlash(relative)
		if isSecretPath(normalized) {
			problems.add("secret-like path is tracked or unignored: %s", normalized)
			continue
		}
		if !needsSPDX(normalized) {
			continue
		}
		data, readErr := os.ReadFile(filepath.Join(a.root, relative))
		if readErr != nil {
			problems.add("read %s: %v", normalized, readErr)
			continue
		}
		if !bytes.Contains(data, []byte(spdxIdentifier)) {
			problems.add("missing PolyForm SPDX header: %s", normalized)
		}
		privateKeyMarker := "-----BEGIN " + "PRIVATE KEY-----"
		if bytes.Contains(data, []byte(privateKeyMarker)) {
			problems.add("private-key material detected: %s", normalized)
		}
	}
	if err := checkPackageLicenses(a.root, problems); err != nil {
		return err
	}
	if err := problems.err("license"); err != nil {
		return err
	}
	runtime := lock.Containers["runtime"]
	fmt.Fprintf(a.stdout, "[PASS] license boundary: official PolyForm hash, %d repository paths, final runtime %s with %d bundled components\n", len(files), runtime.Reference, len(runtime.DistributionComponents))
	return nil
}

func needsSPDX(path string) bool {
	if strings.HasPrefix(path, "apps/") || strings.HasPrefix(path, "cmd/") || strings.HasPrefix(path, "internal/") || strings.HasPrefix(path, "pkg/") || strings.HasPrefix(path, "deploy/") || strings.HasPrefix(path, "tools/") || strings.HasPrefix(path, ".github/workflows/") {
		extension := filepath.Ext(path)
		return extension == ".go" || extension == ".ts" || extension == ".tsx" || extension == ".css" || extension == ".html" || extension == ".yaml" || extension == ".yml" || filepath.Base(path) == "Dockerfile"
	}
	return false
}

func isSecretPath(path string) bool {
	base := filepath.Base(path)
	if base == ".env.example" {
		return false
	}
	return path == ".env" || strings.HasPrefix(base, ".env.") || strings.HasPrefix(path, "secrets/") || strings.HasSuffix(base, ".pem") || strings.HasSuffix(base, ".key")
}

func checkPackageLicenses(root string, problems *validationErrors) error {
	return filepath.WalkDir(root, func(path string, entry fs.DirEntry, walkErr error) error {
		if walkErr != nil {
			return walkErr
		}
		if entry.IsDir() {
			if entry.Name() == ".git" || entry.Name() == "node_modules" || entry.Name() == "dist" {
				return filepath.SkipDir
			}
			return nil
		}
		if entry.Name() != "package.json" {
			return nil
		}
		var metadata struct {
			License string `json:"license"`
			Private bool   `json:"private"`
		}
		data, err := os.ReadFile(path)
		if err != nil {
			return err
		}
		if err := json.Unmarshal(data, &metadata); err != nil {
			return fmt.Errorf("parse %s: %w", path, err)
		}
		relative, _ := filepath.Rel(root, path)
		if metadata.License != "PolyForm-Noncommercial-1.0.0" || !metadata.Private {
			problems.add("%s must be private and PolyForm-Noncommercial-1.0.0", filepath.ToSlash(relative))
		}
		return nil
	})
}

func (a *App) checkScope(ctx context.Context) error {
	files, err := a.repositoryFiles(ctx)
	if err != nil {
		return err
	}
	expected := []string{
		"README.md", "LICENSE", ".gitattributes", "go.mod", "go.sum", "package.json", "pnpm-lock.yaml", "pnpm-workspace.yaml",
		"cmd/platformd/main.go", "cmd/workerd/main.go", "cmd/lua-runner/main.go", "cmd/creator-cli/main.go", "cmd/projectctl/main.go",
		"apps/web-player/src/App.tsx", "apps/creator-studio/main.go", "apps/creator-studio/frontend/src/App.tsx",
		"deploy/compose.yaml", "tools/toolchain.lock.json", "docs/70-decisions/DECISION_REGISTER.yaml", "docs/90-traceability/TRACEABILITY.yaml",
		".github/workflows/m0-baseline.yml", "docs/60-quality/M0_CI_AND_GITHUB_GOVERNANCE.md",
		".codex/SESSION_START.md", ".codex/state/DECISION_DIGEST.md", ".codex/templates/READING_MAP.template.yaml",
	}
	present := map[string]bool{}
	problems := &validationErrors{}
	goModules := 0
	for _, relative := range files {
		normalized := filepath.ToSlash(relative)
		present[normalized] = true
		if normalized == ".codex/runtime" || strings.HasPrefix(normalized, ".codex/runtime/") {
			continue
		}
		base := filepath.Base(normalized)
		if base == "go.mod" {
			goModules++
		}
		if base == "Cargo.toml" || filepath.Ext(base) == ".rs" || hasPathComponent(normalized, "migrations") || hasPathComponent(normalized, "migration") || hasPathComponent(normalized, "legacy") || strings.HasPrefix(normalized, "source-archive/") || strings.HasPrefix(normalized, "docs/codex/") {
			problems.add("legacy or out-of-scope path: %s", normalized)
		}
		if strings.Contains(normalized, "/dist/") || strings.Contains(normalized, "/node_modules/") || strings.HasPrefix(normalized, "target/") {
			problems.add("generated artifact is tracked or unignored: %s", normalized)
		}
		if isSecretPath(normalized) {
			problems.add("secret-like path is tracked or unignored: %s", normalized)
		}
		if isProgramSource(normalized) {
			data, readErr := os.ReadFile(filepath.Join(a.root, relative))
			if readErr != nil {
				problems.add("read %s: %v", normalized, readErr)
				continue
			}
			lower := strings.ToLower(string(data))
			for _, forbidden := range []string{
				"api." + "openai.com",
				"ol" + "lama",
				"llama" + ".cpp",
				"github.com/" + "nats-io",
				"gopher" + "-lua",
				"shopify/" + "go-lua",
				"database/" + "sql",
				"lib/" + "pq",
				"p" + "gx",
			} {
				if strings.Contains(lower, forbidden) {
					problems.add("M0 source %s contains forbidden integration token %q", normalized, forbidden)
				}
			}
		}
	}
	for _, path := range expected {
		if !present[path] {
			problems.add("missing required M0 path: %s", path)
		}
	}
	if goModules != 1 {
		problems.add("expected one active go.mod, found %d", goModules)
	}
	if err := a.checkFrontendBoundaries(); err != nil {
		problems.add("%v", err)
	}
	if err := problems.err("M0 scope"); err != nil {
		return err
	}
	fmt.Fprintf(a.stdout, "[PASS] M0 scope: %d active repository paths, zero legacy implementation paths\n", len(files))
	return nil
}

// x-section-id: PROJECTCTL-FRONTEND-BOUNDARY-CONTRACT
const (
	creatorExtensionCapabilitySentinel = "schemas/package/manifest-v2.schema.json"
	webPlayerFrontendPath              = "apps/web-player/src/App.tsx"
	creatorFrontendPath                = "apps/creator-studio/frontend/src/App.tsx"
	frontendM0RestartMarker            = "V1 restart baseline"
	frontendM0NoPlayableMarker         = "No playable functionality"
	frontendM0ShellMarker              = "M0 engineering shell only"
	creatorGenericSchemaMarker         = "Schema-validated generic JSON"
	creatorGenericPackageMarker        = "Package-generic JSON by design"
	creatorGenericFormsMarker          = "No game-specific forms or templates"
	creatorGenericRuntimeMarker        = "No playable preview or runtime execution"
)

type frontendBoundaryContract struct {
	path     string
	required []string
}

type lstatFunc func(string) (fs.FileInfo, error)

func (a *App) frontendBoundaryContracts() ([]frontendBoundaryContract, error) {
	return a.frontendBoundaryContractsWithLstat(os.Lstat)
}

func (a *App) frontendBoundaryContractsWithLstat(lstat lstatFunc) ([]frontendBoundaryContract, error) {
	contracts := []frontendBoundaryContract{m0FrontendBoundaryContract(webPlayerFrontendPath)}
	sentinel := filepath.Join(a.root, filepath.FromSlash(creatorExtensionCapabilitySentinel))
	info, err := lstat(sentinel)
	switch {
	case err == nil:
		if info == nil || !info.Mode().IsRegular() {
			return nil, fmt.Errorf("Creator capability sentinel %s is not a regular file", creatorExtensionCapabilitySentinel)
		}
		contracts = append(contracts, genericCreatorBoundaryContract())
	case errors.Is(err, fs.ErrNotExist):
		contracts = append(contracts, m0FrontendBoundaryContract(creatorFrontendPath))
	default:
		return nil, fmt.Errorf("inspect Creator capability sentinel %s: %w", creatorExtensionCapabilitySentinel, err)
	}
	return contracts, nil
}

func (a *App) checkFrontendBoundaries() error {
	contracts, err := a.frontendBoundaryContracts()
	if err != nil {
		return err
	}
	return a.checkFrontendBoundaryCopy(contracts)
}

func (a *App) checkFrontendBoundariesWithLstat(lstat lstatFunc) error {
	contracts, err := a.frontendBoundaryContractsWithLstat(lstat)
	if err != nil {
		return err
	}
	return a.checkFrontendBoundaryCopy(contracts)
}

func (a *App) checkFrontendBoundaryCopy(contracts []frontendBoundaryContract) error {
	problems := &validationErrors{}
	for _, contract := range contracts {
		data, err := os.ReadFile(filepath.Join(a.root, filepath.FromSlash(contract.path)))
		if err != nil {
			problems.add("read frontend boundary %s: %v", contract.path, err)
			continue
		}
		for _, marker := range contract.required {
			if !bytes.Contains(data, []byte(marker)) {
				problems.add("%s is missing frontend boundary marker %q", contract.path, marker)
			}
		}
	}
	return problems.err("frontend boundary copy")
}

func m0FrontendBoundaryContract(path string) frontendBoundaryContract {
	return frontendBoundaryContract{
		path: path,
		required: []string{
			frontendM0RestartMarker,
			frontendM0NoPlayableMarker,
			frontendM0ShellMarker,
		},
	}
}

func genericCreatorBoundaryContract() frontendBoundaryContract {
	return frontendBoundaryContract{
		path: creatorFrontendPath,
		required: []string{
			creatorGenericSchemaMarker,
			creatorGenericPackageMarker,
			creatorGenericFormsMarker,
			creatorGenericRuntimeMarker,
		},
	}
}

func (a *App) repositoryFiles(ctx context.Context) ([]string, error) {
	output, err := a.capture(ctx, "git", "ls-files", "--cached", "--others", "--exclude-standard")
	if err != nil {
		return nil, fmt.Errorf("list repository files: %w", err)
	}
	lines := strings.Split(strings.TrimSpace(output), "\n")
	files := make([]string, 0, len(lines))
	for _, line := range lines {
		if line == "" || line == ".codex/runtime" || strings.HasPrefix(filepath.ToSlash(line), ".codex/runtime/") {
			continue
		}
		files = append(files, filepath.Clean(line))
	}
	sort.Strings(files)
	return files, nil
}

func hasPathComponent(path, component string) bool {
	for _, item := range strings.Split(filepath.ToSlash(path), "/") {
		if strings.EqualFold(item, component) {
			return true
		}
	}
	return false
}

func isProgramSource(path string) bool {
	extension := filepath.Ext(path)
	return extension == ".go" || extension == ".ts" || extension == ".tsx" || extension == ".js" || extension == ".jsx"
}
