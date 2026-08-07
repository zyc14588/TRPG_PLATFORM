// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package projectctl

import (
	"bytes"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func testApp(t *testing.T) *App {
	t.Helper()
	root, err := findRoot()
	if err != nil {
		t.Fatal(err)
	}
	return &App{root: root, stdout: &bytes.Buffer{}, stderr: &bytes.Buffer{}}
}

func TestFrozenAuthorityValidates(t *testing.T) {
	a := testApp(t)
	decisions, requirements, tests, trace, err := a.loadAuthority()
	if err != nil {
		t.Fatal(err)
	}
	if err := validateDecisions(decisions); err != nil {
		t.Fatal(err)
	}
	if err := a.validateTraceability(decisions, requirements, tests, trace); err != nil {
		t.Fatal(err)
	}
}

func TestToolchainPinsAreConsistent(t *testing.T) {
	a := testApp(t)
	lock, err := a.loadToolchain()
	if err != nil {
		t.Fatal(err)
	}
	if lock.Tools.Go.Version != "1.26.5" || lock.Tools.Node.Version != "24.18.0" || lock.Tools.PNPM.Version != "11.20.0" {
		t.Fatalf("unexpected frozen toolchain: Go %s, Node %s, pnpm %s", lock.Tools.Go.Version, lock.Tools.Node.Version, lock.Tools.PNPM.Version)
	}
}

func TestGeneratedDocumentsAreCurrentAndMarked(t *testing.T) {
	a := testApp(t)
	documents, err := a.generatedDocuments()
	if err != nil {
		t.Fatal(err)
	}
	if len(documents) != 3 {
		t.Fatalf("generated %d documents, want 3", len(documents))
	}
	for _, document := range documents {
		if !bytes.Contains(document.data, []byte(generatedMarker)) {
			t.Errorf("%s lacks generated marker", document.path)
		}
		actual, err := os.ReadFile(filepath.Join(a.root, filepath.FromSlash(document.path)))
		if err != nil {
			t.Fatal(err)
		}
		if !bytes.Equal(actual, document.data) {
			t.Errorf("%s has generation drift", document.path)
		}
	}
}

func TestMilestoneGateIsExact(t *testing.T) {
	if err := requireM0([]string{"--milestone", "M0"}); err != nil {
		t.Fatal(err)
	}
	for _, arguments := range [][]string{{}, {"--milestone", "M1"}, {"--milestone", "M0", "extra"}} {
		if err := requireM0(arguments); err == nil {
			t.Errorf("requireM0(%q) unexpectedly passed", arguments)
		}
	}
}

func TestRoutesStayProgressive(t *testing.T) {
	for _, mode := range []string{"PLAN", "IMPLEMENT", "ACCEPT", "REPAIR"} {
		paths := routePaths(mode)
		if len(paths.always) == 0 || len(paths.machine) == 0 {
			t.Fatalf("%s route is incomplete", mode)
		}
		for _, path := range append(append(paths.always, paths.normative...), paths.machine...) {
			if path == "docs/**" || strings.Contains(path, "git-history") {
				t.Fatalf("%s route contains a bulk-read path %q", mode, path)
			}
		}
	}
}

func TestScopeAndLicensePathClassifiers(t *testing.T) {
	if !needsSPDX("cmd/projectctl/main.go") || !needsSPDX("apps/web-player/src/App.tsx") {
		t.Fatal("program source must require SPDX")
	}
	if needsSPDX("docs/10-product/PRODUCT_DEFINITION.md") {
		t.Fatal("normative prose must not be treated as program source")
	}
	if !isSecretPath(".env.local") || !isSecretPath("secrets/provider/token") || isSecretPath(".env.example") {
		t.Fatal("secret path classifier drift")
	}
}
