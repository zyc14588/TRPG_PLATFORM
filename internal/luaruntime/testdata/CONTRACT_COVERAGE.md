# B002 frozen obligation mapping

This map implements the original M1-B002 contract; it does not amend its 14 frozen
fields or the native reading map. Digest:
`5dbcccd9ddda20bbcbdcbff473fb9197b83bb087a10f1ee802ed8ad547d74677`.
Baseline: `bb90989a128e6cdf9c64bde1129496d9baed8478` (accepted B011 lineage).

| Obligation | Implementation | Candidate tests |
| --- | --- | --- |
| Pinned, license-compatible Lua 5.5 profile | profile backend/source compiler, embedded MIT notice, go.mod/sum | TestLua55Conformance; TestRunnerProductionBoundaryAndBudgets |
| Source-only and all production-denied facilities | profile restricted standard library, immutable require, no providers or public listener | TestSourceOnlyAndProductionDenial; TestRunnerProductionBoundaryAndBudgets; TestRunnerRejectsMalformedIPC |
| Separate process and bounded local IPC | ipc length framing, strict schema/version/ID, process lifecycle | TestBoundedFramingRejectsTruncationAndOversize; TestRunnerRejectsMalformedIPC; VM fault tests |
| Long-lived Session globals/modules/coroutines/handles isolated | vm per-process Session, generation tokens, per-engine module/coroutine state; random/time not granted before B004 | TestSessionIsolationLifecycleAndHandles; TestExactDependencyGraphAndModuleIsolation; TestMultiSessionRaceIsolation |
| Basic acyclic checkpoint values and precise bindings | checkpoint validator, canonical hashing, owned copies | TestCheckpointRoundTripAndDeterminism; TestCheckpointRejectsInvalidValuesAndBindings; TestCheckpointSizeDepthAndTampering; TestCheckpointUnicodeAndMaximumValidDepth |
| Authoritative plus compatible checkpoint reconstruction | vm start/reconstruct/capture and successful-current-checkpoint requirement | TestCheckpointReconstructionAndMemoryPressure; TestACC008IPCContaminationAndReconstruction; TestRunnerFaultMemoryAndCancellationIsolation |
| Every post-execution error poisons (ACC-008) | profile conversion included in execution boundary; vm errors/audit failures poison | TestPostExecutionConversionPoisonsACC008; TestACC008IPCContaminationAndReconstruction; TestMandatoryAuditAndFailureRecovery |
| Instruction/CPU, wall, hard memory, recursion, output, cancellation | per-invocation hooks, all-coroutine budgets, kernel limits, runner/parent timers | TestExecutionBudgetsAndCancellation; TestCaughtOutputBudgetIsSticky; TestCoroutineBudgetAndDeterministicModules; TestRunnerProductionBoundaryAndBudgets; TestRunnerFaultMemoryAndCancellationIsolation |
| Mandatory minimum security/error/budget audit | nonoptional AUDIT-0 IPC and host sink; fail closed on sink loss | TestMandatoryAuditAndFailureRecovery; runner/profile negative-path tests |
| Capabilities intersection; required rejection; tested optional fallback | existing capability.Resolve, empty execution grants, immutable fallback proof binding | TestCapabilitiesRequireTestedFallbackOnExactPackage; TestB011PackageInputsAndRejections; generation-token tests |
| Stable traversal/serialization/reconstruction | sorted primitive keys, no pointer formatting, exact package/state recovery | TestCoroutineBudgetAndDeterministicModules; TestOpaqueIdentitiesCannotEnterDeterministicResults; checkpoint/recovery tests |
| B011 v1/v2 generic extension and Creator compatibility | readonly Package.Manifest/Entry/ExactLock/ContentHash adapter; no B011 or Creator changes | TestB011PackageInputsAndRejections; TestExactDependencyGraphAndModuleIsolation; existing package/Creator regression suite |
| Candidate-bound actual test execution (ACC-009) | lua-runner evidence command, source test catalog, structured run/pass verification | TestEvidenceRejectsSelectionOverridesACC009; TestEvidenceRequiresExecutedTestsACC009; TestEvidenceCatalogCoversCandidateTests; frozen-candidate override probe |
| Full required suite and platform support | native projectctl platform matrix unchanged | Independent verifier: just check, just test, go vet ./..., just license-check; applicable B011/platform gates |

Historical ACC-M1-B002-008/009 apply to candidate
`3a1e45b464ac66f4d3592e0574a27810205a6073`, tree
`bce928aa7d3f8ff9e4c67947659f68a335e1684d`; blocking commit
`27bc3b7ecf870a27348516ff950526c4fea5f0ed`. Their obligations remain applicable to
this first implementation on accepted B011 lineage. Builder regression results
do not close either OPEN_HISTORICAL_HIGH finding. Independent ACCEPT must give
explicit disposition against the exact current candidate and preserve old FAIL
evidence. No old c1 code, source epoch or envelope was resumed.

No B003 installation, B004 formal Host API, MutationWorkspace, command processing,
database, AI, Creator refactor or event/replay claim is included. Full Host callback
budgets remain with B004; the absent callbacks cannot bypass B002 execution limits.
