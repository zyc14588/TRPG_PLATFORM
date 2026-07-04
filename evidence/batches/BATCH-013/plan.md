# BATCH-013 Plan

Batch: `BATCH-013-03-runtime-orchestration`
Stage: `S06`
Date: 2026-07-04

## Boundary Decision

The user-supplied launcher metadata says the recognized primary prompt count is `0`, while `batches/B013.md`, `docs/codex/03-runtime-orchestration/per-file-prompt-manifest.md`, and the normalized maps identify 4 current-safe primary implementation rows in this batch. This run follows `batches/B013.md` plus normalized output paths, matching the precedent recorded in `evidence/batches/BATCH-012/plan.md`, and records the mismatch as a governance risk.

Primary Rust outputs are limited to:

- `crates/trpg-runtime/src/saga.rs`
- `crates/trpg-runtime/src/campaign_session_runtime_service.rs`
- `crates/trpg-runtime/src/readme.rs`
- `crates/trpg-runtime/src/runtime.rs`

Supplemental prompts are traceability-only. They do not own Rust src/test outputs.

## Prompt Work Plan

| Prompt ID | Target | Allowed range | Test responsibility | Status |
|---|---|---|---|---|
| `CODEX-0351-03-RUNTIME-ORCHESTRATION-69b7ab6212` | `runtime_orchestration::pending_decision` | Supplemental traceability only | Covered by existing pending decision tests | Trace file added |
| `CODEX-0352-03-RUNTIME-ORCHESTRATION-4cbd4b1fb8` | `runtime_orchestration::realtime_room_sync` | Supplemental traceability only | Covered by existing realtime visibility tests | Trace file added |
| `CODEX-0353-03-RUNTIME-ORCHESTRATION-b1f275b36f` | `crates/trpg-runtime/src/saga.rs` | Primary Rust implementation | `saga_contract_tests` | Implemented |
| `CODEX-0354-03-RUNTIME-ORCHESTRATION-152ca50c9c` | `runtime_orchestration::scheduler_service` | Supplemental traceability only | Covered by existing scheduler test path | Trace file added |
| `CODEX-0355-03-RUNTIME-ORCHESTRATION-bbee275591` | `crates/trpg-runtime/src/campaign_session_runtime_service.rs` | Primary Rust implementation | `campaign_session_runtime_service_contract_tests` | Implemented |
| `CODEX-0356-03-RUNTIME-ORCHESTRATION-61bce608e0` | `runtime_orchestration::workflow_engine` | Supplemental traceability only | Covered by existing workflow tests | Trace file added |
| `CODEX-0357-03-RUNTIME-ORCHESTRATION-86c7da0e33` | `runtime_orchestration::pending_decision` | Supplemental traceability only | Covered by existing pending decision tests | Trace file added |
| `CODEX-0358-03-RUNTIME-ORCHESTRATION-5626fcbd5c` | `crates/trpg-runtime/src/readme.rs` | Primary Rust implementation | `readme_contract_tests` | Implemented |
| `CODEX-0359-03-RUNTIME-ORCHESTRATION-e2090e6b4e` | `runtime_orchestration::scheduler_service` | Supplemental traceability only | Covered by existing scheduler test path | Trace file added |
| `CODEX-0360-03-RUNTIME-ORCHESTRATION-c50be2f702` | `runtime_orchestration::session_runtime` | Supplemental traceability only | Covered by existing session runtime tests | Trace file added |
| `CODEX-0361-03-RUNTIME-ORCHESTRATION-f2420f7b36` | `runtime_orchestration::capability_layer_tool_grant` | Supplemental traceability only | Covered by existing tool gate tests | Trace file added |
| `CODEX-0362-03-RUNTIME-ORCHESTRATION-84b7588e07` | `runtime_orchestration::workflow_engine` | Supplemental traceability only | Covered by existing workflow tests | Trace file added |
| `CODEX-0363-03-RUNTIME-ORCHESTRATION-2b19458f57` | `crates/trpg-runtime/src/runtime.rs` | Primary Rust implementation | `runtime_contract_tests` | Implemented |
| `CODEX-0364-03-RUNTIME-ORCHESTRATION-2d57ccb6df` | `runtime_orchestration::saga_transaction` | Supplemental traceability only | Covered by existing saga transaction tests | Trace file added |
| `CODEX-0365-03-RUNTIME-ORCHESTRATION-ef38b50d52` | `runtime_orchestration::realtime_runtime_binding` | Supplemental traceability only | Covered by existing realtime binding tests | Trace file added |
| `CODEX-0366-03-RUNTIME-ORCHESTRATION-2d139b43a4` | `runtime_orchestration::capability_layer` | Supplemental traceability only | Covered by existing capability layer tests | Trace file added |
| `CODEX-0367-03-RUNTIME-ORCHESTRATION-4310937ca3` | `runtime_orchestration::pending_decision` | Supplemental traceability only | Covered by existing pending decision tests | Trace file added |
| `CODEX-0368-03-RUNTIME-ORCHESTRATION-ebd7285fa5` | `runtime_orchestration::realtime_room_sync` | Supplemental traceability only | Covered by existing realtime visibility tests | Trace file added |
| `CODEX-0369-03-RUNTIME-ORCHESTRATION-0a78e83a1a` | `runtime_orchestration::saga` | Supplemental to B013 primary | `saga_contract_tests` checks merge marker | Trace file added |
| `CODEX-0370-03-RUNTIME-ORCHESTRATION-4c244748fd` | `runtime_orchestration::scheduler_service` | Supplemental traceability only | Covered by existing scheduler test path | Trace file added |
| `CODEX-0371-03-RUNTIME-ORCHESTRATION-cc05673cc7` | `runtime_orchestration::campaign_session_runtime_service` | Supplemental to B013 primary | `campaign_session_runtime_service_contract_tests` checks merge marker | Trace file added |
| `CODEX-0372-03-RUNTIME-ORCHESTRATION-350a867cc2` | `runtime_orchestration::workflow_engine` | Supplemental traceability only | Covered by existing workflow tests | Trace file added |
| `CODEX-0373-03-RUNTIME-ORCHESTRATION-cf5fc5b856` | `runtime_orchestration::pending_decision` | Supplemental traceability only | Covered by existing pending decision tests | Trace file added |
| `CODEX-0374-03-RUNTIME-ORCHESTRATION-989f2ac19c` | `runtime_orchestration::readme` | Supplemental to B013 primary | `readme_contract_tests` checks merge marker | Trace file added |
| `CODEX-0375-03-RUNTIME-ORCHESTRATION-ba0d8cb1b6` | `runtime_orchestration::realtime_runtime_binding` | Supplemental traceability only | Covered by existing realtime binding tests | Trace file added |

## Test Plan

1. Minimum related checks: the four B013 target tests.
2. Batch/runtime crate check: `cargo test -p trpg-runtime --all-features`.
3. Stage filters: `cargo test -p trpg-runtime decision_pipeline --all-features` and `cargo test -p trpg-runtime tool_gate --all-features`.
4. Format/check gates: `cargo fmt --all -- --check`; `cargo check --workspace --all-features`.
5. Stage/workspace gate: `cargo test --workspace --all-features`.
6. Warning gate: `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
7. Fixture parse check for S06 stage, detailed decision pipeline, AI decision record, and event stream fixtures.

`pnpm` and Docker checks are not applicable for this batch because no `package.json`, `pnpm-lock.yaml`, `Dockerfile`, or compose YAML exists in this workspace.

## Scope Not Taken

- No SQLx migrations, Axum handlers, OpenAPI schemas, NATS subjects, WebSocket server, or provider integrations were added.
- No direct OpenAI/Ollama/llama/provider call path was added.
- No source path, previous-version token, old hash, or historical intermediate filename was used as a Rust module, event, metric, migration, test, or workflow name.
