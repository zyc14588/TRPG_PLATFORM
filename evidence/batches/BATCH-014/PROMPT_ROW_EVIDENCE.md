# BATCH-014 Prompt Row Evidence

All 25 prompt rows from `batches/B014.md` were checked against `CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`, `CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md`, and `CURRENT_TOKEN_REWRITE_TABLE.md`.

| Row | Prompt ID | Prompt file | Type | Result | Evidence |
|---:|---|---|---|---:|---|
| 1 | `CODEX-0376-03-RUNTIME-ORCHESTRATION-2291e83e48` | `P0051.md` | supplemental-requirement | PASS | Markdown-only supplemental file exists at `docs/codex/90-traceability/supplemental-requirements/CODEX-0376-03-RUNTIME-ORCHESTRATION-2291e83e48.md`; no Rust scope claimed. |
| 2 | `CODEX-0377-03-RUNTIME-ORCHESTRATION-fc718c91e6` | `P0052.md` | primary-implementation | PASS | Implemented `crates/trpg-runtime/src/runtime_workflow_state_machines.rs`; tested by `crates/trpg-runtime/tests/runtime_workflow_state_machines_contract_tests.rs`. |
| 3 | `CODEX-0378-03-RUNTIME-ORCHESTRATION-12ef641d01` | `P0053.md` | supplemental-requirement | PASS | Markdown-only supplemental file exists at `docs/codex/90-traceability/supplemental-requirements/CODEX-0378-03-RUNTIME-ORCHESTRATION-12ef641d01.md`; no Rust scope claimed. |
| 4 | `CODEX-0379-03-RUNTIME-ORCHESTRATION-37bd987305` | `P0054.md` | supplemental-requirement | PASS | Markdown-only supplemental file exists at `docs/codex/90-traceability/supplemental-requirements/CODEX-0379-03-RUNTIME-ORCHESTRATION-37bd987305.md`; no Rust scope claimed. |
| 5 | `CODEX-0380-03-RUNTIME-ORCHESTRATION-4b721c9719` | `P0055.md` | supplemental-requirement | PASS | Markdown-only supplemental file exists at `docs/codex/90-traceability/supplemental-requirements/CODEX-0380-03-RUNTIME-ORCHESTRATION-4b721c9719.md`; no Rust scope claimed. |
| 6 | `CODEX-0381-03-RUNTIME-ORCHESTRATION-d8af636438` | `P0056.md` | supplemental-requirement | PASS | Markdown-only supplemental file exists at `docs/codex/90-traceability/supplemental-requirements/CODEX-0381-03-RUNTIME-ORCHESTRATION-d8af636438.md`; no Rust scope claimed. |
| 7 | `CODEX-0382-03-RUNTIME-ORCHESTRATION-afd897d786` | `P0058.md` | supplemental-requirement | PASS | Markdown-only supplemental file exists at `docs/codex/90-traceability/supplemental-requirements/CODEX-0382-03-RUNTIME-ORCHESTRATION-afd897d786.md`; no Rust scope claimed. |
| 8 | `CODEX-0383-03-RUNTIME-ORCHESTRATION-e02aaf736d` | `P0060.md` | supplemental-requirement | PASS | Markdown-only supplemental file exists at `docs/codex/90-traceability/supplemental-requirements/CODEX-0383-03-RUNTIME-ORCHESTRATION-e02aaf736d.md`; no Rust scope claimed. |
| 9 | `CODEX-0384-03-RUNTIME-ORCHESTRATION-456e879b3c` | `P0057.md` | supplemental-requirement | PASS | Markdown-only supplemental file exists at `docs/codex/90-traceability/supplemental-requirements/CODEX-0384-03-RUNTIME-ORCHESTRATION-456e879b3c.md`; no Rust scope claimed. |
| 10 | `CODEX-0385-03-RUNTIME-ORCHESTRATION-bb77496a2f` | `P0059.md` | supplemental-requirement | PASS | Markdown-only supplemental file exists at `docs/codex/90-traceability/supplemental-requirements/CODEX-0385-03-RUNTIME-ORCHESTRATION-bb77496a2f.md`; no Rust scope claimed. |
| 11 | `CODEX-0386-03-RUNTIME-ORCHESTRATION-027bb089fe` | `P0061.md` | primary-implementation | PASS | Implemented `crates/trpg-runtime/src/capability_layer_impl.rs`; tested by `crates/trpg-runtime/tests/capability_layer_impl_contract_tests.rs`. |
| 12 | `CODEX-0387-03-RUNTIME-ORCHESTRATION-ff36c2cdcf` | `P0062.md` | primary-implementation | PASS | Implemented `crates/trpg-runtime/src/pending_decision_impl.rs`; tested by `crates/trpg-runtime/tests/pending_decision_impl_contract_tests.rs`. |
| 13 | `CODEX-0388-03-RUNTIME-ORCHESTRATION-705a854eb2` | `P0063.md` | primary-implementation | PASS | Implemented `crates/trpg-runtime/src/realtime_room_sync_impl.rs`; tested by `crates/trpg-runtime/tests/realtime_room_sync_impl_contract_tests.rs`. |
| 14 | `CODEX-0389-03-RUNTIME-ORCHESTRATION-1b60a8b386` | `P0064.md` | primary-implementation | PASS | Implemented `crates/trpg-runtime/src/saga_transaction_impl.rs`; tested by `crates/trpg-runtime/tests/saga_transaction_impl_contract_tests.rs`. |
| 15 | `CODEX-0390-03-RUNTIME-ORCHESTRATION-12323c9bd9` | `P0065.md` | primary-implementation | PASS | Implemented `crates/trpg-runtime/src/scheduler_service_impl.rs`; tested by `crates/trpg-runtime/tests/scheduler_service_impl_contract_tests.rs`. |
| 16 | `CODEX-0391-03-RUNTIME-ORCHESTRATION-daba262944` | `P0066.md` | primary-implementation | PASS | Implemented `crates/trpg-runtime/src/session_runtime_impl.rs`; tested by `crates/trpg-runtime/tests/session_runtime_impl_contract_tests.rs`. |
| 17 | `CODEX-0392-03-RUNTIME-ORCHESTRATION-1cb6fb735e` | `P0067.md` | primary-implementation | PASS | Implemented `crates/trpg-runtime/src/workflow_engine_impl.rs`; tested by `crates/trpg-runtime/tests/workflow_engine_impl_contract_tests.rs`. |
| 18 | `CODEX-0393-03-RUNTIME-ORCHESTRATION-bce0a108cd` | `P0068.md` | documentation-or-traceability | PASS | Markdown trace exists at `docs/codex/03-runtime-orchestration/source_processing_record_docs_adr_adr_0007_internal_workflow_vs_temporal.md`; no Rust scope claimed. |
| 19 | `CODEX-0394-03-RUNTIME-ORCHESTRATION-e366f4108a` | `P0088.md` | documentation-or-traceability | PASS | Markdown trace exists at `docs/codex/03-runtime-orchestration/source_processing_record_docs_implementation_03_runtime_orchestration_pending_decision.md`; no Rust scope claimed. |
| 20 | `CODEX-0395-03-RUNTIME-ORCHESTRATION-cc96557b91` | `P0087.md` | documentation-or-traceability | PASS | Markdown trace exists at `docs/codex/03-runtime-orchestration/source_processing_record_docs_implementation_03_runtime_orchestration_runtime_state_machines.md`; no Rust scope claimed. |
| 21 | `CODEX-0396-03-RUNTIME-ORCHESTRATION-1f13d01855` | `P0071.md` | documentation-or-traceability | PASS | Markdown trace exists at `docs/codex/03-runtime-orchestration/source_processing_record_docs_implementation_03_runtime_orchestration_capability_tool_grant.md`; no Rust scope claimed. |
| 22 | `CODEX-0397-03-RUNTIME-ORCHESTRATION-871de3b645` | `P0082.md` | documentation-or-traceability | PASS | Markdown trace exists at `docs/codex/03-runtime-orchestration/source_processing_record_docs_implementation_03_runtime_orchestration_scheduler_service.md`; no Rust scope claimed. |
| 23 | `CODEX-0398-03-RUNTIME-ORCHESTRATION-e731ee37d5` | `P0089.md` | documentation-or-traceability | PASS | Markdown trace exists at `docs/codex/03-runtime-orchestration/source_processing_record_docs_implementation_03_runtime_orchestration_readme.md`; no Rust scope claimed. |
| 24 | `CODEX-0399-03-RUNTIME-ORCHESTRATION-8ddf33c33e` | `P0083.md` | documentation-or-traceability | PASS | Markdown trace exists at `docs/codex/03-runtime-orchestration/source_processing_record_docs_implementation_03_runtime_orchestration_workflow_engine.md`; no Rust scope claimed. |
| 25 | `CODEX-0400-03-RUNTIME-ORCHESTRATION-b6a0a221c2` | `P0086.md` | documentation-or-traceability | PASS | Markdown trace exists at `docs/codex/03-runtime-orchestration/source_processing_record_docs_implementation_03_runtime_orchestration_saga_transaction.md`; no Rust scope claimed. |

## Shared Primary Test Assertions

Every B014 primary contract test asserts:

- The current-safe prompt ID.
- A governed `CommandEnvelope`.
- Event Store append through the runtime decision pipeline.
- `ToolRequestApproved` and `DecisionCommitted` event types.
- Visibility label propagation and no public replay of `KeeperOnly` events.
- Fact provenance propagation from command to event.
- Authority Contract mismatch/fork rejection.
- Tool Gate denial for non-orchestrator formal state changes.
- `FormalWritePath::DirectAgent` rejection before any event append.
