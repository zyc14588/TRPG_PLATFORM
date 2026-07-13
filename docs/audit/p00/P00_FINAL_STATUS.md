# P00 Final Status

> This is the final P00C repository-governance attestation. It does not claim product release readiness: `release-readiness-evidence` correctly remains `BLOCKED` on `AUD-001`, `AUD-002`, and `AUD-006`. No P01 work was executed while producing this record.

## Final decision

```text
BATCH_ID = P00C
BATCH_STATUS = COMPLETE

P00A_COMMIT = c621f306db42c3d59910b7445990446a3741a47f
P00B_FIX_COMMIT = 059ae977a05f0e0e9190dce65e65322342150954
P00_FINAL_COMMIT = 4e157d357ac7d76665178df9c86e2478037f60a6
P00_FINAL_TREE = 62057b7e2a25888049496840ba66c97c9620960f

CANONICAL_REPOSITORY = zyc14588/TRPG_PLATFORM
CANONICAL_BRANCH = master
PUSHED_REMOTE_REF = refs/heads/master@4e157d357ac7d76665178df9c86e2478037f60a6

P00A_STATUS = COMPLETE
P00B_STATUS = COMPLETE
P00C_STATUS = COMPLETE
P00_COMPLETE = YES
P01_ALLOWED = YES
```

The owner decision is `APPROVED` by `zyc14588` at `2026-07-13T03:48:14Z`. The unrelated `main` head `a6d8f8c83c1863c84b654560db35cd16688cbf9a` is preserved at `legacy-main-20260713`; it was not merged, deleted, force-pushed, or mechanically combined with `master`.

## Positive GitHub-hosted evidence

All runs below are attempt `1`, completed successfully on GitHub-hosted `ubuntu-24.04` runners, and bind their manifests to repository `zyc14588/TRPG_PLATFORM` and `P00_FINAL_COMMIT`.

| Workflow / job | Run / job ID | Artifact ID | GitHub artifact ZIP SHA-256 |
|---|---|---:|---|
| `workspace-ci / workspace` | `29225518722 / 86738669788` | `8269598872` | `6c8d04a922aa40e642bac7d561b36af78c490ce011ae27eadfe1feb1f23bb2b7` |
| `repository-truth / truth` | `29225518706 / 86738670047` | `8269591743` | `21e60e490c3e09f4dd0b8ce8ce44a875d4fdf1bcfb290d1b09715ca0ee4b9e8c` |
| `golden-scenarios / golden` | `29225518723 / 86738669863` | `8269589701` | `7b5a2deeb7e8681a0bc386d059f92f253b21b24d6e9774a819430eb0f1960d78` |
| `release-readiness-evidence / blocked-evidence` | `29225560758 / 86738793875` | `8269611832` | `6a2e969e3d792e4d9eeb0bf8f066c1573b9bdc28a135b5e5b15379f7f166df22` |
| `docker-compose-config / config` | `29225561756 / 86738796714` | `8269602181` | `4ed68b9070a6c27603a3bfe7455dd42efb722ba43b10d24bf36b6a09f1db4f01` |

```text
WORKFLOW_RUN_IDS = 29225518722,29225518706,29225518723,29225560758,29225561756
POSITIVE_ARTIFACT_IDS = 8269598872,8269591743,8269589701,8269611832,8269602181
RELEASE_READINESS_STATUS = BLOCKED
RELEASE_READINESS_BLOCKERS = AUD-001,AUD-002,AUD-006
```

## Controlled remote negative matrix

Run `29225562580` completed successfully because every isolated fault made its target validator return the expected non-zero exit code `1`. Each case uploaded its diff, baseline output, validation output, exit code, internal checksum list, and evidence manifest.

| Case | Job ID | Artifact ID | GitHub artifact ZIP SHA-256 |
|---|---:|---:|---|
| `CASE-01_broken_test_assertion` | `86738798525` | `8269604140` | `43052c3a21a3fdbf7ff20fbdfc6fcb5579a21f4cdd570476fb67f4e765a27d03` |
| `CASE-02_missing_referenced_script` | `86738798529` | `8269603032` | `d926ce88048d8419be96fd28cbca5941d9a3c05e625efc6c78f40f1defe81723` |
| `CASE-03_missing_required_evidence` | `86738798555` | `8269602810` | `6884b7f19f52bacd1363a84c116b1c490f0871a6aecc016a3110cdabd57adaff` |
| `CASE-04_manifest_hash_tamper` | `86738798530` | `8269603333` | `28688a112340678820d0aed6cb3c27c29c6ef6c127b30f00720c4f84f60606cc` |
| `CASE-05_readiness_provenance_removed` | `86738798560` | `8269602940` | `048fa3ae7c6098d0cff540b13071c000d4b21c9fe96a5b36f6d9ea737a3c8587` |

```text
NEGATIVE_MATRIX_RUN_ID = 29225562580
NEGATIVE_ARTIFACT_IDS = 8269604140,8269603032,8269602810,8269603333,8269602940
NEGATIVE_CASE_EXIT_CODES = 1,1,1,1,1
```

## Immutable artifact verification

Verified at `2026-07-13T05:18:38Z` by downloading all ten GitHub artifacts from the Actions API:

- 10/10 downloaded ZIP SHA-256 values equal the GitHub service-side artifact digests above; all artifacts reported `expired=false`.
- 10/10 evidence manifests bind the canonical repository, final commit, run ID, run attempt, workflow, job, Linux runner, command exit status, and semantic `PASS` status.
- 60/60 declared report files match their recorded byte sizes and SHA-256 values, including every generated-artifact checksum.
- The release report is genuinely `BLOCKED`, has provenance `4e157d357ac7d76665178df9c86e2478037f60a6`, and retains blockers `AUD-001`, `AUD-002`, and `AUD-006`.
- All five negative artifacts contain the required fault and validation records; their internal checksum lists also recompute exactly.

## Branch protection evidence

Final API read-back captured at `2026-07-13T05:17:07.0628840Z` from `https://api.github.com/repos/zyc14588/TRPG_PLATFORM/branches/master/protection`:

```text
DEFAULT_BRANCH = master
BRANCH_PROTECTED = true
REQUIRE_PULL_REQUEST = true
REQUIRED_APPROVING_REVIEW_COUNT = 0
REQUIRE_BRANCH_UP_TO_DATE = true
REQUIRED_CHECKS = workspace-ci / workspace | repository-truth / truth | golden-scenarios / golden
REQUIRE_CONVERSATION_RESOLUTION = true
ENFORCE_ADMINS = true
ALLOW_FORCE_PUSHES = false
ALLOW_DELETIONS = false
```

PR `#2` repaired the raw GitHub CheckRun names and was merged normally only after all three exact contexts reported `isRequired=true`, completed successfully, and GitHub reported `mergeStateStatus=CLEAN`. Its protected merge commit is `P00_FINAL_COMMIT`; no administrator bypass was used.

## Sentinel result

Final Sentinel PR: `https://github.com/zyc14588/TRPG_PLATFORM/pull/3`

```text
SENTINEL_PR_NUMBER = 3
SENTINEL_BASE = master@4e157d357ac7d76665178df9c86e2478037f60a6
SENTINEL_HEAD = codex/p00c-sentinel-final-4e157d3@8a1f79c460859de601381e646aeada1ed2651bf9
SENTINEL_CHANGED_FILES = 0
SENTINEL_FINAL_STATE = CLOSED
SENTINEL_MERGED = false
ADMIN_BYPASS_USED = false
```

The Sentinel used a tree-identical empty commit. While the three exact required checks were running, all were `isRequired=true` and GitHub reported `mergeStateStatus=BLOCKED`. After runs `29225628379`, `29225628403`, and `29225628374` completed successfully, GitHub reported `mergeStateStatus=CLEAN`. The PR was then closed unmerged. Diagnostic Sentinel PR `#1` was also closed unmerged and retained as migration provenance.

## AUD status

| Audit item | Final status | P00C basis |
|---|---|---|
| `AUD-003` | `CLOSED_PASS` | Workflows activated, references complete, controlled remote failures propagated, and exact required checks enforced. |
| `AUD-004` | `CLOSED_PASS` | The CI implementation and immutable final commit are remotely bound. |
| `AUD-045` | `PARTIAL_DEFERRED_TO_P13_P14` | Fixed-response/self-proof is rejected and readiness remains truthfully blocked; real product E2E remains later scope. |
| `AUD-050` | `CLOSED_PASS` | Repository truth checks are active on the immutable commit. |
| `AUD-052` | `CLOSED_PASS` | Toolchain, commit, command, exit code, file hash, and GitHub artifact digest bind to one final commit. |
| `AUD-053` | `CLOSED_PASS` | Evidence schema and machine-verifiable outputs are active remotely. |
| `AUD-085` | `CLOSED_PASS` | Default branch, workflow triggers, branch protection, and Sentinel all use canonical `master`. |
| `AUD-086` | `CLOSED_PASS` | Canonical repository/branch ownership and protected governance are verified. |
| `AUD-090` | `CLOSED_PASS` | Controlled failure propagation is verified for all five required cases. |
| `AUD-092` | `CLOSED_PASS` | Final commit is pushed, the checkout is clean, and all evidence binds that commit. |

## Residual boundary and rollback

```text
UNVERIFIED_ITEMS_WITHIN_P00C = NONE
PRODUCT_RELEASE_READY = NO
PRODUCT_RELEASE_BLOCKERS = AUD-001,AUD-002,AUD-006
P01_EXECUTED = NO
```

- Repository rollback: create an ordinary revert of the relevant P00 commit; never rewrite published history.
- Protection rollback: first capture the current protection API response, then restore the previous known-good rule through the API.
- Sentinel branches/PRs are retained as explicit closed, unmerged audit archives. They are not canonical product history.
