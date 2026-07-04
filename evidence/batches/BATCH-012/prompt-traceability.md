# BATCH-012 Prompt Traceability

## Inputs Read

- `AGENTS.md`
- `CODEX_STANDALONE_BOOTSTRAP_PROMPT.md`
- `SOURCE_BUNDLE_INTEGRATION_GUIDE.md`
- `docs/top-level-design/CURRENT_TOP_LEVEL_DESIGN.md`
- `docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`
- `docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md`
- `docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md`
- `CODEX_MASTER_EXECUTION_GUIDE.md`
- `CODEX_START_ACCEPT_TEST_RELEASE_GUIDE.md`
- `CODEX_STRICT_OPERATION_CHECKLIST.md`
- `codex-operator-guides/README.md`
- `V1_ACCEPTANCE_EVIDENCE_MATRIX.md`
- `PER_STAGE_FIXTURE_EXPANSION_PLAN.md`
- `docs/codex/00-index/codex-persistent-context.md`
- `docs/codex/00-index/codex-prompt-boundary.md`
- `stages/s06-runtime-orchestration-decision-pipeline/README.md`
- `stages/s06-runtime-orchestration-decision-pipeline/START_PROMPT.md`
- `stages/s06-runtime-orchestration-decision-pipeline/TEST_PLAN.md`
- `stages/s06-runtime-orchestration-decision-pipeline/TEST_DATA.md`
- `stages/s06-runtime-orchestration-decision-pipeline/ACCEPTANCE_PROMPT.md`
- `stages/s06-runtime-orchestration-decision-pipeline/REPAIR_PROMPT.md`
- `docs/codex/03-runtime-orchestration/AGENTS.md`
- `docs/codex/03-runtime-orchestration/README.md`
- `docs/codex/03-runtime-orchestration/per-file-prompt-manifest.md`
- `batches/B012.md`
- B012 per-file prompts: `P0001`, `P0002`, `P0003`, `P0004`, `P0005`, `P0006`, `P0007`, `P0008`, `P0009`, `P0010`, `P0011`, `P0012`, `P0013`, `P0015`, `P0016`, `P0020`, `P0021`, `P0022`, `P0023`, `P0024`, `P0025`, `P0028`, `P0029`, `P0030`, `P0031`.

## Implementation Trace

| Prompt role | Count | Evidence |
|---|---:|---|
| Primary implementation applied | 14 | `crates/trpg-runtime/src/*.rs`; `crates/trpg-runtime/tests/batch_012_runtime_contract_tests.rs`; `crates/trpg-runtime/tests/runtime_pending_decision.rs`; `crates/trpg-runtime/tests/workflow_engine_contract.rs` |
| Supplemental requirement applied | 10 | `docs/codex/90-traceability/supplemental-requirements/*.md` |
| Documentation/traceability only | 1 | `docs/codex/03-runtime-orchestration/m_03_runtime_orchestration.md`; batch evidence records current status |

## Primary Test Coverage

| Prompt ID | Targeted evidence |
|---|---|
| `CODEX-0032-03-RUNTIME-ORCHESTRATION-20830a72ac` | `tool_gate_fixture_error_cases_are_asserted`; `non_orchestrator_agent_cannot_request_formal_state_tool` |
| `CODEX-0033-03-RUNTIME-ORCHESTRATION-0d6882e9c6` | `human_kp_ai_formal_tool_becomes_draft_only_pending_decision` |
| `CODEX-0034-03-RUNTIME-ORCHESTRATION-20e1521d8e` | `realtime_runtime_binding_respects_private_player_visibility` |
| `CODEX-0035-03-RUNTIME-ORCHESTRATION-2f52cb37ae` | `decision_pipeline_fixture_expected_records_are_asserted`; `direct_agent_state_write_is_rejected_before_event_append`; `expected_version_and_idempotency_are_enforced` |
| `CODEX-0036-03-RUNTIME-ORCHESTRATION-12a9414c48` | `session_workflow_saga_and_scheduler_use_governed_runtime_paths` |
| `CODEX-0037-03-RUNTIME-ORCHESTRATION-c9bd0a0635` | `session_workflow_saga_and_scheduler_use_governed_runtime_paths` |
| `CODEX-0038-03-RUNTIME-ORCHESTRATION-ec0e699332` | `session_workflow_saga_and_scheduler_use_governed_runtime_paths` |
| `CODEX-0039-03-RUNTIME-ORCHESTRATION-99d8270e66` | `decision_pipeline_fixture_expected_records_are_asserted`; `workflow_engine_contract_commits_decision_event_chain` |
| `CODEX-0335-03-RUNTIME-ORCHESTRATION-0ca4a1c995` | `adr_boundary_keeps_external_workflows_out_of_canon` |
| `CODEX-0338-03-RUNTIME-ORCHESTRATION-d0fdce8770` | `tool_gate_fixture_error_cases_are_asserted` |
| `CODEX-0344-03-RUNTIME-ORCHESTRATION-22393092aa` | `ai_kp_orchestrator_commits_decision_through_tool_and_event_log`; `direct_agent_state_write_is_rejected_before_event_append` |
| `CODEX-0346-03-RUNTIME-ORCHESTRATION-fc8679858e` | `tool_gate_fixture_error_cases_are_asserted` |
| `CODEX-0347-03-RUNTIME-ORCHESTRATION-b0e055d98c` | `keeper_only_runtime_events_do_not_sync_to_public_room` |
| `CODEX-0349-03-RUNTIME-ORCHESTRATION-0b68fe8e4e` | `runtime_pending_decision_wrapper_opens_and_commits_governed_decisions`; `runtime_pending_decision_target_commits_ready_decision` |

## Governance Checks

- No direct OpenAI/Ollama/llama/provider call was added.
- No agent direct database write path was added.
- Formal runtime decisions use `CommandEnvelope`, `AuthorityContract`, and `EventStore`.
- Visibility is carried on events and public replay excludes keeper-only events.
- Fact provenance remains inherited from `CommandEnvelope` into event envelopes.
- Supplemental prompts did not create Rust outputs.
- S06 fixtures are included by `include_str!` in `batch_012_runtime_contract_tests.rs`; the detailed fixture `automation_target` points to the executable batch test target.

## Known Governance Mismatch

`batch-prompts/start/B012.md` declares `识别到 primary prompt 数量：0`, but current batch and manifest rows include 14 primary prompts. The implementation follows `batches/B012.md` plus normalized output paths and records the mismatch for operator review.
