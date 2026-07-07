# BATCH-029 Test Results

Batch: BATCH-029-07-api-realtime-contracts
Evidence date: 2026-07-07

## Commands

| Order | Command | Scope | Result |
| --- | --- | --- | --- |
| 1 | `cargo fmt --all` | Formatting after repair | PASS |
| 2 | `cargo check -p trpg-api` | Minimal compile check for B029 crate | PASS |
| 3 | `cargo test -p trpg-api --all-features` | Default parallel package test | RETRIED: Windows `link.exe` returned LNK1104 opening output test exe files |
| 4 | `cargo test -p trpg-api --test s08_fixture_acceptance_contract_tests --all-features` | S08 detailed fixture automation target | PASS, 4 tests |
| 5 | `cargo test -p trpg-api --all-features --jobs 1` | Full B029 package tests, serial linker mode | PASS |
| 6 | `cargo fmt --all -- --check` | Formatting verification | PASS |
| 7 | `cargo test --workspace --all-features --jobs 1` | Stage/workspace regression test | PASS |
| 8 | `cargo clippy --workspace --all-targets --all-features --jobs 1 -- -D warnings` | Stage/workspace lint gate | PASS |
| 9 | `cargo check --workspace --all-features --jobs 1` | Workspace compile check | PASS |
| 10 | `rg` over the known old failed supplemental hash set in B029 evidence and B029 docs | Old failed supplemental ID scan | PASS, no matches |
| 11 | `rg -n "OpenAI|Ollama|llama\\.cpp|chat_completion|responses\\.create|createChatCompletion|CompletionCreate|direct LLM|bare LLM" crates/trpg-api docs/codex/07-api-realtime-contracts/m_07_api_realtime_contracts.md` | Direct model access scan | PASS, no matches |
| 12 | `rg -n "automation_target|cargo test -p trpg-api" fixtures/stages/detailed/S08_api_ws_nats_expected.current.json.md crates/trpg-api/tests/s08_fixture_acceptance_contract_tests.rs evidence/batches/BATCH-029` | Fixture automation target binding | PASS |
| 13 | `rg --files -g package.json -g pnpm-lock.yaml -g pnpm-workspace.yaml` | pnpm applicability check | N/A, no pnpm/package targets |
| 14 | `rg --files -g Dockerfile -g "*.Dockerfile" -g docker-compose.yml -g docker-compose.yaml -g compose.yml -g compose.yaml` | docker applicability check | N/A, no docker targets |

## Notes

- Cargo still emits the environment warning `could not canonicalize path C:\Users\zyc14588` on some commands; the successful commands completed.
- The default parallel `cargo test -p trpg-api --all-features` failed at Windows `link.exe` with LNK1104 while opening output test executables. The same package test suite passed with `--jobs 1`, and the new S08 fixture test target passed directly.
- The invalid fixture target `cargo test -p trpg-api openapi websocket nats_subject --all-features` was replaced with `cargo test -p trpg-api --test s08_fixture_acceptance_contract_tests --all-features`.
- No network access or dependency downloads were required.

## Test Coverage Added Or Updated

- `crates/trpg-api/tests/batch_029_api_realtime_contract_tests.rs`
- `crates/trpg-api/tests/s08_fixture_acceptance_contract_tests.rs`
- `crates/trpg-api/tests/api_and_transport_contract_tests.rs`
- `crates/trpg-api/tests/api_contract_tests.rs`
- `crates/trpg-api/tests/api_contracts_contract_tests.rs`
- `crates/trpg-api/tests/api_web_socket_contract_tests.rs`
- `crates/trpg-api/tests/api_web_socket_g_rpc_schema_contract_tests.rs`
- `crates/trpg-api/tests/external_provider_contracts_contract_tests.rs`
- `crates/trpg-api/tests/nats_subject_contracts_contract_tests.rs`
- `crates/trpg-api/tests/openapi_contract_contract_tests.rs`
- `crates/trpg-api/tests/openapi_contract_tests.rs`
- `crates/trpg-api/tests/openapi_index_contract_tests.rs`
- `crates/trpg-api/tests/provider_contract_tests.rs`
- `crates/trpg-api/tests/realtime_room_sync_contract_tests.rs`
- `crates/trpg-api/tests/realtime_sync_contract_tests.rs`
- `crates/trpg-api/tests/request_idempotency_contract_contract_tests.rs`
- `crates/trpg-api/tests/websocket_protocol_contract_tests.rs`
