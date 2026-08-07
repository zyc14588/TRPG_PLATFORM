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
	"runtime"
	"sort"
	"strings"
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
	Containers map[string]lockedContainer `json:"containers"`
}

type lockedTool struct {
	Version     string `json:"version"`
	License     string `json:"license"`
	Source      string `json:"source"`
	ReleaseLine string `json:"release_line"`
	Integrity   string `json:"integrity"`
	Stability   string `json:"stability"`
}

type lockedContainer struct {
	Reference string `json:"reference"`
	License   string `json:"license"`
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
		if strings.TrimSpace(string(data)) != expected {
			problems.add("%s does not match toolchain lock", relative)
		}
	}
	checkTextFile(".go-version", lock.Tools.Go.Version)
	checkTextFile(".node-version", lock.Tools.Node.Version)
	checkTextFile(".tool-versions", fmt.Sprintf("golang %s\nnodejs %s\njust %s", lock.Tools.Go.Version, lock.Tools.Node.Version, lock.Tools.Just.Version))

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

	dockerfile, err := os.ReadFile(filepath.Join(a.root, "deploy", "docker", "Dockerfile"))
	if err != nil {
		problems.add("read deploy/docker/Dockerfile: %v", err)
	} else {
		for name, container := range lock.Containers {
			if !strings.Contains(string(dockerfile), container.Reference) {
				problems.add("Dockerfile does not contain locked %s image", name)
			}
			if !strings.Contains(container.Reference, "@sha256:") {
				problems.add("container %s lacks immutable digest", name)
			}
		}
	}
	return problems.err("toolchain lock")
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
	if _, err := os.Stat(filepath.Join(a.root, ".codex")); err == nil {
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

func (a *App) build(ctx context.Context) error {
	if runtime.GOOS == "darwin" {
		if err := a.runLogged(ctx, "go", "build", "./cmd/projectctl"); err != nil {
			return err
		}
	} else if err := a.runLogged(ctx, "go", "build", "./..."); err != nil {
		return err
	}
	if err := a.runLogged(ctx, "pnpm", "-r", "typecheck"); err != nil {
		return err
	}
	if err := a.runLogged(ctx, "pnpm", "-r", "build"); err != nil {
		return err
	}
	if err := a.buildNativeStudio(ctx); err != nil {
		return err
	}
	if runtime.GOOS == "linux" {
		if _, err := exec.LookPath("docker"); err != nil {
			if isCI() {
				return errors.New("docker is required by the Linux CI profile")
			}
			fmt.Fprintln(a.stdout, "[SKIP] Compose validation: docker CLI unavailable")
		} else if err := a.runLogged(ctx, "docker", "compose", "-f", "deploy/compose.yaml", "config", "--quiet"); err != nil {
			return err
		}
	}
	fmt.Fprintln(a.stdout, "[PASS] build matrix for current platform")
	return nil
}

func (a *App) buildNativeStudio(ctx context.Context) error {
	if runtime.GOOS == "darwin" {
		fmt.Fprintln(a.stdout, "[SKIP] native Studio build is outside the macOS M0 CI profile")
		return nil
	}
	temporary, err := os.MkdirTemp("", "trpg-platform-studio-build-*")
	if err != nil {
		return fmt.Errorf("create Studio build directory: %w", err)
	}
	defer os.RemoveAll(temporary)
	output := filepath.Join(temporary, "creator-studio")
	if runtime.GOOS == "windows" {
		output += ".exe"
		return a.runLogged(ctx, "go", "build", "-tags", "production", "-o", output, "./apps/creator-studio")
	}
	if runtime.GOOS != "linux" {
		return nil
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

func pkgConfigAvailable(ctx context.Context, root string, packages ...string) bool {
	if _, err := exec.LookPath("pkg-config"); err != nil {
		return false
	}
	command := exec.CommandContext(ctx, "pkg-config", append([]string{"--exists"}, packages...)...)
	command.Dir = root
	return command.Run() == nil
}

func (a *App) test(ctx context.Context) error {
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
	goTarget := "./..."
	if runtime.GOOS == "darwin" {
		goTarget = "./internal/projectctl"
	}
	if err := a.runLogged(ctx, "go", "test", goTarget); err != nil {
		return err
	}
	if err := a.runLogged(ctx, "go", "vet", goTarget); err != nil {
		return err
	}
	if err := a.runLogged(ctx, "pnpm", "-r", "test"); err != nil {
		return err
	}
	fmt.Fprintln(a.stdout, "[PASS] tests for current platform")
	return nil
}

func (a *App) ci(ctx context.Context) error {
	steps := []struct {
		name string
		run  func() error
	}{
		{name: "environment", run: func() error { return a.checkEnvironment(ctx) }},
		{name: "policy", run: func() error { return a.checkAll(ctx) }},
		{name: "build", run: func() error { return a.build(ctx) }},
		{name: "test", run: func() error { return a.test(ctx) }},
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
	if _, err := a.loadToolchain(); err != nil {
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
	fmt.Fprintf(a.stdout, "[PASS] license boundary: official PolyForm hash and %d repository paths checked\n", len(files))
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
		"README.md", "LICENSE", "go.mod", "go.sum", "package.json", "pnpm-lock.yaml", "pnpm-workspace.yaml",
		"cmd/platformd/main.go", "cmd/workerd/main.go", "cmd/lua-runner/main.go", "cmd/creator-cli/main.go", "cmd/projectctl/main.go",
		"apps/web-player/src/App.tsx", "apps/creator-studio/main.go", "apps/creator-studio/frontend/src/App.tsx",
		"deploy/compose.yaml", "tools/toolchain.lock.json", "docs/70-decisions/DECISION_REGISTER.yaml", "docs/90-traceability/TRACEABILITY.yaml",
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
	for _, appPath := range []string{"apps/web-player/src/App.tsx", "apps/creator-studio/frontend/src/App.tsx"} {
		data, readErr := os.ReadFile(filepath.Join(a.root, filepath.FromSlash(appPath)))
		if readErr == nil {
			for _, copyLine := range []string{"V1 restart baseline", "No playable functionality", "M0 engineering shell only"} {
				if !bytes.Contains(data, []byte(copyLine)) {
					problems.add("%s is missing required boundary copy %q", appPath, copyLine)
				}
			}
		}
	}
	if err := problems.err("M0 scope"); err != nil {
		return err
	}
	fmt.Fprintf(a.stdout, "[PASS] M0 scope: %d active repository paths, zero legacy implementation paths\n", len(files))
	return nil
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
