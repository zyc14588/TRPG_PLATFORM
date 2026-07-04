# BATCH-012 Plan

Batch: `BATCH-012-03-runtime-orchestration`
Stage: `S06`
Date: 2026-07-04

## Boundary Decision

`batch-prompts/start/B012.md` says the recognized primary prompt count is `0`, while `batches/B012.md`, `docs/codex/03-runtime-orchestration/per-file-prompt-manifest.md`, and the normalized maps identify 14 current-safe primary implementation rows in this batch. The batch execution used the row-level current-safe role from `batches/B012.md` and recorded the mismatch as a governance risk.

The pre-existing workspace did not contain `crates/trpg-runtime`; primary prompt text explicitly allows creating a minimal `trpg-runtime` crate when absent. This batch therefore creates the minimal crate and avoids S07/S08/S03 integration work.

## Prompt Work Plan

| Prompt ID | Target | Allowed range | Test responsibility | Status |
|---|---|---|---|---|
| `CODEX-0031-03-RUNTIME-ORCHESTRATION-cac012cf70` | `docs/codex/03-runtime-orchestration/m_03_runtime_orchestration.md` | Docs/traceability only | Non-code row; covered by docs-governance output and traceability evidence | Implemented |
| `CODEX-0032-03-RUNTIME-ORCHESTRATION-20830a72ac` | `crates/trpg-runtime/src/capability_tool_grant.rs` | Primary Rust implementation | `tool_gate_fixture_error_cases_are_asserted`; `non_orchestrator_agent_cannot_request_formal_state_tool` | Implemented |
| `CODEX-0033-03-RUNTIME-ORCHESTRATION-0d6882e9c6` | `crates/trpg-runtime/src/pending_decision.rs` | Primary Rust implementation | `human_kp_ai_formal_tool_becomes_draft_only_pending_decision` | Implemented |
| `CODEX-0034-03-RUNTIME-ORCHESTRATION-20e1521d8e` | `crates/trpg-runtime/src/realtime_runtime_binding.rs` | Primary Rust implementation | `realtime_runtime_binding_respects_private_player_visibility` | Implemented |
| `CODEX-0035-03-RUNTIME-ORCHESTRATION-2f52cb37ae` | `crates/trpg-runtime/src/runtime_state_machines.rs` | Primary Rust implementation | `decision_pipeline_fixture_expected_records_are_asserted`; `direct_agent_state_write_is_rejected_before_event_append`; `expected_version_and_idempotency_are_enforced` | Implemented |
| `CODEX-0036-03-RUNTIME-ORCHESTRATION-12a9414c48` | `crates/trpg-runtime/src/saga_transaction.rs` | Primary Rust implementation | `session_workflow_saga_and_scheduler_use_governed_runtime_paths` | Implemented |
| `CODEX-0037-03-RUNTIME-ORCHESTRATION-c9bd0a0635` | `crates/trpg-runtime/src/scheduler_service.rs` | Primary Rust implementation | `session_workflow_saga_and_scheduler_use_governed_runtime_paths` | Implemented |
| `CODEX-0038-03-RUNTIME-ORCHESTRATION-ec0e699332` | `crates/trpg-runtime/src/session_runtime.rs` | Primary Rust implementation | `session_workflow_saga_and_scheduler_use_governed_runtime_paths` | Implemented |
| `CODEX-0039-03-RUNTIME-ORCHESTRATION-99d8270e66` | `crates/trpg-runtime/src/workflow_engine.rs` | Primary Rust implementation | `decision_pipeline_fixture_expected_records_are_asserted`; `workflow_engine_contract_commits_decision_event_chain` | Implemented |
| `CODEX-0335-03-RUNTIME-ORCHESTRATION-0ca4a1c995` | `crates/trpg-runtime/src/adr_0007_internal_workflow_vs_temporal.rs` | Primary Rust boundary constants | `adr_boundary_keeps_external_workflows_out_of_canon` | Implemented |
| `CODEX-0336-03-RUNTIME-ORCHESTRATION-c822b429eb` | `docs/codex/90-traceability/supplemental-requirements/...md` | Supplemental traceability only | Trace file exists | Supplemental applied |
| `CODEX-0337-03-RUNTIME-ORCHESTRATION-7b50c14f8c` | `docs/codex/90-traceability/supplemental-requirements/...md` | Supplemental traceability only | Trace file exists | Supplemental applied |
| `CODEX-0338-03-RUNTIME-ORCHESTRATION-d0fdce8770` | `crates/trpg-runtime/src/capability_layer_tool_grant.rs` | Primary Rust implementation | `tool_gate_fixture_error_cases_are_asserted` | Implemented |
| `CODEX-0339-03-RUNTIME-ORCHESTRATION-d43dd17cb2` | `docs/codex/90-traceability/supplemental-requirements/...md` | Supplemental traceability only | Trace file exists | Supplemental applied |
| `CODEX-0340-03-RUNTIME-ORCHESTRATION-10b2ea170e` | `docs/codex/90-traceability/supplemental-requirements/...md` | Supplemental traceability only | Trace file exists | Supplemental applied |
| `CODEX-0341-03-RUNTIME-ORCHESTRATION-aebaddafe7` | `docs/codex/90-traceability/supplemental-requirements/...md` | Supplemental traceability only | Trace file exists | Supplemental applied |
| `CODEX-0342-03-RUNTIME-ORCHESTRATION-7a95160d72` | `docs/codex/90-traceability/supplemental-requirements/...md` | Supplemental traceability only | Trace file exists | Supplemental applied |
| `CODEX-0343-03-RUNTIME-ORCHESTRATION-e4504af27a` | `docs/codex/90-traceability/supplemental-requirements/...md` | Supplemental traceability only | Trace file exists | Supplemental applied |
| `CODEX-0344-03-RUNTIME-ORCHESTRATION-22393092aa` | `crates/trpg-runtime/src/runtime_workflow_engine.rs` | Primary Rust implementation | `ai_kp_orchestrator_commits_decision_through_tool_and_event_log`; `direct_agent_state_write_is_rejected_before_event_append` | Implemented |
| `CODEX-0345-03-RUNTIME-ORCHESTRATION-b1c8a10647` | `docs/codex/90-traceability/supplemental-requirements/...md` | Supplemental traceability only | Trace file exists | Supplemental applied |
| `CODEX-0346-03-RUNTIME-ORCHESTRATION-fc8679858e` | `crates/trpg-runtime/src/capability_layer.rs` | Primary Rust implementation | `tool_gate_fixture_error_cases_are_asserted` | Implemented |
| `CODEX-0347-03-RUNTIME-ORCHESTRATION-b0e055d98c` | `crates/trpg-runtime/src/realtime_room_sync.rs` | Primary Rust implementation | `keeper_only_runtime_events_do_not_sync_to_public_room` | Implemented |
| `CODEX-0348-03-RUNTIME-ORCHESTRATION-90faa1ed8c` | `docs/codex/90-traceability/supplemental-requirements/...md` | Supplemental traceability only | Trace file exists | Supplemental applied |
| `CODEX-0349-03-RUNTIME-ORCHESTRATION-0b68fe8e4e` | `crates/trpg-runtime/src/runtime_pending_decision.rs` | Primary Rust implementation | `runtime_pending_decision_wrapper_opens_and_commits_governed_decisions`; `runtime_pending_decision_target_commits_ready_decision` | Implemented |
| `CODEX-0350-03-RUNTIME-ORCHESTRATION-1a43208285` | `docs/codex/90-traceability/supplemental-requirements/...md` | Supplemental traceability only | Trace file exists | Supplemental applied |

## Test Plan

1. Minimum related check: `cargo test -p trpg-runtime --test batch_012_runtime_contract_tests --all-features`.
2. Target checks: `cargo test -p trpg-runtime --test runtime_pending_decision --all-features`; `cargo test -p trpg-runtime --test workflow_engine_contract --all-features`.
3. Batch crate check: `cargo test -p trpg-runtime --all-features`.
4. S06 filters: `cargo test -p trpg-runtime decision_pipeline --all-features`; `cargo test -p trpg-runtime tool_gate --all-features`.
5. Formatting gate: `cargo fmt --all -- --check`.
6. Cargo check: `cargo check --workspace --all-features`.
7. Stage/workspace impact: `cargo test --workspace --all-features`.
8. Warning gate: `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
9. Fixture check: parse JSON blocks in `fixtures/stages/S06_stage_acceptance_fixture.v1.json.md` and `fixtures/stages/detailed/S06_decision_pipeline_commit_expected.current.json.md`.

`pnpm` and Docker checks are not applicable for this batch because the workspace has no `package.json`, `pnpm-lock.yaml`, `Dockerfile`, Compose file, or compose YAML.

## Scope Not Taken

- No SQLx migrations, Axum handlers, OpenAPI schemas, NATS subjects, or provider calls were added because the required S03/S08/S07 crates are not present in this workspace yet.
- No source path, old hash, or historical intermediate filename was used as a Rust module, event, metric, migration, test, or workflow name.
