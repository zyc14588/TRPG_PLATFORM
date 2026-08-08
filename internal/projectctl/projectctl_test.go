// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package projectctl

import (
	"bytes"
	"context"
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

func TestRoutesStayProgressive(t *testing.T) {
	a := testApp(t)
	for _, mode := range []string{"PLAN", "IMPLEMENT", "ACCEPT", "REPAIR"} {
		paths := routePaths(mode)
		if len(paths.always) == 0 || len(paths.machine) == 0 {
			t.Fatalf("%s route is incomplete", mode)
		}
		all := append(append(append(paths.always, paths.normative...), paths.machine...), paths.onDemand...)
		seen := map[string]bool{}
		for _, spec := range all {
			if err := validateBoundedRoutePath(spec.path); err != nil {
				t.Fatalf("%s route: %v", mode, err)
			}
			key := spec.path + "#" + spec.sectionID
			if seen[key] {
				t.Fatalf("%s route repeats %s", mode, key)
			}
			seen[key] = true
			data, err := os.ReadFile(filepath.Join(a.root, filepath.FromSlash(spec.path)))
			if err != nil {
				t.Fatal(err)
			}
			if _, err := sectionMaterial(data, spec.sectionID); err != nil {
				t.Fatalf("%s route has invalid Section ID %s: %v", mode, key, err)
			}
		}
	}
}

func TestGeneratedReadingMapContractIsMachineValid(t *testing.T) {
	a := testApp(t)
	for _, mode := range []string{"PLAN", "IMPLEMENT", "ACCEPT", "REPAIR"} {
		specs := routePaths(mode)
		routeMap := readingMap{
			RouteSchemaVersion: readingMapSchemaVersion,
			Mode:               mode, Milestone: "M0", BatchID: "M0-B001",
			ContextProfile:  contextProfile{ProfileID: contextProfileID, CapacityBytes: defaultContextCapacity, Measurement: contextMeasurement},
			MustNotBulkRead: []string{"docs/**", "git-history", "prior-chat-transcripts"},
		}
		var err error
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
		if err := validateReadingMapMetadata(routeMap); err != nil {
			t.Fatalf("%s reading map contract is invalid: %v", mode, err)
		}
		routeMap.BatchID = "M0-B999"
		if err := validateReadingMapMetadata(routeMap); err == nil {
			t.Fatalf("%s reading map accepted a batch ID detached from its binding", mode)
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
	request, err := parseCodexRouteRequest([]string{
		"--mode", "repair", "--milestone", "M0", "--batch", "M0-B007", "--context-capacity-bytes", "4096",
	})
	if err != nil {
		t.Fatal(err)
	}
	if request.Mode != "REPAIR" || request.Milestone != "M0" || request.BatchID != "M0-B007" || request.ContextCapacityBytes != 4096 {
		t.Fatalf("unexpected route request: %+v", request)
	}
	for _, invalid := range []codexRouteRequest{
		{Mode: "IMPLEMENT", Milestone: "M0", BatchID: "M1-B001", ContextCapacityBytes: 1},
		{Mode: "IMPLEMENT", Milestone: "M1", BatchID: "M1-B001", ContextCapacityBytes: 1},
		{Mode: "UNKNOWN", Milestone: "M0", BatchID: "M0-B001", ContextCapacityBytes: 1},
	} {
		if err := validateCodexRouteRequest(invalid); err == nil {
			t.Fatalf("invalid route request unexpectedly passed: %+v", invalid)
		}
	}
	for _, path := range []string{"docs/**", "docs", "../docs/file.md", "git-history", ".git/objects"} {
		if err := validateBoundedRoutePath(path); err == nil {
			t.Fatalf("unbounded route path %q unexpectedly passed", path)
		}
	}
}

func TestContextBudgetBoundariesAndReduction(t *testing.T) {
	makeRoute := func(always, onDemand, capacity int) readingMap {
		return readingMap{
			ContextProfile: contextProfile{ProfileID: contextProfileID, CapacityBytes: capacity, Measurement: contextMeasurement},
			AlwaysRead:     []routeSection{{MaterialBytes: always}},
			ReadOnDemand:   []routeSection{{Path: "optional", SectionID: "OPTIONAL", MaterialBytes: onDemand}},
		}
	}

	at55 := makeRoute(550, 0, 1000)
	if err := applyContextBudget(&at55); err != nil || at55.Budget.Status != "WITHIN_BUDGET" || at55.Budget.ActualRatio != 0.55 {
		t.Fatalf("55%% boundary mishandled: budget=%+v err=%v", at55.Budget, err)
	}
	at70 := makeRoute(700, 0, 1000)
	if err := applyContextBudget(&at70); err != nil || at70.Budget.Status != "SOFT_LIMIT_EXCEEDED_REVIEW_REQUIRED" || at70.Budget.ActualRatio != 0.70 {
		t.Fatalf("70%% boundary mishandled: budget=%+v err=%v", at70.Budget, err)
	}
	over70 := makeRoute(701, 0, 1000)
	if err := applyContextBudget(&over70); err == nil || !strings.Contains(err.Error(), "PLAN") {
		t.Fatalf("hard limit did not require PLAN split: %v", err)
	}
	reduced := makeRoute(500, 100, 1000)
	if err := applyContextBudget(&reduced); err != nil {
		t.Fatal(err)
	}
	if reduced.Budget.Status != "SOFT_LIMIT_REDUCED" || reduced.Budget.ActualRatio != 0.5 || len(reduced.ReadOnDemand) != 0 || len(reduced.SoftLimitOmissions) != 1 {
		t.Fatalf("soft-limit route was not deterministically reduced: %+v", reduced)
	}
}

func TestCurrentMilestonePlanIsValidAndDoesNotPlanM1(t *testing.T) {
	a := testApp(t)
	plan, err := loadYAML[milestonePlan](a.root, ".codex/state/MILESTONE_PLAN.yaml")
	if err != nil {
		t.Fatal(err)
	}
	if err := validateMilestonePlan(plan); err != nil {
		t.Fatal(err)
	}
	if plan.Milestone != "M0" || plan.Status != "NOT_GENERATED" || plan.PlanVersion != 0 || len(plan.Batches) != 0 {
		t.Fatalf("M0 placeholder generated a route prematurely: %+v", plan)
	}
}

func TestMilestonePlanIsWritableOnlyInPlanModeAndIDsNeverReuse(t *testing.T) {
	previous := emptyMilestonePlan()
	proposed := activeMilestonePlan(1, 2, []milestoneBatch{testMilestoneBatch("M0-B001", 1)}, nil)
	if err := validateMilestonePlanChange(previous, proposed, "PLAN"); err != nil {
		t.Fatal(err)
	}
	for _, mode := range []string{"IMPLEMENT", "ACCEPT", "REPAIR"} {
		if err := validateMilestonePlanChange(previous, proposed, mode); err == nil {
			t.Fatalf("%s mode unexpectedly modified the plan", mode)
		}
	}

	reused := proposed
	reused.Tombstones = []batchTombstone{{BatchID: "M0-B001", Sequence: 1, Action: "CANCELLED", Reason: "reuse test"}}
	if err := validateMilestonePlan(reused); err == nil {
		t.Fatal("tombstoned batch ID reuse unexpectedly passed")
	}
	skipped := activeMilestonePlan(1, 3, []milestoneBatch{testMilestoneBatch("M0-B002", 2)}, nil)
	if err := validateMilestonePlanChange(previous, skipped, "PLAN"); err == nil {
		t.Fatal("non-monotonic batch allocation unexpectedly passed")
	}
}

func TestMilestonePlanSplitMergeCancelPreserveTombstones(t *testing.T) {
	original := activeMilestonePlan(1, 2, []milestoneBatch{testMilestoneBatch("M0-B001", 1)}, nil)
	cancelled := activeMilestonePlan(2, 2, nil, []batchTombstone{{
		BatchID: "M0-B001", Sequence: 1, Action: "CANCELLED", Reason: "no longer needed", ReplacementIDs: []string{},
	}})
	if err := validateMilestonePlanChange(original, cancelled, "PLAN"); err != nil {
		t.Fatalf("valid cancellation failed: %v", err)
	}

	split := activeMilestonePlan(2, 4, []milestoneBatch{
		testMilestoneBatch("M0-B002", 2), testMilestoneBatch("M0-B003", 3),
	}, []batchTombstone{{
		BatchID: "M0-B001", Sequence: 1, Action: "SPLIT", Reason: "single objective per replacement", ReplacementIDs: []string{"M0-B002", "M0-B003"},
	}})
	if err := validateMilestonePlanChange(original, split, "PLAN"); err != nil {
		t.Fatalf("valid split failed: %v", err)
	}
	withoutTombstone := split
	withoutTombstone.Tombstones = nil
	if err := validateMilestonePlanChange(original, withoutTombstone, "PLAN"); err == nil {
		t.Fatal("split without tombstone unexpectedly passed")
	}

	mergeSource := activeMilestonePlan(1, 3, []milestoneBatch{
		testMilestoneBatch("M0-B001", 1), testMilestoneBatch("M0-B002", 2),
	}, nil)
	merged := activeMilestonePlan(2, 4, []milestoneBatch{testMilestoneBatch("M0-B003", 3)}, []batchTombstone{
		{BatchID: "M0-B001", Sequence: 1, Action: "MERGED", Reason: "same objective", ReplacementIDs: []string{"M0-B003"}},
		{BatchID: "M0-B002", Sequence: 2, Action: "MERGED", Reason: "same objective", ReplacementIDs: []string{"M0-B003"}},
	})
	if err := validateMilestonePlanChange(mergeSource, merged, "PLAN"); err != nil {
		t.Fatalf("valid merge failed: %v", err)
	}
}

func TestMilestonePlanFreezeTransitionsAndParallelSafety(t *testing.T) {
	plannedBatch := testMilestoneBatch("M0-B001", 1)
	plannedBatch.State = "PLANNED"
	planned := activeMilestonePlan(1, 2, []milestoneBatch{plannedBatch}, nil)
	frozenBatch := plannedBatch
	frozenBatch.State = "FROZEN"
	frozenBatch.FrozenContractSHA256 = mustBatchContractDigest(t, frozenBatch)
	frozen := activeMilestonePlan(2, 2, []milestoneBatch{frozenBatch}, nil)
	if err := validateMilestonePlanChange(planned, frozen, "PLAN"); err != nil {
		t.Fatalf("valid freeze failed: %v", err)
	}
	implementingBatch := frozenBatch
	implementingBatch.State = "IMPLEMENTING"
	implementing := activeMilestonePlan(3, 2, []milestoneBatch{implementingBatch}, nil)
	if err := validateMilestonePlanChange(frozen, implementing, "PLAN"); err != nil {
		t.Fatalf("valid frozen-to-implementing transition failed: %v", err)
	}
	changed := frozenBatch
	changed.Objective = "silently expanded objective"
	changed.FrozenContractSHA256 = mustBatchContractDigest(t, changed)
	changedPlan := activeMilestonePlan(3, 2, []milestoneBatch{changed}, nil)
	if err := validateMilestonePlanChange(frozen, changedPlan, "PLAN"); err == nil {
		t.Fatal("silent frozen-contract change unexpectedly passed")
	}
	skipped := plannedBatch
	skipped.State = "IMPLEMENTING"
	skipped.FrozenContractSHA256 = mustBatchContractDigest(t, skipped)
	if err := validateMilestonePlanChange(planned, activeMilestonePlan(2, 2, []milestoneBatch{skipped}, nil), "PLAN"); err == nil {
		t.Fatal("illegal PLANNED-to-IMPLEMENTING transition unexpectedly passed")
	}

	left := testMilestoneBatch("M0-B001", 1)
	left.ParallelSafe, left.DependencyIndependent, left.ParallelScopeKeys = true, true, []string{"apps/web-player"}
	right := testMilestoneBatch("M0-B002", 2)
	right.ParallelSafe, right.DependencyIndependent, right.ParallelScopeKeys = true, true, []string{"apps/creator-studio"}
	parallel := activeMilestonePlan(1, 3, []milestoneBatch{left, right}, nil)
	if err := validateMilestonePlan(parallel); err != nil {
		t.Fatalf("valid parallel-safe plan failed: %v", err)
	}
	right.ParallelScopeKeys = []string{"apps/web-player"}
	if err := validateMilestonePlan(activeMilestonePlan(1, 3, []milestoneBatch{left, right}, nil)); err == nil {
		t.Fatal("overlapping parallel scopes unexpectedly passed")
	}
	unsafe := testMilestoneBatch("M0-B001", 1)
	unsafe.ParallelSafe = true
	if err := validateMilestonePlan(activeMilestonePlan(1, 2, []milestoneBatch{unsafe}, nil)); err == nil {
		t.Fatal("parallel_safe without evidence unexpectedly passed")
	}
}

func emptyMilestonePlan() milestonePlan {
	return milestonePlan{
		SchemaVersion: 1, PlanID: "M0-MILESTONE-PLAN", PlanVersion: 0, Milestone: "M0", Status: "NOT_GENERATED",
		ModifiableOnlyInMode: "PLAN", NextBatchSequence: 1, Batches: []milestoneBatch{}, Tombstones: []batchTombstone{},
	}
}

func activeMilestonePlan(version, next int, batches []milestoneBatch, tombstones []batchTombstone) milestonePlan {
	return milestonePlan{
		SchemaVersion: 1, PlanID: "M0-MILESTONE-PLAN", PlanVersion: version, Milestone: "M0", Status: "ACTIVE",
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
