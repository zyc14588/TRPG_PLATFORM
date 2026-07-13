# P01 Minimal Repair Test Results

## Result boundary

- Baseline commit reviewed: `9c8c12cd4a04adf4c4953ca2e4ef875781939765`.
- This file records the current uncommitted repair worktree. It is not CI evidence and does not retroactively prove the original P01 execution order.
- All product assertions below come from commands actually executed in this Codex session. Environment/invocation failures are retained instead of being rewritten as product passes.
- No database, migration behavior, business endpoint, remote provider call, P02 feature, or release action was exercised or added.

## Pre-repair baseline

| Command | Exit | Key result |
|---|---:|---|
| `cargo test -p trpg-contracts --all-targets` | 0 | 4 tests passed |
| `cargo test -p trpg-service-runtime --all-targets` | 0 | 5 tests passed |
| `python scripts/ci/test_repo_truth.py` | 0 | 18 tests passed |
| Git Bash `./scripts/ci/service-process-smoke.sh` | 0 | five Rust services, invalid-config negative case, and Web passed |

Two earlier baseline smoke invocations failed before the repository script ran: Windows `bash.exe` selected an unavailable WSL runtime, and a non-login Git Bash invocation lacked `dirname`. They are invocation failures, not product test results.

## Required P01 commands after repair

| Command | Exit | Key result |
|---|---:|---|
| `cargo build --workspace --all-targets --release --locked` | 0 | Workspace release build completed; all five service binaries compiled. |
| `cargo test -p trpg-contracts --all-targets` | 0 | 4/4 registry tests passed, including fixture/event name, version, schema identity, and error-code closure. |
| `pnpm --filter ./apps/web... build` | 1 | PowerShell blocked `pnpm.ps1` before the project command ran. |
| `pnpm.cmd --filter ./apps/web... build` | 0 | Equivalent installed Windows entry point; Web v0.1.0 built and 5 files were verified. |
| Git Bash `pnpm --filter ./apps/web... build` | 0 | The exact requested command ran under Git Bash; Web v0.1.0 built and 5 files were verified. |
| Git Bash `./scripts/ci/service-process-smoke.sh` | 0 | API, realtime, agent worker, admin, migration runner, unsafe-production admin, invalid max-request configuration, and Web all passed. |

The final smoke used `CHERE_INVOKING=1` with `C:\Program Files\Git\bin\bash.exe -lc` so the requested repository script ran under Git Bash from the repository root. Two preceding post-repair invocations did not enter the script: one resolved the shebang to unavailable WSL, and one lacked the Git Bash utility path. Both exited 1 and are retained as setup failures.

## Focused repair verification

| Command | Exit | Key result |
|---|---:|---|
| `cargo fmt --all -- --check` | 0 | Rust formatting clean after one mechanical `cargo fmt --all`. |
| `git diff --check` | 0 | No whitespace errors; only Windows LF/CRLF advisory warnings. |
| Git Bash `bash -n scripts/ci/service-process-smoke.sh` | 0 | Shell syntax valid. |
| `python scripts/ci/test_repo_truth.py` | 0 | 19 tests passed, including the new production construction-metadata negative case. |
| `cargo test -p trpg-shared-kernel --all-targets` | 0 | Shared-kernel contract suite passed. |
| `cargo test -p trpg-data-eventing --test canonical_event_registry_contract` | 0 | 3/3 canonical envelope/projection identity tests passed. |
| `cargo test -p trpg-api --test batch_029_api_realtime_contract_tests` | 0 | 9/9 API/OpenAPI contract tests passed. |
| `cargo test -p trpg-service-runtime --all-targets` | 0 | 5/5 shared HTTP runtime tests passed. |
| Five binary test targets | 0 | Each binary's invalid-initialization readiness test passed. |
| `python scripts/ci/validate_workflows.py` | 0 | Workflow static validation passed. |
| `python scripts/ci/verify_test_inventory.py` | 0 | Test inventory resolved; `orphan_fixtures` was empty. |
| `python scripts/ci/verify_manifest.py` existing-index verification | 0 | Existing Git-index manifest verified 3,769 hashed files; this result does not bind the uncommitted repair worktree. |

## Full regression and Windows retry lineage

`cargo test --workspace --lib --locked` first exited 1 because Windows `link.exe` returned `LNK1104` for several test executables. No compile diagnostic or test assertion failed. The same command with `--jobs 1` exited 0 and ran every workspace library target.

The stronger command `cargo test --workspace --all-targets --locked --jobs 1` eventually exited 0 and ran the complete workspace test inventory. Its retry lineage is retained:

1. Initial serial attempts advanced one or more newly linked test executables and then hit `LNK1104` on a different new EXE.
2. `trpg-runtime --all-targets` completed on guarded attempt 11; attempts 1-10 contained only `LNK1104`.
3. The first guarded workspace sequence had 38 `LNK1104` attempts, then stopped on Windows application-control error 4551 before `authority_contract_contract_tests` executed.
4. The blocked target was run directly and passed 3/3 with exit 0.
5. The next complete workspace all-targets invocation exited 0. The retry guard accepted only `LNK1104` and the already reproduced one-time error 4551; any compile, assertion, or other error would have stopped it.

The final all-targets pass includes the event registry tests, projection/replay tests, governance boundary tests, Authority Contract tests, visibility tests, agent/provider gates, and all other discovered Rust targets. No test was deleted, filtered, ignored, or weakened.

## Independent-acceptance repair rounds

Independent read-only acceptance found two code gaps after the first repair round:

1. API projection accepted a canonical event-name string without validating the canonical event descriptor.
2. AUD-018 construction metadata remained in product-facing document models, and governance contracts were still hand-assembled at call sites.

The minimal follow-up repair made API projection fail closed on absent, mismatched, or unexpected descriptors; centralized all 13 governance-contract call sites through `GovernanceContract::new`; removed the residual construction document models; and extended repository-truth checks to reject qualified as well as unqualified direct contract literals.

| Post-round command | Exit | Key result |
|---|---:|---|
| `cargo check --workspace --all-targets --all-features --locked` | 0 | Entire workspace checked without warnings. |
| `cargo test -p trpg-shared-kernel --all-targets --locked --jobs 1` | 0 | All shared-kernel targets passed after the construction-boundary repair. |
| `python scripts/ci/test_repo_truth.py` | 0 | 19/19 repository-truth tests passed. |
| `cargo test -p trpg-api --all-targets --locked --jobs 1` | 0 | All API targets passed after descriptor fail-closed enforcement. |
| `cargo test --workspace --all-targets --locked --jobs 1` | 0 | Final full workspace pass completed in approximately 420 seconds. MSVC inspection activity made the process appear idle, but the command completed normally with exit 0. |

The final smoke sequence had one transient `curl: (56) Recv failure: Connection was aborted` while probing admin after earlier services had passed. No process remained. An unchanged exact retry exited 0 and passed all service, unsafe-provider, invalid-config, and Web checks.

The second repair required changing `crates/trpg-shared-kernel/tests/document_set_impl_contract_tests.rs`, which was not in the pre-readiness expected-file list. The narrow scope deviation and justification are recorded in `evidence/operator/sessions/P01_SCOPE_DEVIATION.md`.

## Process behavior proved by smoke

- Every Rust service returned `200` for `/health/live` and readiness JSON with its exact expected check names.
- Normal instances returned ready checks with status `pass`.
- An admin process started with unsafe production provider settings stayed live but returned `503`/`not_ready` with a failed `admin_provider_boundary` check.
- On Windows, each service ran in a distinct process group, received `CTRL_BREAK`, exited 0, and emitted its shutdown-complete log.
- The shared runtime also compiles Unix SIGINT/SIGTERM listeners, but this Windows session did not execute a Linux process-level signal test.
- `TRPG_MAX_REQUESTS=0` was rejected before listener startup.
- Routes outside the two health endpoints returned 404; non-GET health requests returned 405.

## Release boundary

`python scripts/ci/release_readiness.py --require-blocked` exited 0 and confirmed release remains `BLOCKED`. The report includes the pre-existing, out-of-P01 `AUD-002` and `AUD-006` container/Compose blockers, missing product Dockerfile/current immutable evidence, placeholder services, and the intentionally dirty repair worktree. This repair makes no release-readiness claim.

## Manifest synchronization repair

After explicit user authorization to repair the exposed gaps, a one-time temporary Git index was populated with the complete worktree and passed to the repository-native generator. `python scripts/ci/verify_manifest.py` under that temporary index exited 0 with 3,773 hashed files. The removed design image is absent from all three byte-identical outputs. The real `.git/index` SHA-256 was unchanged and `git diff --cached --name-only` remained empty.

## 2026-07-14 follow-up revalidation

The exact locked release build, four contract tests, exact Git Bash pnpm build, service-process smoke, 19 repository-truth tests, workflow validation, and test-inventory validation all exited 0 against the unchanged repaired source. `cargo test --workspace --all-targets --locked --jobs 1` completed without filtering or retries and exited 0 in 333.5 seconds. `git diff --check` remained clean apart from LF/CRLF advisory warnings.

## Tooling limitation

The CodeRabbit CLI is not yet runnable in this environment. Its official installer rejects native Windows, and the Microsoft-supported WSL path now has WSL 2.7.8 plus an Ubuntu VHDX installed but awaiting Windows restart to finish distribution registration. An automated CodeRabbit review was therefore not run and is not impersonated by this report.
