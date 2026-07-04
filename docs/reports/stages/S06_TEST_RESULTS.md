# S06 Test Results - BATCH-013

Stage: `S06`
Batch: `BATCH-013-03-runtime-orchestration`
Date: 2026-07-04
Scope: recorded results from strict acceptance reruns for BATCH-013/S06.

## Targeted B013 Tests

| Command | Result |
|---|---|
| `cargo test -p trpg-runtime --test saga_contract_tests --all-features` | PASS, 2 passed |
| `cargo test -p trpg-runtime --test campaign_session_runtime_service_contract_tests --all-features` | PASS, 2 passed |
| `cargo test -p trpg-runtime --test runtime_contract_tests --all-features` | PASS, 5 passed |
| `cargo test -p trpg-runtime --test readme_contract_tests --all-features` | PASS, 1 passed |

## S06 Required Runtime Tests

| Command | Result |
|---|---|
| `cargo test -p trpg-runtime --all-features` | PASS |
| `cargo test -p trpg-runtime --test runtime_pending_decision --all-features` | PASS, 1 passed |
| `cargo test -p trpg-runtime --test workflow_engine_contract --all-features` | PASS, 1 passed |
| `cargo test -p trpg-runtime decision_pipeline --all-features` | PASS, 1 matching test passed |
| `cargo test -p trpg-runtime tool_gate --all-features` | PASS, 1 matching test passed |

## Workspace Gates

| Command | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo check --workspace --all-features` | PASS |
| `cargo test --workspace --all-features` | PASS |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |

## Fixture Checks

JSON fenced blocks parsed successfully for:

- `fixtures/stages/S06_stage_acceptance_fixture.v1.json.md`
- `fixtures/stages/detailed/S06_decision_pipeline_commit_expected.current.json.md`
- `fixtures/agent/ai_decision_record_cases.v1.json.md`
- `fixtures/event_store/golden_event_stream_expected.v1.json.md`
- `fixtures/authority/authority_contract_cases.v1.json.md`
- `test-data/agent_tool_call_cases.md`
- `test-data/event_store_stream_cases.md`
- `test-data/authority_contract_cases.md`

## Non-applicable Checks

`rg --files -g package.json -g pnpm-lock.yaml -g Dockerfile -g docker-compose.yml -g docker-compose.yaml -g compose.yml -g compose.yaml` returned no matches. `pnpm` and Docker checks are not applicable for this batch.

## Notes

An initial parallel Cargo test run recorded in batch evidence failed due a Windows linker output file lock. Sequential reruns passed and are the accepted results above.
