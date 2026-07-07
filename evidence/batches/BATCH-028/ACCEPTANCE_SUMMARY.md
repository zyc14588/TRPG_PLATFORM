# BATCH-028 Acceptance Summary

Batch: `BATCH-028-06-data-eventing`
Stage: `S03`
Conclusion: PASS for current batch scope.

## Changed Files

- `crates/trpg-data-eventing/src/event_json_schema.rs`
- `crates/trpg-data-eventing/src/lib.rs`
- `crates/trpg-data-eventing/tests/event_json_schema_contract_tests.rs`
- `evidence/batches/BATCH-028/WORK_PLAN.md`
- `evidence/batches/BATCH-028/PROMPT_COVERAGE.md`
- `evidence/batches/BATCH-028/TEST_RESULTS.md`
- `evidence/batches/BATCH-028/ACCEPTANCE_SUMMARY.md`

## Implementation Summary

- Added the current-safe `data_eventing::event_json_schema` flat module owned by `CODEX-0682-06-DATA-EVENTING-af0d5b5090`.
- Registered the module in `trpg-data-eventing` and exposed `batch_028_data_event_contracts()`.
- Added schema catalog metadata for command fields, event fields, visibility labels, governance error codes, NATS subjects, metrics, canonical write flow, and rebuildable read-model policy.
- Added contract tests proving governed Event Store append, expected-version/idempotency rejection, direct-agent rejection, HUMAN_KP/AI_KP authority rejection, visibility redaction, and fact provenance preservation.

## Acceptance Gates

| Gate | Result | Evidence |
|---|---|---|
| Event Store remains canon | PASS | `event_json_schema_appends_only_through_governed_event_store_path`; `event_store_contract` |
| Projection/cache/RAG remain rebuildable read models | PASS | `READ_MODEL_POLICY`; `projection_replay` |
| Formal writes require governed envelope fields | PASS | `event_json_schema_catalog_declares_governed_command_and_event_fields` |
| Authority and write-path bypass blocked | PASS | `event_json_schema_appends_only_through_governed_event_store_path` |
| Visibility and Fact Provenance propagate | PASS | `event_json_schema_preserves_visibility_provenance_and_fixture_bindings` |
| Historical tokens not used as current names | PASS | `is_current_safe_name` assertions and targeted `rg` check over B028 new Rust files returned no hits. |

## Risks

- No P0/P1 unresolved risks in current batch scope.
- Full workspace clippy and full workspace tests were not run; current evidence is limited to S03/data-eventing scope.
- `cargo` repeatedly emitted `could not canonicalize path C:\Users\zyc14588`; tests still passed.

## Handoff

Next batch may consume `data_eventing::event_json_schema` through `all_data_event_contracts()` or `batch_028_data_event_contracts()`. Do not reopen B028 supplemental prompts as implementation owners; their constraints belong to their earlier primary prompts.
