# P01 Minimal Repair Acceptance Report

BATCH_STATUS: PARTIAL_NOT_ACCEPTED
BATCH_ID: P01

## Decision

- `P01_CURRENT_CODE_REVALIDATION=PASS`
- `P01_ORIGINAL_PROCESS_EVIDENCE=UNVERIFIED`
- `P01_REPAIR_PROCESS_CONFORMANCE=PARTIAL`
- `P01_MANIFEST_SYNC=PASS`
- `P02_EXECUTED=NO`
- `RELEASE_CLAIM=NO`

The current repaired code satisfies the six P01 AUD findings and the product-level P01 acceptance checks exercised in this session. The batch is not reported as `COMPLETE`: fresh evidence cannot retroactively prove the original execution order, and one repair file exceeded the pre-recorded expected-file list.

## AUD acceptance matrix

| Finding | Current code | Evidence |
|---|---|---|
| AUD-001 | PASS | Five real Rust service binaries build and start; Web builds and previews; process smoke verifies service-owned liveness/readiness, invalid initialization, unsafe admin provider readiness, and graceful Windows process-group shutdown. |
| AUD-018 | PASS | Production governance contracts use the single `GovernanceContract::new` construction source at all 13 call sites; construction-document and Prompt/batch/path metadata were removed from product models; repository truth rejects qualified and unqualified direct contract literals. |
| AUD-026 | PASS | No fixed-fixture command factory is exposed by the production public API; fixture creation remains test-scoped and contract tests pass. |
| AUD-039 | PASS | Repository-truth and compatibility tests find no prohibited identical production modules; the public compatibility boundary remains explicitly tested. |
| AUD-040 | PASS | The wire error-code registry is exhaustive and uses the normalized naming contract; all registry tests pass. |
| AUD-044 | PASS | Canonical event name, version, and schema identity share one registry and propagate through typed construction, envelopes, fixtures, projection, and OpenAPI; API and data projection fail closed on absent or mismatched descriptors. |

Independent read-only acceptance initially found the residual AUD-018 construction boundary and the API-side AUD-044 descriptor gap. Both were repaired minimally. Its final current-code review found all six findings passing; the stricter constructor and repository-truth guards were then added and rerun successfully.

## Product acceptance matrix

| P01 criterion | Result | Evidence |
|---|---|---|
| Release artifacts for API, realtime, agent worker, admin, migration runner, and Web | PASS | Locked all-target release build and exact Web build exited 0. |
| Start, own health state, and graceful exit | PASS on exercised platform | Real-process smoke passed on Windows, including failed readiness from unsafe initialization and successful `CTRL_BREAK` shutdown. Unix SIGINT/SIGTERM paths compile but were not process-tested in this Windows session. |
| Web is a real local product shell, not a Compose/Nginx substitute | PASS | Package build and local preview smoke passed; five built files were verified. |
| Dependency direction, event registry, and wire errors are enforceable | PASS | Rust contract suites and 19 repository-truth tests passed. |
| No production fixed-fixture factory or completely duplicated implementation | PASS | Shared-kernel, contract, and repository-truth suites passed. |
| No P02/business/database/provider expansion | PASS | Diff review found health endpoints and P01 contract/runtime work only; no database or migration behavior, business API, remote provider call, or P02 implementation was added. |

## Required and regression commands

| Command | Exit | Result |
|---|---:|---|
| `cargo build --workspace --all-targets --release --locked` | 0 | Release build passed. |
| `cargo test -p trpg-contracts --all-targets` | 0 | 4/4 tests passed. |
| Git Bash `pnpm --filter ./apps/web... build` | 0 | Exact required command passed; five output files verified. |
| Git Bash `./scripts/ci/service-process-smoke.sh` | 0 | Unchanged retry passed every positive and negative case after one transient admin curl abort. |
| `cargo check --workspace --all-targets --all-features --locked` | 0 | Workspace checked without warnings. |
| `cargo test --workspace --all-targets --locked --jobs 1` | 0 | Complete workspace target inventory passed. |
| `cargo test -p trpg-shared-kernel --all-targets --locked --jobs 1` | 0 | Shared-kernel targets passed. |
| `cargo test -p trpg-api --all-targets --locked --jobs 1` | 0 | API targets passed. |
| `python scripts/ci/test_repo_truth.py` | 0 | 19/19 tests passed. |
| `python scripts/ci/validate_workflows.py` | 0 | Workflow validation passed. |
| `python scripts/ci/verify_test_inventory.py` | 0 | Inventory passed with no orphan fixtures. |
| `python scripts/ci/release_readiness.py --require-blocked` | 0 | Correctly confirmed global release remains blocked. |

PowerShell's `pnpm.ps1` execution-policy failure and earlier shell-selection failures are retained as invocation failures. They were not relabeled as product passes. The exact pnpm command subsequently ran successfully under Git Bash. Full Windows retry lineage, including transient linker locks and the smoke retry, is recorded in `test-results.md`.

## Process and evidence gaps

1. Fresh repair evidence does not prove the original P01 first-edit timing, original pre-change status/baseline, per-slice execution order, original raw command logs, or original independent-acceptance separation. Those facts remain `UNVERIFIED`.
2. `crates/trpg-shared-kernel/tests/document_set_impl_contract_tests.rs` was changed after independent acceptance exposed a stricter AUD-018 gap, although it was absent from the repair's expected-file list. The change is in P01 scope and tested, but the required pre-edit `SCOPE_BLOCKED` discipline was not followed.
3. Evidence is local and uncommitted, not immutable CI or commit evidence. The worktree is intentionally dirty; nothing was staged or committed.
4. The CodeRabbit CLI remains unavailable. Its official shell installer rejects native Windows (`Unsupported operating system: mingw64_nt-10.0-26200`). After explicit authorization, WSL 2.7.8 and the Ubuntu VHDX were installed through Microsoft tooling, but the distribution registration is pending a Windows restart; no CodeRabbit result is claimed before installation, authentication, and review actually succeed.
5. Unix shutdown support is compile-verified only; the process-level shutdown test ran on Windows.

## Follow-up manifest repair

After explicit user authorization to repair the exposed gaps, the repository-native generator rebuilt all three manifest outputs from a one-time temporary Git index representing the complete worktree. Verification passed for 3,773 hashed files, the removed design image is absent, all three outputs are byte-identical, the real `.git/index` SHA-256 remained `4F9E07AA9203A3F649ECCC2EFEAA07919631821968A206EDAD72149A2E0979F5`, and the real staged-path count remained zero.

## Scope, risk, and rollback

The repair changes only P01 runtime/readiness, canonical event/error and construction boundaries, their tests/tooling, and evidence. It adds no migration, persistent format, business endpoint, remote call, Authority mutation, direct LLM path, or release operation.

Rollback can remove the scoped uncommitted repair by file group. The event-envelope descriptor is optional for forward compatibility, the projection hash input is unchanged, and no database rollback is involved. The only runtime dependency change is the locked Tokio feature/version needed for signal handling.

Global release readiness remains separately blocked by pre-existing out-of-P01 AUD-002/AUD-006/container and immutable-evidence gaps. This report does not start P02 and makes no release claim.
