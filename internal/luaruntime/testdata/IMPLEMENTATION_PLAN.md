# M1-B002 implementation plan

Baseline: `bb90989a128e6cdf9c64bde1129496d9baed8478` / tree
`9f3220edb96c9f373ac170ec75edd08d120cd5a9`, M1/v26, IMPLEMENT/builder.
Contract: `5dbcccd9ddda20bbcbdcbff473fb9197b83bb087a10f1ee802ed8ad547d74677`.
Kind: FIRST_IMPLEMENTATION_ON_ACCEPTED_B011_LINEAGE. The frozen plan and
fresh native reading map remain authoritative; this is an implementation checklist.

1. Establish Lua 5.5 conformance and resource-boundary tests, then select a pinned
   license-compatible backend. Implement source-only UTF-8 loading, a restricted
   standard library, hard instruction/time/memory/recursion/output budgets, and
   mandatory minimum security/error audit records.
2. Implement bounded checkpoint values and exact state/package/lock/profile/runtime
   binding. Reject executable values, handles, cycles, metatables and invalid values.
3. Implement isolated long-lived VM lifecycle, validated B011 package inputs,
   capability denial, deterministic reconstruction and memory-pressure recovery.
   ACC-M1-B002-008 requires every post-execution failure to poison the VM.
4. Implement the independent runner and bounded local IPC, with cancellation,
   crash isolation and no inherited credentials or external access providers.
5. Implement candidate-bound structured test evidence. ACC-M1-B002-009 requires
   rejecting test-selection overrides and proving required tests actually ran.
6. Run targeted/affected tests and minimal fixes, then freeze a signed candidate
   and export exact SHA/tree, changed files, evidence and finding dispositions.
   Full required-suite execution and acceptance belong to a fresh independent
   context. Historical findings remain OPEN_HISTORICAL_HIGH until that acceptance.

B011 package and Creator compatibility remains required. Linux native Core,
cross-builds and other platform-native gates are reported separately. B003/B004,
Creator changes, host governance, main integration and remote writes are excluded.

All new business tests: NOT_RUN at plan creation.
