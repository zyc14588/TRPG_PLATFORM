# P01 Minimal Repair Readiness

## Current anchor

- Recorded at: `2026-07-13T12:43:38.8148076Z`
- P00 base commit: `c86da75dbfa1f6ac0cf6ab7764e08a363fc91396`
- P00 base tree: `030ce4e52a7bb1bde4a81dc7907cdb0215106130`
- Reviewed P01 commit: `9c8c12cd4a04adf4c4953ca2e4ef875781939765`
- Reviewed P01 tree: `d90afce632e69f4011160db979118d0ae9869a3d`
- P00 gate: `P00_COMPLETE=YES`, `P01_ALLOWED=YES`
- P00..P01 changed paths: `331`; path-list hash from the current Windows checkout: `fb8a950ccca2099c571e3b125be7ef5372ad27e7`
- Initial repair worktree: clean; `git diff --check` exit `0`.

## Authoritative inputs read for this repair

- `AGENTS.md`
- `CODEX_STANDALONE_BOOTSTRAP_PROMPT.md`
- `SOURCE_BUNDLE_INTEGRATION_GUIDE.md`
- `docs/top-level-design/CURRENT_TOP_LEVEL_DESIGN.md`
- the three current normalized/safe/token maps
- the attached P01 request, preserved at `evidence/batches/P01/request-provenance.md`
- `CODEX_MASTER_EXECUTION_GUIDE.md`
- `CODEX_START_ACCEPT_TEST_RELEASE_GUIDE.md`
- `CODEX_STRICT_OPERATION_CHECKLIST.md`
- the operator README plus execution, acceptance, testing, evidence, and strict-validation playbooks

## Repair-only scope

The repair closes only findings already established by the P01 retrospective:

1. Remove repository-path and Prompt/batch construction metadata from production Rust contracts; enforce the boundary in repository tooling.
2. Preserve canonical event name/version/schema identity from registry through `EventEnvelope`, constructors, projection, fixtures, and OpenAPI.
3. Add SIGINT/SIGTERM-aware graceful shutdown to the shared service runtime and process smoke; prove every service can construct a failed readiness result from invalid initialization input.
4. Remove the unreferenced P01 Web concept image.
5. Record fresh test and acceptance evidence without rewriting the original process history.

No database, migration behavior, business API, remote service call, P02 table/API/UI/worker, Authority mutation, direct LLM access, or release claim is allowed.

## Expected changed files before implementation

### Runtime and readiness

- `Cargo.lock`
- `apps/service-runtime/Cargo.toml`
- `apps/service-runtime/src/lib.rs`
- `apps/api-server/src/main.rs`
- `apps/realtime-server/src/main.rs`
- `apps/agent-worker/src/main.rs`
- `apps/admin-server/src/main.rs`
- `apps/migration-runner/src/main.rs`
- `scripts/ci/service-process-smoke.sh`
- `scripts/ci/windows_service_launcher.py`

### Canonical event contract

- `crates/trpg-contracts/src/event.rs`
- `crates/trpg-contracts/src/lib.rs`
- `crates/trpg-contracts/tests/registry_contract.rs`
- `crates/trpg-shared-kernel/src/shared_kernel.rs`
- `crates/trpg-data-eventing/src/lib.rs`
- `crates/trpg-data-eventing/tests/canonical_event_registry_contract.rs`
- `crates/trpg-api/src/contract_core.rs`
- `crates/trpg-api/tests/batch_029_api_realtime_contract_tests.rs`

### Construction metadata boundary

- the 13 `crates/trpg-shared-kernel/src/*.rs` files currently constructing `GovernanceContract`
- `crates/trpg-shared-kernel/tests/adr_0001_rust_first_contract_tests.rs`
- `crates/trpg-shared-kernel/tests/document_set_contract_tests.rs`
- `crates/trpg-shared-kernel/tests/readme_contract_tests.rs`
- `crates/trpg-shared-kernel/tests/technology_selection_rust_contract_tests.rs`
- `crates/trpg-shared-kernel/tests/workspace_and_governance_contract_tests.rs`
- `scripts/ci/repo_truth.py`
- `scripts/ci/test_repo_truth.py`

### Evidence and generated manifests

- `apps/web/design/p01-health-shell-concept.png` (delete)
- `evidence/batches/P01/request-provenance.md`
- `evidence/operator/sessions/P01_REPAIR_READINESS.md`
- `evidence/batches/P01/test-results.md`
- `evidence/batches/P01/acceptance-report.md`
- `MANIFEST.md`
- `manifests/CURRENT_PACKAGE_MANIFEST.md`
- `manifests/SELF_CONTAINED_PACKAGE_MANIFEST.md`

If a required edit falls outside this list, stop and record `SCOPE_BLOCKED` before making it.

## Pre-change repair baseline

| Exact command | Exit | Key output |
|---|---:|---|
| `cargo test -p trpg-contracts --all-targets` | 0 | 4 contract tests passed |
| `cargo test -p trpg-service-runtime --all-targets` | 0 | 5 runtime tests passed |
| `python scripts/ci/test_repo_truth.py` | 0 | 18 tests passed |
| Git Bash `./scripts/ci/service-process-smoke.sh` | 0 | five Rust services, negative config, and Web smoke passed |

An accidental invocation of Windows `bash.exe` failed before running the repository script because WSL is not installed. A second Git Bash invocation without login environment also failed before running because `dirname` was unavailable and the working directory changed. Both setup failures are retained in the Codex session and are not reported as product test failures.

## Historical evidence boundary

Fresh repair evidence cannot retroactively prove the original P01 execution order. These original-process facts remain `UNVERIFIED`: first-edit document timing/order, original pre-change status and baseline logs, original expected-file list, per-slice test timing, original exact command exits/durations, LNK/hang raw logs and retry lineage, original independent acceptance separation, transient-log secret scan, and the original unstaged diff interpretation.

The eventual acceptance report must state separate conclusions for current repaired code and original P01 process evidence.
