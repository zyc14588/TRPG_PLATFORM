# BATCH-013 Prompt Traceability

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
- `codex-operator-guides/04_TESTING_PLAYBOOK.md`
- `codex-operator-guides/10_STRICT_VALIDATION_COMMANDS.md`
- `V1_ACCEPTANCE_EVIDENCE_MATRIX.md`
- `02_STAGE_CONFIRMATION_MATRIX.md`
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
- `docs/codex/03-runtime-orchestration/codex-module-code-prompt.md`
- `docs/codex/03-runtime-orchestration/codex-module-test-prompt.md`
- `docs/codex/03-runtime-orchestration/per-file-prompt-manifest.md`
- `batches/B013.md`
- B013 per-file prompts: `P0014`, `P0017`, `P0018`, `P0019`, `P0026`, `P0027`, `P0032`, `P0033`, `P0034`, `P0035`, `P0036`, `P0037`, `P0038`, `P0039`, `P0040`, `P0041`, `P0042`, `P0043`, `P0044`, `P0045`, `P0046`, `P0047`, `P0048`, `P0049`, `P0050`.

## Implementation Trace

| Prompt role | Count | Evidence |
|---|---:|---|
| Primary implementation applied | 4 | `crates/trpg-runtime/src/saga.rs`; `crates/trpg-runtime/src/campaign_session_runtime_service.rs`; `crates/trpg-runtime/src/readme.rs`; `crates/trpg-runtime/src/runtime.rs`; matching tests |
| Supplemental requirement applied | 21 | `docs/codex/90-traceability/supplemental-requirements/CODEX-0351...md` through `CODEX-0375...md` for B013 supplemental rows |
| Documentation/traceability only | 0 | B013 has no docs-only rows |

## Primary Test Coverage

| Prompt ID | Targeted evidence |
|---|---|
| `CODEX-0353-03-RUNTIME-ORCHESTRATION-b1f275b36f` | `saga_compensation_uses_governed_event_append`; `saga_rejects_direct_agent_state_write` |
| `CODEX-0355-03-RUNTIME-ORCHESTRATION-bbee275591` | `campaign_session_runtime_service_appends_session_and_workflow_events`; `campaign_session_runtime_service_enforces_expected_version` |
| `CODEX-0358-03-RUNTIME-ORCHESTRATION-5626fcbd5c` | `readme_contract_records_runtime_governance_invariants` |
| `CODEX-0363-03-RUNTIME-ORCHESTRATION-2b19458f57` | `runtime_indexes_current_batch_primary_modules`; `runtime_boundary_snapshot_preserves_s06_governance_fields`; `runtime_commits_ai_kp_decision_through_evented_pipeline`; `runtime_replay_does_not_expose_keeper_only_events_to_public`; `runtime_rejects_agent_direct_write_before_append` |

## Governance Checks

- Formal runtime writes use `CommandEnvelope`, `AuthorityContract`, and `EventStore`.
- B013 runtime entrypoints reuse `runtime_state_machines::append_runtime_event` and `commit_decision`.
- Direct agent write is rejected before event append in B013 `saga` and `runtime` tests.
- Visibility and fact provenance are copied from command envelope into event envelopes.
- Public replay does not expose keeper-only runtime events.
- Supplemental prompts did not create Rust outputs.
- No direct LLM/provider references were found in `crates/trpg-runtime` or `crates/trpg-shared-kernel`.

## Known Governance Mismatch

The launcher metadata says `primary prompt count = 0`, but `batches/B013.md` and the normalized maps identify 4 primary implementation rows. This run followed the normalized row-level batch source and records the launcher mismatch for operator review.
