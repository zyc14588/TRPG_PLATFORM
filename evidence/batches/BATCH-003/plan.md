# BATCH-003 Work Plan

Batch: `BATCH-003-01-foundation`
Stage: `S01` (`stages/s01-foundation-shared-kernel`)
Current batch file: `batches/B003.md`

## Scope

- Declared prompt count: 25.
- Batch table primary implementation rows: 9.
- Batch table supplemental requirement rows: 15.
- Batch table documentation/traceability rows: 1.
- Operator input said the recognized primary prompt count was 0. The current
  authoritative `batches/B003.md` table lists primary implementation rows, so
  this plan follows the batch table plus normalized current-safe output map.
- Allowed implementation changes: create the minimal Rust workspace skeleton
  and `trpg-shared-kernel` crate required by B003 primary prompts, then add the
  current-safe flat modules and contract tests listed below.
- Allowed documentation/evidence changes: BATCH-003 evidence files and the
  B003 documentation target.
- Disallowed changes: source-archive promotion, historical V3/V4/V5/V6 names
  as current code names, migrations, API handlers, NATS subjects, provider
  adapters, workflows outside S01, Authority Contract mutation behavior,
  direct LLM access, direct agent database writes, or any later-batch work.

## Prompt Mapping

| Prompt ID | Prompt file | Target file(s) | Allowed change scope | Test responsibility |
|---|---|---|---|---|
| CODEX-0015-01-FOUNDATION-d0600c0f55 | `codex-prompts/01-foundation/P0006.md` | `docs/codex/01-foundation/m_01_foundation.md` | Documentation/traceability only | Evidence link and current-safe docs boundary |
| CODEX-0016-01-FOUNDATION-47cddc714f | `codex-prompts/01-foundation/P0001.md` | `crates/trpg-shared-kernel/src/cargo_workspace.rs`; `crates/trpg-shared-kernel/tests/cargo_workspace_contract_tests.rs` | Primary implementation for `shared_kernel::cargo_workspace` | Command envelope, idempotency, expected version, Event Store, visibility/provenance contract tests |
| CODEX-0017-01-FOUNDATION-f884586b0c | `codex-prompts/01-foundation/P0002.md` | `crates/trpg-shared-kernel/src/config_model.rs`; `crates/trpg-shared-kernel/tests/config_model_contract_tests.rs` | Primary implementation for `shared_kernel::config_model` | Provider boundary, no direct LLM access, required config validation |
| CODEX-0018-01-FOUNDATION-432c82edce | `codex-prompts/01-foundation/P0003.md` | `crates/trpg-shared-kernel/src/crate_ownership.rs`; `crates/trpg-shared-kernel/tests/crate_ownership_contract_tests.rs` | Primary implementation for `shared_kernel::crate_ownership` | Crate ownership and write-authority boundary tests |
| CODEX-0019-01-FOUNDATION-9e72d06346 | `codex-prompts/01-foundation/P0004.md` | `crates/trpg-shared-kernel/src/dependency_direction.rs`; `crates/trpg-shared-kernel/tests/dependency_direction_contract_tests.rs` | Primary implementation for `shared_kernel::dependency_direction` | Layer direction and forbidden dependency tests |
| CODEX-0020-01-FOUNDATION-ebc932d045 | `codex-prompts/01-foundation/P0005.md` | `crates/trpg-shared-kernel/src/error_model.rs`; `crates/trpg-shared-kernel/tests/error_model_contract_tests.rs` | Primary implementation for `shared_kernel::error_model` | Stable error code and retryability tests |
| CODEX-0021-01-FOUNDATION-0e50b519e8 | `codex-prompts/01-foundation/P0007.md` | `crates/trpg-shared-kernel/src/rust_coding_model.rs`; `crates/trpg-shared-kernel/tests/rust_coding_model_contract_tests.rs` | Primary implementation for `shared_kernel::rust_coding_model` | Template-name, source-hash, serde boundary, unsafe pattern tests |
| CODEX-0022-01-FOUNDATION-78879e4006 | `codex-prompts/01-foundation/P0008.md` | `crates/trpg-shared-kernel/src/shared_kernel.rs`; `crates/trpg-shared-kernel/tests/shared_kernel_contract_tests.rs` | Primary implementation for `shared_kernel::shared_kernel` | Typed IDs, visibility lattice, provenance, authority, replay and fixture contract tests |
| CODEX-0147-01-FOUNDATION-8f2053b8f3 | `codex-prompts/01-foundation/P0009.md` | Merged into `CODEX-0016-01-FOUNDATION-47cddc714f` | Supplemental requirement only | Covered by cargo workspace tests |
| CODEX-0148-01-FOUNDATION-288a79953f | `codex-prompts/01-foundation/P0010.md` | Merged into `CODEX-0017-01-FOUNDATION-f884586b0c` | Supplemental requirement only | Covered by config model tests |
| CODEX-0149-01-FOUNDATION-b9637d2faa | `codex-prompts/01-foundation/P0011.md` | Merged into `CODEX-0018-01-FOUNDATION-432c82edce` | Supplemental requirement only | Covered by crate ownership tests |
| CODEX-0150-01-FOUNDATION-7765bce861 | `codex-prompts/01-foundation/P0012.md` | Merged into `CODEX-0019-01-FOUNDATION-9e72d06346` | Supplemental requirement only | Covered by dependency direction tests |
| CODEX-0151-01-FOUNDATION-17dfd623b1 | `codex-prompts/01-foundation/P0013.md` | Merged into `CODEX-0020-01-FOUNDATION-ebc932d045` | Supplemental requirement only | Covered by error model tests |
| CODEX-0152-01-FOUNDATION-4354750404 | `codex-prompts/01-foundation/P0014.md` | `crates/trpg-shared-kernel/src/rust_cargo_workspace.rs`; `crates/trpg-shared-kernel/tests/rust_cargo_workspace_contract_tests.rs` | Primary implementation for `shared_kernel::rust_cargo_workspace` | Workspace manifest and package naming tests |
| CODEX-0153-01-FOUNDATION-0493c6b4df | `codex-prompts/01-foundation/P0015.md` | Future primary `shared_kernel::constitution` | Supplemental requirement only | Trace only in this batch |
| CODEX-0154-01-FOUNDATION-c74e5b5ee7 | `codex-prompts/01-foundation/P0016.md` | Future primary `shared_kernel::document_set` | Supplemental requirement only | Trace only in this batch |
| CODEX-0155-01-FOUNDATION-aab0ed92a1 | `codex-prompts/01-foundation/P0017.md` | Merged into `CODEX-0161-01-FOUNDATION-bfdfdd079f` | Supplemental requirement only | Covered by open source reference matrix tests |
| CODEX-0156-01-FOUNDATION-d3cf6be88e | `codex-prompts/01-foundation/P0018.md` | Future primary `shared_kernel::system_context` | Supplemental requirement only | Trace only in this batch |
| CODEX-0157-01-FOUNDATION-a5821814eb | `codex-prompts/01-foundation/P0019.md` | Future primary `shared_kernel::technology_selection_rust` | Supplemental requirement only | Trace only in this batch |
| CODEX-0158-01-FOUNDATION-cb6d378974 | `codex-prompts/01-foundation/P0021.md` | Merged into `CODEX-0016-01-FOUNDATION-47cddc714f` | Supplemental requirement only | Covered by cargo workspace tests |
| CODEX-0159-01-FOUNDATION-a32db93100 | `codex-prompts/01-foundation/P0020.md` | Merged into `CODEX-0020-01-FOUNDATION-ebc932d045` | Supplemental requirement only | Covered by error model tests |
| CODEX-0160-01-FOUNDATION-e0de0ada72 | `codex-prompts/01-foundation/P0022.md` | Merged into `CODEX-0022-01-FOUNDATION-78879e4006` | Supplemental requirement only | Covered by shared kernel tests |
| CODEX-0161-01-FOUNDATION-bfdfdd079f | `codex-prompts/01-foundation/P0028.md` | `crates/trpg-shared-kernel/src/open_source_reference_matrix.rs`; `crates/trpg-shared-kernel/tests/open_source_reference_matrix_contract_tests.rs` | Primary implementation for `shared_kernel::open_source_reference_matrix` | Approved-source, license policy, and provider-boundary tests |
| CODEX-0162-01-FOUNDATION-860023295f | `codex-prompts/01-foundation/P0025.md` | Future primary `shared_kernel::technology_selection_rust` | Supplemental requirement only | Trace only in this batch |
| CODEX-0163-01-FOUNDATION-a8dd6915b3 | `codex-prompts/01-foundation/P0026.md` | Merged into `CODEX-0016-01-FOUNDATION-47cddc714f` | Supplemental requirement only | Covered by cargo workspace tests |

## Check Order

1. Minimal B003 current-safe/path assertions.
2. Rust formatting and shared-kernel unit/contract tests.
3. S01 stage checks from `stages/s01-foundation-shared-kernel/TEST_PLAN.md`.
4. S01 acceptance fixture checks, including the detailed fixture contract.
5. Acceptance evidence write-up under `evidence/batches/BATCH-003/`.

## Expected Strict Result

`PASS` when the created `trpg-shared-kernel` crate passes formatting,
clippy, unit tests, contract tests, fixture checks, and B003 evidence checks
without weakening Authority Contract, visibility, provenance, event-log, or
provider-boundary constraints.
