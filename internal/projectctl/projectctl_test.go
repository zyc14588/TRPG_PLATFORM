// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package projectctl

import (
	"bytes"
	"context"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"

	"gopkg.in/yaml.v3"
)

const testContextCapacity = 131072

func testCodexRequest(mode, milestone, batchID string) codexRouteRequest {
	request := newCodexRouteRequest(mode)
	request.Milestone = milestone
	request.BatchID = batchID
	request.ContextCapacityBytes = testContextCapacity
	return request
}

func testApp(t *testing.T) *App {
	t.Helper()
	root, err := findRoot()
	if err != nil {
		t.Fatal(err)
	}
	return &App{root: root, stdout: &bytes.Buffer{}, stderr: &bytes.Buffer{}}
}

func testV1MilestoneCatalog(t *testing.T, a *App) v1MilestoneCatalog {
	t.Helper()
	catalog, err := a.loadV1MilestoneCatalog()
	if err != nil {
		t.Fatal(err)
	}
	return catalog
}

func TestJustCodexPlanForwardsMilestone(t *testing.T) {
	a := testApp(t)
	data, err := os.ReadFile(filepath.Join(a.root, "Justfile"))
	if err != nil {
		t.Fatal(err)
	}
	text := string(data)
	if !strings.Contains(text, "codex-plan milestone:\n    {{projectctl}} codex plan --milestone {{milestone}}") {
		t.Fatal("codex-plan does not forward its required milestone argument to projectctl")
	}
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

func TestPinnedTextAcceptsWindowsCheckoutNewlines(t *testing.T) {
	input := []byte("golang 1.26.5\r\nnodejs 24.18.0\r\njust 1.58.0\r\n")
	want := "golang 1.26.5\nnodejs 24.18.0\njust 1.58.0"
	if got := normalizePinnedText(input); got != want {
		t.Fatalf("normalizePinnedText()=%q, want %q", got, want)
	}
}

func TestGeneratedDocumentsAreCurrentAndMarked(t *testing.T) {
	a := testApp(t)
	documents, err := a.generatedDocuments()
	if err != nil {
		t.Fatal(err)
	}
	if len(documents) != 4 {
		t.Fatalf("generated %d documents, want 4", len(documents))
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

// x-section-id: PROJECTCTL-CODEX-ROUTE-TESTS
func TestRoutesStayProgressive(t *testing.T) {
	a := testApp(t)
	request := testCodexRequest("PLAN", "M1", "")
	paths, err := a.routePaths(request)
	if err != nil {
		t.Fatal(err)
	}
	if len(paths.always) == 0 || len(paths.machine) == 0 {
		t.Fatal("M1 PLAN route is incomplete")
	}
	all := append(append(append(paths.always, paths.normative...), paths.machine...), paths.onDemand...)
	seen := map[string]bool{}
	wanted := map[string]bool{
		"docs/80-roadmap/V1_MILESTONES.md#SPEC-V1-ROADMAP-M1":                   false,
		"docs/80-roadmap/M1_SCOPE_AND_EXIT_GATE.md#SPEC-M1-EXIT":                false,
		"docs/90-traceability/REQUIREMENTS.yaml#REQ-PACKAGE-001":                false,
		"docs/90-traceability/TEST_CATALOG.yaml#TEST-PACKAGE-001":               false,
		"docs/90-traceability/TRACEABILITY.yaml#REQ-PACKAGE-001":                false,
		"docs/70-decisions/DECISION_REGISTER.yaml#R2-A01":                       false,
		"docs/30-package-spec/PACKAGE_MODEL.md#SPEC-PACKAGE-001":                false,
		"docs/20-architecture/LUA_RUNTIME.md#SPEC-LUA-RUNTIME-001":              false,
		"docs/30-package-spec/HOST_API_AND_CALLBACKS.md#SPEC-HOST-CALLBACK-001": false,
	}
	for _, spec := range all {
		if err := validateBoundedRoutePath(spec.path); err != nil {
			t.Fatalf("M1 PLAN route: %v", err)
		}
		if spec.path == "docs/**" || strings.ContainsAny(spec.path, "*?[") {
			t.Fatalf("M1 PLAN route contains an unbounded path %q", spec.path)
		}
		key := spec.path + "#" + spec.sectionID
		if seen[key] {
			t.Fatalf("M1 PLAN route repeats %s", key)
		}
		seen[key] = true
		if _, ok := wanted[key]; ok {
			wanted[key] = true
		}
		data, err := os.ReadFile(filepath.Join(a.root, filepath.FromSlash(spec.path)))
		if err != nil {
			t.Fatal(err)
		}
		if _, err := sectionMaterial(data, spec.sectionID); err != nil {
			t.Fatalf("M1 PLAN route has invalid Section ID %s: %v", key, err)
		}
	}
	for key, found := range wanted {
		if !found {
			t.Errorf("M1 PLAN route omitted %s", key)
		}
	}
}

func TestGeneratedReadingMapContractIsMachineValid(t *testing.T) {
	a := testApp(t)
	catalog := testV1MilestoneCatalog(t, a)
	request := testCodexRequest("PLAN", "M1", "")
	specs, err := a.routePaths(request)
	if err != nil {
		t.Fatal(err)
	}
	routeMap := readingMap{
		RouteSchemaVersion: readingMapSchemaVersion,
		Mode:               "PLAN", Milestone: "M1", RouteScope: milestoneRouteScope,
		SourceCommit: strings.Repeat("a", 40), SourceTree: strings.Repeat("b", 40),
		GeneratedUTC: "2026-08-08T00:00:00Z",
		ContextProfile: contextProfile{
			Executor: codexExecutor, ProfileID: codexContextProfileID, Enforcement: false,
			CapacityBytes: testContextCapacity, Measurement: contextMeasurement,
		},
		MustNotBulkRead: []string{"docs/**", "git-history", "prior-chat-transcripts"},
	}
	if routeMap.AlwaysRead, err = a.hashRouteSections(specs.always); err != nil {
		t.Fatal(err)
	}
	if routeMap.NormativeReferences, err = a.hashRouteSections(specs.normative); err != nil {
		t.Fatal(err)
	}
	if routeMap.MachineContracts, err = a.hashRouteSections(specs.machine); err != nil {
		t.Fatal(err)
	}
	if routeMap.ReadOnDemand, err = a.hashRouteSections(specs.onDemand); err != nil {
		t.Fatal(err)
	}
	if err := applyContextBudget(&routeMap); err != nil {
		t.Fatal(err)
	}
	routeMap.RouteBindingSHA256, err = readingMapBindingDigest(routeMap)
	if err != nil {
		t.Fatal(err)
	}
	if err := validateReadingMapMetadata(routeMap, catalog); err != nil {
		t.Fatalf("M1 PLAN reading map contract is invalid: %v", err)
	}
	if err := a.validateCanonicalReadingMap(routeMap); err != nil {
		t.Fatalf("M1 PLAN canonical reading map failed: %v", err)
	}
	if routeMap.BatchID != "" || routeMap.RouteScope != milestoneRouteScope {
		t.Fatalf("M1 PLAN route allocated a batch: %+v", routeMap)
	}
	nonCanonical := routeMap
	nonCanonical.MachineContracts = append([]routeSection(nil), routeMap.MachineContracts[1:]...)
	if err := a.validateCanonicalReadingMap(nonCanonical); err == nil {
		t.Fatal("M1 PLAN reading map accepted a non-canonical but non-empty Section set")
	}
	routeMap.BatchID = "M1-B999"
	if err := validateReadingMapMetadata(routeMap, catalog); err == nil {
		t.Fatal("M1 PLAN reading map accepted a prematurely allocated batch ID")
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

func TestFinalRuntimeSupplyChainIsComponentFree(t *testing.T) {
	a := testApp(t)
	lock, err := a.loadToolchain()
	if err != nil {
		t.Fatal(err)
	}
	runtime := lock.Containers["runtime"]
	if runtime.Role != "final-runtime" || runtime.Reference != "scratch" || runtime.License != "NONE" || len(runtime.DistributionComponents) != 0 {
		t.Fatalf("unexpected final runtime lock: %+v", runtime)
	}
}

func TestContainerSupplyChainRejectsAggregateAndCopyleftMisclassification(t *testing.T) {
	base := toolchainLock{Containers: map[string]lockedContainer{
		"builder": {Reference: "builder@example@sha256:abc", Role: "build-only", License: "NOASSERTION"},
	}}
	tests := []struct {
		name       string
		runtime    lockedContainer
		dockerfile string
	}{
		{
			name:       "aggregate MIT label cannot hide Alpine packages",
			runtime:    lockedContainer{Reference: "alpine@example@sha256:def", Role: "final-runtime", License: "MIT"},
			dockerfile: "FROM builder@example@sha256:abc AS build\nFROM alpine@example@sha256:def\n",
		},
		{
			name: "strong copyleft component is prohibited",
			runtime: lockedContainer{
				Reference: "runtime@example@sha256:def", Role: "final-runtime", License: "NOASSERTION",
				DistributionComponents: []lockedContainerComponent{{Name: "busybox", Version: "1.37.0-r30", License: "GPL-2.0-only"}},
			},
			dockerfile: "FROM builder@example@sha256:abc AS build\nFROM runtime@example@sha256:def\n",
		},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			lock := base
			lock.Containers = map[string]lockedContainer{
				"builder": base.Containers["builder"],
				"runtime": test.runtime,
			}
			if err := validateContainerSupplyChain(lock, []byte(test.dockerfile)); err == nil {
				t.Fatal("invalid final runtime unexpectedly passed")
			}
		})
	}
}

func TestCodexSourceCommitHasTrustedCandidateProvenance(t *testing.T) {
	a := testApp(t)
	if err := a.verifyCodexSourceCommit(context.Background(), governanceSourceCommit); err != nil {
		t.Fatal(err)
	}
}

func TestCodexSourceCommitRejectsPredecessorAndInvalidIdentity(t *testing.T) {
	a := testApp(t)
	ctx := context.Background()
	for name, commit := range map[string]string{
		"replaced predecessor chain": "1bb2e4ec9b569e4b8dab1e43ffb91e03c44f2acc",
		"missing object":             "0000000000000000000000000000000000000000",
	} {
		t.Run(name, func(t *testing.T) {
			if err := a.verifyCodexSourceCommit(ctx, commit); err == nil {
				t.Fatalf("invalid source commit %s unexpectedly passed", commit)
			}
		})
	}
	if err := a.verifyTrustedSSHCommit(ctx, "73bcee9500720f0d26d2b6f9676d4a2e6f30d964"); err == nil {
		t.Fatal("commit outside the trusted SSH signer identity unexpectedly passed")
	}
}

func TestSectionMaterialRequiresUniqueStableID(t *testing.T) {
	valid := []byte("intro\n<a id=\"SECTION-A\"></a>\n## A\nbody\n<a id=\"SECTION-B\"></a>\n## B\n")
	material, err := sectionMaterial(valid, "SECTION-A")
	if err != nil {
		t.Fatal(err)
	}
	if strings.Contains(string(material), "SECTION-B") {
		t.Fatal("section extraction crossed the next stable Section ID")
	}
	duplicate := append(valid, []byte("<a id=\"SECTION-A\"></a>\n")...)
	if _, err := sectionMaterial(duplicate, "SECTION-A"); err == nil {
		t.Fatal("duplicate stable Section ID unexpectedly passed")
	}
	if _, err := sectionMaterial(valid, "MISSING"); err == nil {
		t.Fatal("missing stable Section ID unexpectedly passed")
	}
}

func TestRouteRequestBindsModeMilestoneAndBatch(t *testing.T) {
	a := testApp(t)
	catalog := testV1MilestoneCatalog(t, a)
	planRequest, err := parseCodexPlanRequest([]string{"--milestone", "M1", "--context-capacity-bytes", "4096"})
	if err != nil {
		t.Fatal(err)
	}
	if planRequest.Mode != "PLAN" || planRequest.Milestone != "M1" || planRequest.BatchID != "" {
		t.Fatalf("unexpected milestone-level plan request: %+v", planRequest)
	}
	if err := validateCodexRouteRequest(planRequest, catalog); err != nil {
		t.Fatal(err)
	}

	request, err := parseCodexRouteRequest([]string{
		"--mode", "repair", "--milestone", "M1", "--batch", "M1-B001", "--context-capacity-bytes", "4096",
	})
	if err != nil {
		t.Fatal(err)
	}
	if request.Mode != "REPAIR" || request.Milestone != "M1" || request.BatchID != "M1-B001" || request.ContextCapacityBytes != 4096 {
		t.Fatalf("unexpected route request: %+v", request)
	}
	if err := validateCodexRouteRequest(request, catalog); err != nil {
		t.Fatal(err)
	}
	for _, invalid := range []codexRouteRequest{
		{Mode: "IMPLEMENT", Milestone: "M0", BatchID: "M1-B001", Executor: codexExecutor, ProfileID: codexContextProfileID},
		{Mode: "IMPLEMENT", Milestone: "M9", BatchID: "M9-B001", Executor: codexExecutor, ProfileID: codexContextProfileID},
		{Mode: "IMPLEMENT", Milestone: "M01", BatchID: "M01-B001", Executor: codexExecutor, ProfileID: codexContextProfileID},
		{Mode: "IMPLEMENT", Milestone: "M1", Executor: codexExecutor, ProfileID: codexContextProfileID},
		{Mode: "PLAN", Milestone: "M1", BatchID: "M1-B001", Executor: codexExecutor, ProfileID: codexContextProfileID},
		{Mode: "UNKNOWN", Milestone: "M0", BatchID: "M0-B001", Executor: codexExecutor, ProfileID: codexContextProfileID},
	} {
		if err := validateCodexRouteRequest(invalid, catalog); err == nil {
			t.Fatalf("invalid route request unexpectedly passed: %+v", invalid)
		}
	}
	notGeneratedPlan := milestonePlan{
		SchemaVersion: milestonePlanSchemaVersion, PlanID: "M1-MILESTONE-PLAN", PlanVersion: 0,
		Milestone: "M1", Status: "NOT_GENERATED", ModifiableOnlyInMode: "PLAN", NextBatchSequence: 1,
		Batches: []milestoneBatch{}, Tombstones: []batchTombstone{},
	}
	if err := validateRouteAgainstPlan(planRequest, notGeneratedPlan, catalog); err != nil {
		t.Fatalf("M1 PLAN request did not match the planning baseline: %v", err)
	}
	if err := validateRouteAgainstPlan(request, notGeneratedPlan, catalog); err == nil {
		t.Fatal("batch-scoped route accepted a syntactic but unallocated M1 batch ID")
	}
	realBatch := testMilestoneBatch("M1-B001", 1)
	realBatch.Requirements = []string{"REQ-PACKAGE-001"}
	activePlan := milestonePlan{
		SchemaVersion: milestonePlanSchemaVersion, PlanID: "M1-MILESTONE-PLAN", PlanVersion: 1,
		Milestone: "M1", Status: "ACTIVE", ModifiableOnlyInMode: "PLAN", NextBatchSequence: 2,
		Batches: []milestoneBatch{realBatch}, Tombstones: []batchTombstone{},
	}
	for _, mode := range []string{"IMPLEMENT", "ACCEPT", "REPAIR"} {
		batchRequest := testCodexRequest(mode, "M1", "M1-B001")
		if err := validateCodexRouteRequest(batchRequest, catalog); err != nil {
			t.Fatalf("%s rejected a valid batch binding: %v", mode, err)
		}
		if err := validateRouteAgainstPlan(batchRequest, activePlan, catalog); err != nil {
			t.Fatalf("%s rejected a real allocated batch: %v", mode, err)
		}
	}
	for _, path := range []string{"docs/**", "docs", "../docs/file.md", "git-history", ".git/objects"} {
		if err := validateBoundedRoutePath(path); err == nil {
			t.Fatalf("unbounded route path %q unexpectedly passed", path)
		}
	}
}

func TestGovernanceMaintenanceTargetsAndProgressiveRoute(t *testing.T) {
	a := testApp(t)
	catalog := testV1MilestoneCatalog(t, a)
	maintenanceID := "GOV-M1-PLANNING-RUNTIME"
	for _, mode := range []string{"REPAIR", "ACCEPT"} {
		request := newCodexRouteRequest(mode)
		request.MaintenanceID = maintenanceID
		if err := validateCodexRouteRequest(request, catalog); err != nil {
			t.Fatalf("%s maintenance target failed request validation: %v", mode, err)
		}
		if err := a.validateRouteAgainstCurrentPlan(request, catalog); err != nil {
			t.Fatalf("%s maintenance target failed contract validation: %v", mode, err)
		}
		if routeScopeForRequest(request) != maintenanceRouteScope {
			t.Fatalf("%s maintenance target has wrong route scope", mode)
		}
	}

	implement := newCodexRouteRequest("IMPLEMENT")
	implement.MaintenanceID = maintenanceID
	if err := validateCodexRouteRequest(implement, catalog); err == nil {
		t.Fatal("IMPLEMENT governance maintenance unexpectedly passed")
	}
	missing := newCodexRouteRequest("REPAIR")
	missing.MaintenanceID = "GOV-NOT-REGISTERED"
	if err := validateCodexRouteRequest(missing, catalog); err != nil {
		t.Fatalf("syntactically valid missing maintenance ID failed before contract lookup: %v", err)
	}
	if err := a.validateRouteAgainstCurrentPlan(missing, catalog); err == nil || !strings.Contains(err.Error(), "does not exist") {
		t.Fatalf("missing maintenance contract did not fail closed: %v", err)
	}
	invalidID := newCodexRouteRequest("REPAIR")
	invalidID.MaintenanceID = "M1-B001"
	if err := validateCodexRouteRequest(invalidID, catalog); err == nil {
		t.Fatal("non-GOV maintenance ID unexpectedly passed")
	}
	conflicting := newCodexRouteRequest("REPAIR")
	conflicting.Milestone, conflicting.BatchID, conflicting.MaintenanceID = "M1", "M1-B001", maintenanceID
	if err := validateCodexRouteRequest(conflicting, catalog); err == nil || !strings.Contains(err.Error(), "mutually exclusive") {
		t.Fatalf("batch + maintenance target did not fail closed: %v", err)
	}

	request := newCodexRouteRequest("REPAIR")
	request.MaintenanceID = maintenanceID
	paths, err := a.routePaths(request)
	if err != nil {
		t.Fatal(err)
	}
	contract, err := a.loadGovernanceMaintenanceContract(maintenanceID)
	if err != nil {
		t.Fatal(err)
	}
	present := map[string]bool{}
	allSpecs := append(append(append(paths.always, paths.normative...), paths.machine...), paths.onDemand...)
	for _, spec := range allSpecs {
		key := spec.path + "#" + spec.sectionID
		present[key] = true
		if strings.HasPrefix(spec.path, "apps/") || strings.HasPrefix(spec.path, "packages/") || strings.HasPrefix(spec.path, "docs/10-product/") {
			t.Fatalf("maintenance route disclosed product scope %s", key)
		}
	}
	for _, reference := range contract.AllowedScope {
		key := reference.Path + "#" + reference.SectionID
		if !present[key] {
			t.Errorf("maintenance route omitted allowed scope %s", key)
		}
	}
	for _, forbidden := range []string{"docs/**", "git-history", "prior-chat-transcripts", "unrelated-product-specs", "M1-business-implementation"} {
		if !contains(mustNotBulkReadForRequest(request), forbidden) {
			t.Errorf("maintenance route omitted forbidden bulk-read rule %q", forbidden)
		}
	}

	profile, err := contextProfileForRequest(request)
	if err != nil {
		t.Fatal(err)
	}
	routeMap := readingMap{
		RouteSchemaVersion: readingMapSchemaVersion, Mode: request.Mode, RouteScope: maintenanceRouteScope,
		MaintenanceID: maintenanceID, SourceCommit: strings.Repeat("a", 40), SourceTree: strings.Repeat("b", 40),
		GeneratedUTC: "2026-08-09T00:00:00Z", ContextProfile: profile, MustNotBulkRead: mustNotBulkReadForRequest(request),
	}
	if routeMap.AlwaysRead, err = a.hashRouteSections(paths.always); err != nil {
		t.Fatal(err)
	}
	if routeMap.NormativeReferences, err = a.hashRouteSections(paths.normative); err != nil {
		t.Fatal(err)
	}
	if routeMap.MachineContracts, err = a.hashRouteSections(paths.machine); err != nil {
		t.Fatal(err)
	}
	if routeMap.ReadOnDemand, err = a.hashRouteSections(paths.onDemand); err != nil {
		t.Fatal(err)
	}
	if err := applyContextBudget(&routeMap); err != nil {
		t.Fatal(err)
	}
	if routeMap.RouteBindingSHA256, err = readingMapBindingDigest(routeMap); err != nil {
		t.Fatal(err)
	}
	if err := validateReadingMapMetadata(routeMap, catalog); err != nil {
		t.Fatalf("maintenance Reading Map metadata is invalid: %v", err)
	}
	if err := a.validateCanonicalReadingMap(routeMap); err != nil {
		t.Fatalf("maintenance Reading Map is not canonical: %v", err)
	}
	routeMap.SourceTree = strings.Repeat("c", 40)
	if err := validateReadingMapMetadata(routeMap, catalog); err == nil {
		t.Fatal("maintenance Reading Map accepted a stale/invalid binding")
	}
}

func TestCodexContextProfileIsTelemetryOnlyAtEveryRatio(t *testing.T) {
	for _, material := range []int{500, 600, 701} {
		routeMap := readingMap{
			ContextProfile: contextProfile{
				Executor: codexExecutor, ProfileID: codexContextProfileID, Enforcement: false,
				CapacityBytes: 1000, Measurement: contextMeasurement,
			},
			AlwaysRead:   []routeSection{{MaterialBytes: material}},
			ReadOnDemand: []routeSection{{Path: "optional", SectionID: "OPTIONAL", MaterialBytes: 1}},
		}
		if err := applyContextBudget(&routeMap); err != nil {
			t.Fatalf("Codex telemetry route at %d bytes failed: %v", material, err)
		}
		if routeMap.Budget.Status != "TELEMETRY_ONLY" || routeMap.Budget.ActualRatio <= 0 ||
			routeMap.Budget.ReductionApplied || len(routeMap.ReadOnDemand) != 1 || len(routeMap.SoftLimitOmissions) != 0 {
			t.Fatalf("Codex profile applied a context gate at %d bytes: %+v", material, routeMap)
		}
		if err := validateContextBudget(routeMap); err != nil {
			t.Fatalf("Codex telemetry budget failed validation: %v", err)
		}
	}
	withoutCapacity := readingMap{
		ContextProfile: contextProfile{Executor: codexExecutor, ProfileID: codexContextProfileID, Measurement: contextMeasurement},
		AlwaysRead:     []routeSection{{MaterialBytes: 900}},
	}
	if err := applyContextBudget(&withoutCapacity); err != nil || withoutCapacity.Budget.MaterialBytes != 900 || withoutCapacity.Budget.ActualRatio != 0 {
		t.Fatalf("Codex optional capacity telemetry failed: budget=%+v err=%v", withoutCapacity.Budget, err)
	}
	withoutCapacity.Budget.MaterialBytes++
	if err := validateContextBudget(withoutCapacity); err == nil {
		t.Fatal("invalid Codex telemetry measurement unexpectedly passed")
	}
}

func TestOpenCodeDeepSeekContextProfileEnforcesExplicitBudget(t *testing.T) {
	a := testApp(t)
	catalog := testV1MilestoneCatalog(t, a)
	request, err := parseCodexPlanRequest([]string{
		"--milestone", "M1", "--executor", openCodeExecutor, "--profile", openCodeContextProfileID,
	})
	if err != nil {
		t.Fatal(err)
	}
	if err := validateCodexRouteRequest(request, catalog); err == nil || !strings.Contains(err.Error(), "PROFILE_REQUIRED") {
		t.Fatalf("OpenCode request without explicit capacity did not fail closed: %v", err)
	}
	request.ContextCapacityBytes = 1000
	if err := validateCodexRouteRequest(request, catalog); err != nil {
		t.Fatalf("OpenCode request with explicit profile capacity failed: %v", err)
	}

	makeRoute := func(always, onDemand int) readingMap {
		return readingMap{
			ContextProfile: contextProfile{
				Executor: openCodeExecutor, ProfileID: openCodeContextProfileID, Enforcement: true,
				CapacityBytes: 1000, Measurement: contextMeasurement,
			},
			AlwaysRead:   []routeSection{{MaterialBytes: always}},
			ReadOnDemand: []routeSection{{Path: "optional", SectionID: "OPTIONAL", MaterialBytes: onDemand}},
		}
	}
	at55 := makeRoute(550, 0)
	if err := applyContextBudget(&at55); err != nil || at55.Budget.Status != "WITHIN_BUDGET" || at55.Budget.ActualRatio != 0.55 {
		t.Fatalf("55%% boundary mishandled: budget=%+v err=%v", at55.Budget, err)
	}
	reduced := makeRoute(500, 100)
	if err := applyContextBudget(&reduced); err != nil || reduced.Budget.Status != "SOFT_LIMIT_REDUCED" ||
		reduced.Budget.ActualRatio != 0.5 || len(reduced.ReadOnDemand) != 0 || len(reduced.SoftLimitOmissions) != 1 {
		t.Fatalf("soft-limit route was not deterministically reduced: budget=%+v err=%v", reduced.Budget, err)
	}
	at70 := makeRoute(700, 0)
	if err := applyContextBudget(&at70); err != nil || at70.Budget.Status != "SOFT_LIMIT_EXCEEDED_REVIEW_REQUIRED" || at70.Budget.ActualRatio != 0.70 {
		t.Fatalf("70%% boundary mishandled: budget=%+v err=%v", at70.Budget, err)
	}
	over70 := makeRoute(701, 0)
	if err := applyContextBudget(&over70); err == nil || !strings.Contains(err.Error(), "split") {
		t.Fatalf("hard limit did not require route shrink/split: %v", err)
	}
	missingCapacity := contextProfile{Executor: openCodeExecutor, ProfileID: openCodeContextProfileID, Enforcement: true, Measurement: contextMeasurement}
	if err := validateContextProfile(missingCapacity); err == nil || !strings.Contains(err.Error(), "PROFILE_REQUIRED") {
		t.Fatalf("missing OpenCode capacity did not fail closed: %v", err)
	}
	unknown := contextProfile{Executor: "unknown", ProfileID: "unknown", Measurement: contextMeasurement}
	if err := validateContextProfile(unknown); err == nil || !strings.Contains(err.Error(), "PROFILE_REQUIRED") {
		t.Fatalf("unknown profile did not fail closed: %v", err)
	}
}

func TestBoundedNormativeSectionCatalogCoversAuthorityContracts(t *testing.T) {
	a := testApp(t)
	catalog, err := a.normativeSectionCatalog()
	if err != nil {
		t.Fatal(err)
	}
	_, requirements, _, _, err := a.loadAuthority()
	if err != nil {
		t.Fatal(err)
	}
	for _, requirement := range requirements.Requirements {
		if _, err := resolveNormativeSection(catalog, requirement.OwningSpec); err != nil {
			t.Errorf("requirement %s owning spec %s is not uniquely cataloged: %v", requirement.ID, requirement.OwningSpec, err)
		}
	}
	plan, err := loadYAML[milestonePlan](a.root, ".codex/state/MILESTONE_PLAN.yaml")
	if err != nil {
		t.Fatal(err)
	}
	for _, batch := range plan.Batches {
		for _, sectionID := range batch.ReadingMapSections {
			if _, err := resolveNormativeSection(catalog, sectionID); err != nil {
				t.Errorf("batch %s frozen Section ID %s is not uniquely cataloged: %v", batch.BatchID, sectionID, err)
			}
		}
	}
	for _, path := range normativeSpecificationPaths {
		if err := validateBoundedRoutePath(path); err != nil {
			t.Errorf("bounded normative inventory contains %q: %v", path, err)
		}
	}
}

func TestFrozenReadingMapSectionsMaterializeAndBind(t *testing.T) {
	expected := map[string]string{
		"SPEC-LUA-RUNTIME-001": "docs/20-architecture/LUA_RUNTIME.md",
		"SPEC-PACKAGE-001":     "docs/30-package-spec/PACKAGE_MODEL.md",
		"SPEC-SECURITY-001":    "docs/40-security/SECURITY_MODEL.md",
		"SPEC-QUALITY-001":     "docs/60-quality/TEST_STRATEGY.md",
	}
	a := newFrozenRouteFixture(t, []string{
		"SPEC-LUA-RUNTIME-001", "SPEC-PACKAGE-001", "SPEC-SECURITY-001", "SPEC-QUALITY-001",
	})
	for _, mode := range []string{"IMPLEMENT", "ACCEPT", "REPAIR"} {
		t.Run(mode, func(t *testing.T) {
			routeMap := generateFixtureBatchRoute(t, a, mode)
			counts := map[string]int{}
			for _, section := range allReadingMapSections(routeMap) {
				if path, wanted := expected[section.SectionID]; wanted {
					counts[section.SectionID]++
					if section.Path != path {
						t.Errorf("%s resolved to %s, want %s", section.SectionID, section.Path, path)
					}
					if len(section.SHA256) != 64 || len(section.SectionSHA256) != 64 || section.MaterialBytes == 0 {
						t.Errorf("%s lacks file/Section SHA-256 material binding: %+v", section.SectionID, section)
					}
				}
			}
			for sectionID := range expected {
				if counts[sectionID] != 1 {
					t.Errorf("%s route materialized %s %d times, want exactly once", mode, sectionID, counts[sectionID])
				}
			}
			binding, err := readingMapBindingDigest(routeMap)
			if err != nil {
				t.Fatal(err)
			}
			if routeMap.RouteBindingSHA256 != binding {
				t.Fatalf("%s route binding does not cover its materialized sections", mode)
			}
			if err := a.checkCodex(context.Background()); err != nil {
				t.Fatalf("%s fixture route failed codex check: %v", mode, err)
			}
		})
	}
}

func TestFrozenReadingMapSectionResolutionFailsClosed(t *testing.T) {
	baseSections := []string{"SPEC-LUA-RUNTIME-001", "SPEC-PACKAGE-001", "SPEC-SECURITY-001", "SPEC-QUALITY-001"}
	t.Run("missing", func(t *testing.T) {
		sections := append(append([]string(nil), baseSections...), "SPEC-DOES-NOT-EXIST")
		a := newFrozenRouteFixture(t, sections)
		err := a.generateCodexRouteRequest(context.Background(), testCodexRequest("REPAIR", "M1", "M1-B001"))
		if err == nil || !strings.Contains(err.Error(), "found 0") {
			t.Fatalf("missing frozen Section ID did not fail route generation: %v", err)
		}
	})
	t.Run("ambiguous", func(t *testing.T) {
		a := newFrozenRouteFixture(t, baseSections)
		appendFixtureText(t, a, "docs/60-quality/TEST_STRATEGY.md", "\n<a id=\"SPEC-QUALITY-001\"></a>\nfixture duplicate\n")
		commitFixturePaths(t, a, "duplicate authoritative Section ID", "docs/60-quality/TEST_STRATEGY.md")
		err := a.generateCodexRouteRequest(context.Background(), testCodexRequest("REPAIR", "M1", "M1-B001"))
		if err == nil || !strings.Contains(err.Error(), "found 2") {
			t.Fatalf("ambiguous frozen Section ID did not fail route generation: %v", err)
		}
	})
}

func TestFrozenReadingMapChangesInvalidateExistingRoute(t *testing.T) {
	sections := []string{"SPEC-LUA-RUNTIME-001", "SPEC-PACKAGE-001", "SPEC-SECURITY-001", "SPEC-QUALITY-001"}
	t.Run("supplemental material", func(t *testing.T) {
		a := newFrozenRouteFixture(t, sections)
		generateFixtureBatchRoute(t, a, "REPAIR")
		appendFixtureText(t, a, "docs/40-security/SECURITY_MODEL.md", "\nfixture supplemental material change\n")
		if err := a.checkCodex(context.Background()); err == nil {
			t.Fatal("codex check accepted a route after supplemental frozen Section material changed")
		}
	})
	t.Run("frozen contract removal", func(t *testing.T) {
		a := newFrozenRouteFixture(t, sections)
		generateFixtureBatchRoute(t, a, "REPAIR")
		plan, err := loadYAML[milestonePlan](a.root, ".codex/state/MILESTONE_PLAN.yaml")
		if err != nil {
			t.Fatal(err)
		}
		plan.Batches[0].ReadingMapSections = []string{"SPEC-LUA-RUNTIME-001", "SPEC-PACKAGE-001", "SPEC-QUALITY-001"}
		plan.Batches[0].FrozenContractSHA256 = mustBatchContractDigest(t, plan.Batches[0])
		writeFixtureMilestonePlan(t, a, plan)
		if err := a.checkCodex(context.Background()); err == nil {
			t.Fatal("codex check accepted an existing route after frozen reading_map_sections removal")
		}
	})
}

func TestCurrentB002FrozenReadingMapExactCoverage(t *testing.T) {
	a := testApp(t)
	plan, err := loadYAML[milestonePlan](a.root, ".codex/state/MILESTONE_PLAN.yaml")
	if err != nil {
		t.Fatal(err)
	}
	var batch *milestoneBatch
	for index := range plan.Batches {
		if plan.Batches[index].BatchID == "M1-B002" {
			batch = &plan.Batches[index]
			break
		}
	}
	if batch == nil {
		t.Fatal("current milestone plan has no M1-B002 frozen contract")
	}
	if err := validateMilestoneBatch(*batch); err != nil {
		t.Fatalf("current M1-B002 frozen contract is invalid: %v", err)
	}
	expected := map[string]string{
		"SPEC-V1-ROADMAP-M1":   "docs/80-roadmap/V1_MILESTONES.md",
		"SPEC-M1-ALLOWED":      "docs/80-roadmap/M1_SCOPE_AND_EXIT_GATE.md",
		"SPEC-M1-FORBIDDEN":    "docs/80-roadmap/M1_SCOPE_AND_EXIT_GATE.md",
		"SPEC-M1-EXIT":         "docs/80-roadmap/M1_SCOPE_AND_EXIT_GATE.md",
		"SPEC-LUA-RUNTIME-001": "docs/20-architecture/LUA_RUNTIME.md",
		"SPEC-PACKAGE-001":     "docs/30-package-spec/PACKAGE_MODEL.md",
		"SPEC-SECURITY-001":    "docs/40-security/SECURITY_MODEL.md",
		"SPEC-QUALITY-001":     "docs/60-quality/TEST_STRATEGY.md",
	}
	for _, mode := range []string{"IMPLEMENT", "ACCEPT", "REPAIR"} {
		paths, err := a.routePaths(testCodexRequest(mode, "M1", "M1-B002"))
		if err != nil {
			t.Fatalf("%s current M1-B002 route failed: %v", mode, err)
		}
		counts := map[string]int{}
		resolvedPaths := map[string]string{}
		for _, group := range [][]routeSpec{paths.always, paths.normative, paths.machine, paths.onDemand} {
			for _, spec := range group {
				counts[spec.sectionID]++
				resolvedPaths[spec.sectionID] = spec.path
			}
		}
		for _, sectionID := range batch.ReadingMapSections {
			if counts[sectionID] != 1 {
				t.Errorf("%s current M1-B002 route materialized frozen %s %d times", mode, sectionID, counts[sectionID])
			}
		}
		for sectionID, path := range expected {
			if counts[sectionID] != 1 || resolvedPaths[sectionID] != path {
				t.Errorf("%s current M1-B002 route resolved %s as %q with count %d; want %s exactly once", mode, sectionID, resolvedPaths[sectionID], counts[sectionID], path)
			}
		}
	}
}

func newFrozenRouteFixture(t *testing.T, readingMapSections []string) *App {
	t.Helper()
	source := testApp(t)
	root := filepath.Join(t.TempDir(), "repo")
	command := exec.Command("git", "clone", "--quiet", "--shared", source.root, root)
	if output, err := command.CombinedOutput(); err != nil {
		t.Fatalf("clone route fixture: %v\n%s", err, output)
	}
	a := &App{root: root, stdout: &bytes.Buffer{}, stderr: &bytes.Buffer{}}
	batch := milestoneBatch{
		BatchID: "M1-B001", Sequence: 1, State: "FROZEN", Objective: "isolated frozen reading-map route fixture",
		NonGoals: []string{"product implementation"}, Requirements: []string{"REQ-LUA-001"},
		AllowedScope: []string{"internal/projectctl"}, ForbiddenScope: []string{"apps"},
		MachineContracts:   []string{"SCHEMA-CODEX-MILESTONE-PLAN-V2"},
		ReadingMapSections: append([]string(nil), readingMapSections...), Acceptance: []string{"isolated route acceptance"},
		Tests: []string{"go test ./internal/projectctl"}, StopConditions: []string{"product contract change"}, DependsOn: []string{},
		ParallelSafe: false, DependencyIndependent: false, ParallelScopeKeys: []string{},
	}
	batch.FrozenContractSHA256 = mustBatchContractDigest(t, batch)
	plan := milestonePlan{
		SchemaVersion: milestonePlanSchemaVersion, PlanID: "M1-MILESTONE-PLAN", PlanVersion: 1,
		Milestone: "M1", Status: "ACTIVE", ModifiableOnlyInMode: "PLAN", NextBatchSequence: 2,
		Batches: []milestoneBatch{batch}, Tombstones: []batchTombstone{},
	}
	writeFixtureMilestonePlan(t, a, plan)
	commitFixturePaths(t, a, "isolated milestone plan", ".codex/state/MILESTONE_PLAN.yaml")
	return a
}

func generateFixtureBatchRoute(t *testing.T, a *App, mode string) readingMap {
	t.Helper()
	if err := a.generateCodexRouteRequest(context.Background(), testCodexRequest(mode, "M1", "M1-B001")); err != nil {
		t.Fatalf("generate %s fixture route: %v", mode, err)
	}
	routeMap, err := loadYAML[readingMap](a.root, ".codex/runtime/READING_MAP.yaml")
	if err != nil {
		t.Fatal(err)
	}
	return routeMap
}

func writeFixtureMilestonePlan(t *testing.T, a *App, plan milestonePlan) {
	t.Helper()
	data, err := yaml.Marshal(plan)
	if err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(a.root, ".codex", "state", "MILESTONE_PLAN.yaml"), data, 0o644); err != nil {
		t.Fatal(err)
	}
}

func appendFixtureText(t *testing.T, a *App, relative, text string) {
	t.Helper()
	file, err := os.OpenFile(filepath.Join(a.root, filepath.FromSlash(relative)), os.O_APPEND|os.O_WRONLY, 0)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := file.WriteString(text); err != nil {
		file.Close()
		t.Fatal(err)
	}
	if err := file.Close(); err != nil {
		t.Fatal(err)
	}
}

func commitFixturePaths(t *testing.T, a *App, message string, paths ...string) {
	t.Helper()
	runFixtureGit(t, a.root, append([]string{"add", "--"}, paths...)...)
	runFixtureGit(t, a.root,
		"-c", "user.name=Codex Fixture", "-c", "user.email=fixture@example.invalid",
		"commit", "--quiet", "--no-gpg-sign", "-m", message,
	)
}

func runFixtureGit(t *testing.T, root string, arguments ...string) {
	t.Helper()
	command := exec.Command("git", append([]string{"-C", root}, arguments...)...)
	if output, err := command.CombinedOutput(); err != nil {
		t.Fatalf("git %s: %v\n%s", strings.Join(arguments, " "), err, output)
	}
}

// x-section-id: PROJECTCTL-MILESTONE-LIFECYCLE-TESTS
func TestTrackedMilestonePlanMatchesItsLifecycleState(t *testing.T) {
	a := testApp(t)
	catalog := testV1MilestoneCatalog(t, a)
	plan, err := loadYAML[milestonePlan](a.root, ".codex/state/MILESTONE_PLAN.yaml")
	if err != nil {
		t.Fatal(err)
	}
	if err := validateMilestonePlan(plan, catalog); err != nil {
		t.Fatal(err)
	}
	switch plan.Status {
	case "NOT_GENERATED":
		if plan.PlanVersion != 0 || plan.NextBatchSequence != 1 || len(plan.Batches) != 0 || len(plan.Tombstones) != 0 {
			t.Fatalf("tracked NOT_GENERATED plan violates lifecycle invariants: %+v", plan)
		}
	case "ACTIVE":
		if plan.PlanVersion < 1 || plan.NextBatchSequence < 2 {
			t.Fatalf("tracked ACTIVE plan violates lifecycle invariants: %+v", plan)
		}
	case "COMPLETE":
		if plan.PlanVersion < 1 {
			t.Fatalf("tracked COMPLETE plan violates lifecycle invariants: %+v", plan)
		}
		for _, batch := range plan.Batches {
			if batch.State != "COMPLETED" {
				t.Fatalf("tracked COMPLETE plan contains non-terminal batch %s in %s", batch.BatchID, batch.State)
			}
		}
	default:
		t.Fatalf("tracked plan has unknown status %q", plan.Status)
	}
}

func TestMilestonePlanLifecycleStatesAreValid(t *testing.T) {
	a := testApp(t)
	catalog := testV1MilestoneCatalog(t, a)
	notGenerated := emptyMilestonePlan()
	if err := validateMilestonePlan(notGenerated, catalog); err != nil {
		t.Fatalf("valid NOT_GENERATED fixture failed: %v", err)
	}
	versionZeroWithBatch := notGenerated
	versionZeroWithBatch.Batches = []milestoneBatch{testMilestoneBatch("M0-B001", 1)}
	if err := validateMilestonePlan(versionZeroWithBatch, catalog); err == nil {
		t.Fatal("version-0 plan with a batch unexpectedly passed")
	}
	active := activeMilestonePlan(1, 2, []milestoneBatch{testMilestoneBatch("M0-B001", 1)}, nil)
	if err := validateMilestonePlan(active, catalog); err != nil {
		t.Fatalf("valid ACTIVE fixture failed: %v", err)
	}
	completedBatch := testMilestoneBatch("M0-B001", 1)
	completedBatch.State = "COMPLETED"
	completedBatch.FrozenContractSHA256 = mustBatchContractDigest(t, completedBatch)
	complete := activeMilestonePlan(2, 2, []milestoneBatch{completedBatch}, nil)
	complete.Status = "COMPLETE"
	if err := validateMilestonePlan(complete, catalog); err != nil {
		t.Fatalf("valid COMPLETE fixture failed: %v", err)
	}
}

func TestClosedM0RemainsHistoricallyExplainableButInactive(t *testing.T) {
	a := testApp(t)
	catalog := testV1MilestoneCatalog(t, a)
	request := testCodexRequest("PLAN", "M0", "")
	if err := validateCodexRouteRequest(request, catalog); err != nil {
		t.Fatalf("known historical M0 request became syntactically invalid: %v", err)
	}
	paths, err := a.routePaths(request)
	if err != nil {
		t.Fatal(err)
	}
	foundM0Scope := false
	for _, spec := range paths.normative {
		if spec.path == "docs/80-roadmap/M1_SCOPE_AND_EXIT_GATE.md" {
			t.Fatal("historical M0 route was rebound to M1 scope")
		}
		if spec.path == "docs/80-roadmap/M0_SCOPE_AND_EXIT_GATE.md" && spec.sectionID == "SPEC-M0-EXIT" {
			foundM0Scope = true
		}
	}
	if !foundM0Scope {
		t.Fatal("historical M0 route lost its M0 exit gate")
	}
	currentPlan, err := loadYAML[milestonePlan](a.root, ".codex/state/MILESTONE_PLAN.yaml")
	if err != nil {
		t.Fatal(err)
	}
	if err := validateRouteAgainstPlan(request, currentPlan, catalog); err == nil {
		t.Fatal("closed M0 was reinterpreted as the active planning boundary")
	}
	for _, relative := range []string{"schemas/codex/reading-map-v2.schema.json", "schemas/codex/milestone-plan-v1.schema.json"} {
		data, err := os.ReadFile(filepath.Join(a.root, filepath.FromSlash(relative)))
		if err != nil {
			t.Fatal(err)
		}
		if !bytes.Contains(data, []byte(`"const": "M0"`)) {
			t.Fatalf("legacy M0 schema semantics were silently rewritten in %s", relative)
		}
	}
	status, err := os.ReadFile(filepath.Join(a.root, ".codex/state/MILESTONE_STATUS.md"))
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Contains(status, []byte("M0 result: `PASS / MERGED / CLOSED`")) {
		t.Error("milestone history lost the closed M0 result")
	}
}

func TestMilestonePlanAdvancesOnlyToNextNotGeneratedBaseline(t *testing.T) {
	a := testApp(t)
	catalog := testV1MilestoneCatalog(t, a)
	completedBatch := testMilestoneBatch("M0-B001", 1)
	completedBatch.State = "COMPLETED"
	completedBatch.FrozenContractSHA256 = mustBatchContractDigest(t, completedBatch)
	previous := activeMilestonePlan(4, 2, []milestoneBatch{completedBatch}, nil)
	previous.Status = "COMPLETE"
	proposed := milestonePlan{
		SchemaVersion: milestonePlanSchemaVersion, PlanID: "M1-MILESTONE-PLAN", Milestone: "M1",
		PlanVersion: 0, Status: "NOT_GENERATED", ModifiableOnlyInMode: "PLAN", NextBatchSequence: 1,
		Batches: []milestoneBatch{}, Tombstones: []batchTombstone{},
	}
	if err := validateMilestonePlanChange(previous, proposed, "PLAN", catalog); err != nil {
		t.Fatalf("valid M0 to M1 planning-boundary transition failed: %v", err)
	}
	if err := validateMilestonePlanChange(previous, proposed, "REPAIR", catalog); err == nil {
		t.Fatal("REPAIR mode changed the milestone plan")
	}
	skipped := proposed
	skipped.PlanID, skipped.Milestone = "M2-MILESTONE-PLAN", "M2"
	if err := validateMilestonePlanChange(previous, skipped, "PLAN", catalog); err == nil {
		t.Fatal("milestone plan skipped a known V1 milestone")
	}
}

func TestMilestonePlanIsWritableOnlyInPlanModeAndIDsNeverReuse(t *testing.T) {
	a := testApp(t)
	catalog := testV1MilestoneCatalog(t, a)
	previous := emptyMilestonePlan()
	proposed := activeMilestonePlan(1, 2, []milestoneBatch{testMilestoneBatch("M0-B001", 1)}, nil)
	if err := validateMilestonePlanChange(previous, proposed, "PLAN", catalog); err != nil {
		t.Fatal(err)
	}
	for _, mode := range []string{"IMPLEMENT", "ACCEPT", "REPAIR"} {
		if err := validateMilestonePlanChange(previous, proposed, mode, catalog); err == nil {
			t.Fatalf("%s mode unexpectedly modified the plan", mode)
		}
	}

	reused := proposed
	reused.Tombstones = []batchTombstone{{BatchID: "M0-B001", Sequence: 1, Action: "CANCELLED", Reason: "reuse test"}}
	if err := validateMilestonePlan(reused, catalog); err == nil {
		t.Fatal("tombstoned batch ID reuse unexpectedly passed")
	}
	skipped := activeMilestonePlan(1, 3, []milestoneBatch{testMilestoneBatch("M0-B002", 2)}, nil)
	if err := validateMilestonePlanChange(previous, skipped, "PLAN", catalog); err == nil {
		t.Fatal("non-monotonic batch allocation unexpectedly passed")
	}
}

func TestMilestonePlanSplitMergeCancelPreserveTombstones(t *testing.T) {
	a := testApp(t)
	catalog := testV1MilestoneCatalog(t, a)
	original := activeMilestonePlan(1, 2, []milestoneBatch{testMilestoneBatch("M0-B001", 1)}, nil)
	cancelled := activeMilestonePlan(2, 2, nil, []batchTombstone{{
		BatchID: "M0-B001", Sequence: 1, Action: "CANCELLED", Reason: "no longer needed", ReplacementIDs: []string{},
	}})
	if err := validateMilestonePlanChange(original, cancelled, "PLAN", catalog); err != nil {
		t.Fatalf("valid cancellation failed: %v", err)
	}
	modifiedTombstone := cancelled
	modifiedTombstone.PlanVersion++
	modifiedTombstone.Tombstones = append([]batchTombstone(nil), cancelled.Tombstones...)
	modifiedTombstone.Tombstones[0].Reason = "rewritten history"
	if err := validateMilestonePlanChange(cancelled, modifiedTombstone, "PLAN", catalog); err == nil {
		t.Fatal("immutable tombstone rewrite unexpectedly passed")
	}

	split := activeMilestonePlan(2, 4, []milestoneBatch{
		testMilestoneBatch("M0-B002", 2), testMilestoneBatch("M0-B003", 3),
	}, []batchTombstone{{
		BatchID: "M0-B001", Sequence: 1, Action: "SPLIT", Reason: "single objective per replacement", ReplacementIDs: []string{"M0-B002", "M0-B003"},
	}})
	if err := validateMilestonePlanChange(original, split, "PLAN", catalog); err != nil {
		t.Fatalf("valid split failed: %v", err)
	}
	withoutTombstone := split
	withoutTombstone.Tombstones = nil
	if err := validateMilestonePlanChange(original, withoutTombstone, "PLAN", catalog); err == nil {
		t.Fatal("split without tombstone unexpectedly passed")
	}

	mergeSource := activeMilestonePlan(1, 3, []milestoneBatch{
		testMilestoneBatch("M0-B001", 1), testMilestoneBatch("M0-B002", 2),
	}, nil)
	merged := activeMilestonePlan(2, 4, []milestoneBatch{testMilestoneBatch("M0-B003", 3)}, []batchTombstone{
		{BatchID: "M0-B001", Sequence: 1, Action: "MERGED", Reason: "same objective", ReplacementIDs: []string{"M0-B003"}},
		{BatchID: "M0-B002", Sequence: 2, Action: "MERGED", Reason: "same objective", ReplacementIDs: []string{"M0-B003"}},
	})
	if err := validateMilestonePlanChange(mergeSource, merged, "PLAN", catalog); err != nil {
		t.Fatalf("valid merge failed: %v", err)
	}
}

func TestMilestonePlanFreezeTransitionsAndParallelSafety(t *testing.T) {
	a := testApp(t)
	catalog := testV1MilestoneCatalog(t, a)
	plannedBatch := testMilestoneBatch("M0-B001", 1)
	plannedBatch.State = "PLANNED"
	planned := activeMilestonePlan(1, 2, []milestoneBatch{plannedBatch}, nil)
	frozenBatch := plannedBatch
	frozenBatch.State = "FROZEN"
	frozenBatch.FrozenContractSHA256 = mustBatchContractDigest(t, frozenBatch)
	frozen := activeMilestonePlan(2, 2, []milestoneBatch{frozenBatch}, nil)
	if err := validateMilestonePlanChange(planned, frozen, "PLAN", catalog); err != nil {
		t.Fatalf("valid freeze failed: %v", err)
	}
	implementingBatch := frozenBatch
	implementingBatch.State = "IMPLEMENTING"
	implementing := activeMilestonePlan(3, 2, []milestoneBatch{implementingBatch}, nil)
	if err := validateMilestonePlanChange(frozen, implementing, "PLAN", catalog); err != nil {
		t.Fatalf("valid frozen-to-implementing transition failed: %v", err)
	}
	changed := frozenBatch
	changed.Objective = "silently expanded objective"
	changed.FrozenContractSHA256 = mustBatchContractDigest(t, changed)
	changedPlan := activeMilestonePlan(3, 2, []milestoneBatch{changed}, nil)
	if err := validateMilestonePlanChange(frozen, changedPlan, "PLAN", catalog); err == nil {
		t.Fatal("silent frozen-contract change unexpectedly passed")
	}
	skipped := plannedBatch
	skipped.State = "IMPLEMENTING"
	skipped.FrozenContractSHA256 = mustBatchContractDigest(t, skipped)
	if err := validateMilestonePlanChange(planned, activeMilestonePlan(2, 2, []milestoneBatch{skipped}, nil), "PLAN", catalog); err == nil {
		t.Fatal("illegal PLANNED-to-IMPLEMENTING transition unexpectedly passed")
	}

	left := testMilestoneBatch("M0-B001", 1)
	left.ParallelSafe, left.DependencyIndependent, left.ParallelScopeKeys = true, true, []string{"apps/web-player"}
	right := testMilestoneBatch("M0-B002", 2)
	right.ParallelSafe, right.DependencyIndependent, right.ParallelScopeKeys = true, true, []string{"apps/creator-studio"}
	parallel := activeMilestonePlan(1, 3, []milestoneBatch{left, right}, nil)
	if err := validateMilestonePlan(parallel, catalog); err != nil {
		t.Fatalf("valid parallel-safe plan failed: %v", err)
	}
	right.ParallelScopeKeys = []string{"apps/web-player"}
	if err := validateMilestonePlan(activeMilestonePlan(1, 3, []milestoneBatch{left, right}, nil), catalog); err == nil {
		t.Fatal("overlapping parallel scopes unexpectedly passed")
	}
	unsafe := testMilestoneBatch("M0-B001", 1)
	unsafe.ParallelSafe = true
	if err := validateMilestonePlan(activeMilestonePlan(1, 2, []milestoneBatch{unsafe}, nil), catalog); err == nil {
		t.Fatal("parallel_safe without evidence unexpectedly passed")
	}
}

func emptyMilestonePlan() milestonePlan {
	return milestonePlan{
		SchemaVersion: milestonePlanSchemaVersion, PlanID: "M0-MILESTONE-PLAN", PlanVersion: 0, Milestone: "M0", Status: "NOT_GENERATED",
		ModifiableOnlyInMode: "PLAN", NextBatchSequence: 1, Batches: []milestoneBatch{}, Tombstones: []batchTombstone{},
	}
}

func activeMilestonePlan(version, next int, batches []milestoneBatch, tombstones []batchTombstone) milestonePlan {
	return milestonePlan{
		SchemaVersion: milestonePlanSchemaVersion, PlanID: "M0-MILESTONE-PLAN", PlanVersion: version, Milestone: "M0", Status: "ACTIVE",
		ModifiableOnlyInMode: "PLAN", NextBatchSequence: next, Batches: batches, Tombstones: tombstones,
	}
}

func testMilestoneBatch(id string, sequence int) milestoneBatch {
	return milestoneBatch{
		BatchID: id, Sequence: sequence, State: "DRAFT", Objective: "one bounded objective",
		NonGoals: []string{}, Requirements: []string{"REQ-GOV-003"}, AllowedScope: []string{"internal/projectctl"},
		ForbiddenScope: []string{"apps"}, MachineContracts: []string{"SCHEMA-CODEX-MILESTONE-PLAN-V1"},
		ReadingMapSections: []string{"SPEC-CODEX-PLAN"}, Acceptance: []string{"TEST-GOV-003"},
		Tests: []string{"go test ./internal/projectctl"}, StopConditions: []string{"public contract change"}, DependsOn: []string{},
		ParallelSafe: false, DependencyIndependent: false, ParallelScopeKeys: []string{},
	}
}

func mustBatchContractDigest(t *testing.T, batch milestoneBatch) string {
	t.Helper()
	digest, err := batchContractDigest(batch)
	if err != nil {
		t.Fatal(err)
	}
	return digest
}

// x-section-id: PROJECTCTL-PLATFORM-CI-TESTS
func TestPlatformCIProfileResolution(t *testing.T) {
	tests := []struct {
		name        string
		goos        string
		wantProfile platformCIProfileName
		wantError   bool
	}{
		{name: "linux", goos: "linux", wantProfile: platformProfileLinuxCore},
		{name: "windows", goos: "windows", wantProfile: platformProfileWindowsProduct},
		{name: "darwin", goos: "darwin", wantProfile: platformProfileMacOSProduct},
		{name: "unsupported", goos: "freebsd", wantError: true},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			plan, err := platformCIPlanForGOOS(test.goos)
			if test.wantError {
				if err == nil || !strings.Contains(err.Error(), "unsupported CI platform") {
					t.Fatalf("platformCIPlanForGOOS(%q) error = %v, want unsupported platform error", test.goos, err)
				}
				return
			}
			if err != nil {
				t.Fatal(err)
			}
			if plan.profile != test.wantProfile {
				t.Fatalf("platformCIPlanForGOOS(%q) profile = %q, want %q", test.goos, plan.profile, test.wantProfile)
			}
			if !plan.policy {
				t.Fatalf("platformCIPlanForGOOS(%q) omitted the repository policy gate", test.goos)
			}
		})
	}
}

func TestLinuxCorePlanKeepsFullGate(t *testing.T) {
	plan, err := platformCIPlanForGOOS("linux")
	if err != nil {
		t.Fatal(err)
	}
	text := platformCIPlanText(plan)
	requirePlanContains(t, text,
		"command go build ./...",
		"command go test ./...",
		"command go vet ./...",
		"command pnpm -r typecheck",
		"command pnpm -r build",
		"command pnpm -r test",
		"native-studio linux",
		"compose",
	)
}

func TestWindowsProductPlanUsesSupportedAllowlist(t *testing.T) {
	plan, err := platformCIPlanForGOOS("windows")
	if err != nil {
		t.Fatal(err)
	}
	text := platformCIPlanText(plan)
	requirePlanContains(t, text,
		"command go build ./cmd/projectctl",
		"command go build ./cmd/creator-cli",
		"command go test ./internal/projectctl/...",
		"command go test ./internal/package/...",
		"command go test ./cmd/projectctl/...",
		"command go test ./cmd/creator-cli/...",
		"command go test ./apps/creator-studio/...",
		"command go vet ./internal/projectctl/...",
		"command go vet ./internal/package/...",
		"command go vet ./cmd/creator-cli/...",
		"command go vet ./apps/creator-studio/...",
		"command pnpm -r typecheck",
		"command pnpm -r build",
		"command pnpm -r test",
		"native-studio windows",
	)
	for _, forbidden := range []string{
		"command go build ./...",
		"command go test ./...",
		"command go vet ./...",
		"internal/luaruntime",
		"cmd/lua-runner",
		"cmd/platformd",
		"cmd/workerd",
		"compose",
	} {
		if strings.Contains(text, forbidden) {
			t.Errorf("Windows product plan contains forbidden gate %q:\n%s", forbidden, text)
		}
	}
}

func TestMacOSProductPlanMatchesDeclaredSurface(t *testing.T) {
	plan, err := platformCIPlanForGOOS("darwin")
	if err != nil {
		t.Fatal(err)
	}
	if !plan.policy {
		t.Fatal("macOS product plan omitted the repository policy gate")
	}
	text := platformCIPlanText(plan)
	requirePlanContains(t, text,
		"command go build ./cmd/projectctl",
		"command go test ./internal/projectctl/...",
		"command go test ./cmd/projectctl/...",
		"command go vet ./internal/projectctl/...",
		"command go vet ./cmd/projectctl/...",
		"command pnpm -r typecheck",
		"command pnpm -r build",
		"command pnpm -r test",
	)
	for _, forbidden := range []string{
		"command go build ./...",
		"command go test ./...",
		"command go vet ./...",
		"native-studio",
		"compose",
		"creator-cli",
		"internal/luaruntime",
		"cmd/lua-runner",
	} {
		if strings.Contains(text, forbidden) {
			t.Errorf("macOS product plan contains out-of-surface gate %q:\n%s", forbidden, text)
		}
	}
}

func TestWindowsProductPlanPropagatesSupportedSurfaceFailures(t *testing.T) {
	plan, err := platformCIPlanForGOOS("windows")
	if err != nil {
		t.Fatal(err)
	}
	steps := append([]platformCIStep{}, plan.buildSteps...)
	steps = append(steps, plan.testSteps...)
	tests := []struct {
		name  string
		match func(platformCIStep) bool
	}{
		{name: "projectctl", match: func(step platformCIStep) bool { return step.surface == "projectctl Windows build" }},
		{name: "Creator CLI", match: func(step platformCIStep) bool { return step.surface == "Creator CLI Windows build" }},
		{name: "Creator Studio", match: func(step platformCIStep) bool { return step.kind == platformStepNativeStudio }},
		{name: "Web Player", match: func(step platformCIStep) bool { return step.surface == "Web Player and Creator Studio frontend build" }},
		{name: "portable package tests", match: func(step platformCIStep) bool { return step.surface == "portable package tests" }},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			matched := false
			err := executePlatformSteps(context.Background(), steps, func(_ context.Context, step platformCIStep) error {
				if test.match(step) {
					matched = true
					return context.Canceled
				}
				return nil
			})
			if !matched {
				t.Fatal("supported surface has no required Windows step")
			}
			if err == nil || !strings.Contains(err.Error(), context.Canceled.Error()) {
				t.Fatalf("supported surface failure was ignored: %v", err)
			}
		})
	}
}

func TestWindowsProductPlanSkipsLinuxOnlyCoreFixture(t *testing.T) {
	plan, err := platformCIPlanForGOOS("windows")
	if err != nil {
		t.Fatal(err)
	}
	root := writePlatformCIFixture(t)
	if output, err := runFixtureGo(root, "linux", "build", "./..."); err != nil {
		t.Fatalf("Linux fixture Core build failed: %v\n%s", err, output)
	}
	if _, err := runFixtureGo(root, "windows", "build", "./..."); err == nil {
		t.Fatal("fixture-wide Windows build unexpectedly accepted the Linux-only Core package")
	}

	steps := append([]platformCIStep{}, plan.buildSteps...)
	steps = append(steps, plan.testSteps...)
	for _, step := range steps {
		switch {
		case step.kind == platformStepCommand && step.name == "go":
			args := append([]string{}, step.args...)
			if len(args) > 0 && args[0] == "test" {
				args = append([]string{"test", "-exec=true"}, args[1:]...)
			}
			if output, err := runFixtureGo(root, "windows", args...); err != nil {
				t.Fatalf("Windows supported step %q failed because of the isolated Core fixture: %v\n%s", step.surface, err, output)
			}
		case step.kind == platformStepNativeStudio:
			outputPath := filepath.Join(root, "creator-studio.exe")
			if output, err := runFixtureGo(root, "windows", "build", "-tags", "production", "-o", outputPath, "./apps/creator-studio"); err != nil {
				t.Fatalf("Windows native Studio fixture build failed: %v\n%s", err, output)
			}
		}
	}
}

func platformCIPlanText(plan platformCIPlan) string {
	steps := append([]platformCIStep{}, plan.buildSteps...)
	steps = append(steps, plan.testSteps...)
	lines := make([]string, 0, len(steps))
	for _, step := range steps {
		line := string(step.kind)
		if step.name != "" {
			line += " " + step.name
		}
		if len(step.args) != 0 {
			line += " " + strings.Join(step.args, " ")
		}
		if step.nativeStudio != "" {
			line += " " + string(step.nativeStudio)
		}
		lines = append(lines, line)
	}
	return strings.Join(lines, "\n")
}

func requirePlanContains(t *testing.T, text string, wanted ...string) {
	t.Helper()
	for _, item := range wanted {
		if !strings.Contains(text, item) {
			t.Errorf("platform plan omitted %q:\n%s", item, text)
		}
	}
}

func writePlatformCIFixture(t *testing.T) string {
	t.Helper()
	root := t.TempDir()
	files := map[string]string{
		"go.mod":                               "module example.com/platform-ci-fixture\n\ngo 1.26.0\n",
		"cmd/projectctl/main.go":               "package main\n\nimport _ \"example.com/platform-ci-fixture/internal/projectctl\"\n\nfunc main() {}\n",
		"internal/projectctl/projectctl.go":    "package projectctl\n\nconst Portable = true\n",
		"cmd/creator-cli/main.go":              "package main\n\nimport _ \"example.com/platform-ci-fixture/internal/package/model\"\n\nfunc main() {}\n",
		"internal/package/model/model.go":      "package model\n\nconst Portable = true\n",
		"apps/creator-studio/main.go":          "package main\n\nfunc main() {}\n",
		"cmd/lua-runner/main.go":               "package main\n\nimport \"example.com/platform-ci-fixture/internal/luaruntime\"\n\nfunc main() { luaruntime.Run() }\n",
		"internal/luaruntime/runtime_linux.go": "//go:build linux\n\npackage luaruntime\n\nfunc Run() {}\n",
	}
	for relative, content := range files {
		path := filepath.Join(root, filepath.FromSlash(relative))
		if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(path, []byte(content), 0o600); err != nil {
			t.Fatal(err)
		}
	}
	return root
}

func runFixtureGo(root, goos string, args ...string) (string, error) {
	command := exec.Command("go", args...)
	command.Dir = root
	environment := make([]string, 0, len(os.Environ())+4)
	for _, item := range os.Environ() {
		if strings.HasPrefix(item, "GOOS=") || strings.HasPrefix(item, "CGO_ENABLED=") || strings.HasPrefix(item, "GOCACHE=") || strings.HasPrefix(item, "GOFLAGS=") {
			continue
		}
		environment = append(environment, item)
	}
	command.Env = append(environment, "GOOS="+goos, "CGO_ENABLED=0", "GOCACHE="+filepath.Join(root, ".gocache"), "GOFLAGS=-buildvcs=false")
	output, err := command.CombinedOutput()
	return string(output), err
}
