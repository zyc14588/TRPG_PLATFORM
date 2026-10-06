//go:build linux && security

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package security_test

import "testing"

// These are executed boundary tests, not copied PASS claims. Each target must
// actually run and pass in the same clean Git source, including its subtests.
// The integration targets build the production Runner/daemon and use owned SQL.
var matrices = []matrix{
	{"identity-seat", false, []target{
		{"internal/session/command", []string{"TestAuthenticatedEnvelopeBoundary", "TestEnvelopeDecodeRejectsAmbiguousWireData"}},
		{"internal/session/realtime", []string{"TestQueuesFilterBeforeCopyRevalidateOnDeliveryAndCloseSlowConsumers"}},
	}},
	{"hidden-view", true, []target{
		{"tests/integration/session", []string{"TestRealCommitBarrierSevenEffectsDuplicateAndSeatFiltering", "TestRealSleepReadOnlyCheckpointReconnectRevocationAndEnd", "TestActualPlatformdTwoSeatWebsocketRestartAndSignalCleanup", "TestActualProjectionCannotCreateAnyMutation", "TestActualReadCallbacksDenyEveryWriteSurface"}},
	}},
	{"tenant", true, []target{
		{"tests/integration/hostapi", []string{"TestExplicitTenantAndSessionStorageBoundary", "TestDatabaseTierAndScopeDenyMatrix"}},
		{"tests/integration/replay", []string{"TestRealTenantGraphACLAndPolicyReauthentication"}},
	}},
	{"sandbox", false, []target{
		{"internal/luaruntime/profile", []string{"TestSourceOnlyAndProductionDenial", "TestCoroutineBudgetAndDeterministicModules", "TestOpaqueIdentitiesCannotEnterDeterministicResults"}},
		{"internal/luaruntime/ipc", []string{"TestBoundedFramingRejectsTruncationAndOversize", "TestCallbackDirectionSequenceAndBindingFailClosed", "TestCallbackFailureCannotBecomeChildPASS", "TestUnsolicitedCallbackAndCancellationAreJoined"}},
		{"cmd/lua-runner", []string{"TestRunnerProductionBoundaryAndBudgets", "TestRunnerRejectsMalformedIPC"}},
	}},
	{"secret", false, []target{
		{"internal/luaruntime/checkpoint", []string{"TestValueDiagnosticsNeverRenderPrivatePayloads", "TestDiagnosticRedactionPreservesExplicitJSONState"}},
		{"internal/luaruntime/vm", []string{"TestAuthorizedOriginAndOpaqueTokenStayInParent", "TestActualExecutionTokenDiagnosticsPreserveAuthority", "TestActualSessionHandleDiagnosticsPreserveAuthorityAndState"}},
		{"internal/hostapi", []string{"TestSevenEffectsAtomicCommitAndIdempotency", "TestAuditControlsCannotDisableOrExposeSecrets"}},
	}},
	{"capability", true, []target{
		{"internal/package/capability", []string{"TestHostRegistryNeverGrantsOutsideThreeLayerIntersection", "TestHostRegistryPreservesZeroGrantDefaults", "TestHostRegistryKeepsPrivilegedAndUnknownNamesClosed", "TestUnknownCapabilityInAllThreeRawSetsStillFailsClosed", "TestResolutionRevalidatesDirectlyConstructedUnknownCapability"}},
		{"internal/package/install", []string{"TestPolicyRequiresActualBoundSignaturesAndIndependentCertification", "TestCapabilitiesAndFallbackEvidenceCannotEscalate", "TestHostCertificationDeniesDefaultsIntersectionsAndUntypedSource", "TestHostDependencyCannotBorrowRootCapabilities"}},
		{"internal/luaruntime/vm", []string{"TestFullPackageHashCopyCannotChangeVMOrModuleAuthority", "TestHostModuleOriginCannotBorrowRootAuthority", "TestHostIntersectionsAndConditionalEntrypoint"}},
		{"tests/integration/hostapi", []string{"TestFullGraphOptionsRejectMissingExtraAndSubstitutedPackages", "TestFullGraphBindingCopiesCannotCreatePassiveScriptAuthority", "TestFullGraphDoesNotGrantLibraryOrPassivePackageAuthority", "TestPassiveFullGraphRetainsDefaultZeroAuthorization"}},
	}},
	{"audit", false, []target{
		{"internal/luaruntime/vm", []string{"TestMandatoryAuditAndFailureRecovery"}},
		{"internal/hostapi", []string{"TestAuditControlsCannotDisableOrExposeSecrets", "TestEveryStagedEffectRollsBackOnFailure"}},
		{"cmd/lua-runner", []string{"TestEvidenceRejectsSelectionOverridesACC009", "TestEvidenceRequiresExecutedTestsACC009", "TestEvidenceCatalogCoversCandidateTests"}},
	}},
	{"resource", false, []target{
		{"internal/session/actor", []string{"TestBoundedMailboxSerializesWrites", "TestIdleReclaimsCapacityBeforeAnotherSessionActivates"}},
		{"internal/luaruntime/profile", []string{"TestExecutionBudgetsAndCancellation", "TestCaughtOutputBudgetIsSticky", "TestPostExecutionConversionPoisonsACC008"}},
		{"internal/luaruntime/ipc", []string{"TestUntrustedOutputAccountingCannotLowerCommandCharge"}},
		{"internal/hostapi", []string{"TestIndependentHostBudgets", "TestOriginalOutputBudgetRollsBackEveryEffect", "TestActiveCallbackCancellationRollsBackAndReaps", "TestActiveCallbackWallDeadlineRollsBack"}},
		{"internal/luaruntime/checkpoint", []string{"TestCheckpointSizeDepthAndTampering"}},
		{"internal/eventstore", []string{"TestIncompleteTamperedOrUnboundedHistoryRejected"}},
	}},
	{"import", false, []target{
		{"internal/package/archive", []string{"TestPreflightRejectsPathsSpecialFilesAndMetadataAttacks", "TestPreflightRejectsLimitsBeforeZIPReader", "TestPreflightRejectsEOCDParserDifferential", "TestImportProjectRejectsSymlinksSpecialFilesAndPortableCollisions", "TestImportProjectEnforcesMetadataLimitsBeforeContentReads", "TestImportFileRejectsSymlinkSpecialOversizeAndInPlaceChange", "TestFromFilesRejectsLimitsBeforeCloningCallerBytes", "TestCanonicalPayloadAggregateUsesRemainingPackageBudget"}},
		{"internal/package/manifest", []string{"TestManifestFailsClosed", "TestManifestConformanceSizeCheckedBeforeJSONParse", "TestR2AdversarialCanonicalConformanceProbes", "TestHostCategorySchemaRemainsClosedForPrivilegedNames"}},
		{"internal/package/install", []string{"TestContentRejectsExecutablesAndUndeclaredBinary", "TestStagingPortableArchiveRejectionsAndCleanup", "TestStagingBindsImmutableSourceAndHonorsCancellation"}},
		{"internal/storage/object", []string{"TestObjectPathsCannotEscapeOrFollowLinks"}},
	}},
	{"migration", true, []target{
		{"tests/integration/migration", []string{"TestUnsafeUndeclaredAndCriticalBoundariesDenyBeforePoint", "TestInFlightCommandAndStaleHeadDenyBeforePoint", "TestPlanSchemaFailureAndPostUpgradeCommandDenyPointRestoration", "TestMigrationAuthorityGraphAndEntryBoundsFailClosed", "TestTargetRestoreRehearsalFailureKeepsOldSessionUsable"}},
		{"tests/integration/replay", []string{"TestRealCorruptAndSelfHashedCacheMismatchesFallBack", "TestRealLegacyIncompleteHistoryFailsClosed"}},
		{"internal/projection", []string{"TestHistoryGapsSchemaSubstitutionAndIncompleteEvidenceFailClosed"}},
		{"internal/luaruntime/vm", []string{"TestACC010InvalidCheckpointPreservesLifecycle", "TestACC010FailedReplacementIsReaped"}},
	}},
	{"cross-session", false, []target{
		{"internal/hostapi", []string{"TestValidationCannotStageEffectsAndTokensDoNotCrossSessions", "TestConcurrentSessionsKeepWorkspacesAndTokensSeparate"}},
		{"internal/session/actor", []string{"TestPanicAndCancellationDoNotAffectOtherSessions", "TestConcurrentSessionsRemainIsolated"}},
		{"internal/luaruntime/vm", []string{"TestMultiSessionRaceIsolation", "TestACC010RestoreIsolationAndMutation", "TestRunnerFaultMemoryAndCancellationIsolation"}},
	}},
}

func TestSecurityBypassMatrices(t *testing.T) {
	for _, m := range matrices {
		t.Run(m.name, func(t *testing.T) { executeMatrix(t, m) })
	}
}
