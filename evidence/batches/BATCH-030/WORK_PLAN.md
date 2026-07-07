# BATCH-030 Work Plan

Batch: `BATCH-030-07-api-realtime-contracts`
Execution label: `Strict Governance Final`
Stage: `S08 - 07-api-realtime-contracts`
Declared prompts: 23
Accepted primary prompt count for this repair: 1

## Required Inputs Read

- `AGENTS.md`
- `batches/B030.md`
- `codex-prompts/07-api-realtime-contracts/P0026.md`
- `docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`
- `docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md`
- `docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md`
- `stages/s08-api-realtime-contracts/TEST_PLAN.md`
- `stages/s08-api-realtime-contracts/TEST_DATA.md`
- `stages/s08-api-realtime-contracts/ACCEPTANCE_PROMPT.md`
- `docs/codex/90-traceability/supplemental-requirements/CODEX-0719-07-API-REALTIME-CONTRACTS-ccf8b3c12e.md`

## Execution Scope Decision

This repair accepts only `CODEX-0700-07-API-REALTIME-CONTRACTS-32445eadff` as the in-scope primary implementation prompt. The current-safe target is `api_realtime_contracts::readme`. Rust changes are limited to:

- `crates/trpg-api/src/readme.rs`
- `crates/trpg-api/tests/readme_contract_tests.rs`
- `crates/trpg-api/src/lib.rs` module export only: `pub mod readme;`

`CODEX-0719` remains supplemental and is merged into the readme primary without creating an independent implementation surface.

## Prompt Mapping

| Prompt | Role | Current-safe target | Result basis |
|---|---|---|---|
| `CODEX-0700` | Primary implementation | `api_realtime_contracts::readme` | Implemented in `crates/trpg-api/src/readme.rs`; exported by `crates/trpg-api/src/lib.rs:13`; covered by `crates/trpg-api/tests/readme_contract_tests.rs`. |
| `CODEX-0701` | Supplemental | `api_realtime_contracts::realtime_sync` | Supplemental merge instruction exists; no independent implementation scope. |
| `CODEX-0702` | Supplemental | `api_realtime_contracts::request_idempotency_contract` | Supplemental merge instruction exists; no independent implementation scope. |
| `CODEX-0703` | Supplemental | `api_realtime_contracts::websocket_protocol` | Supplemental merge instruction exists; no independent implementation scope. |
| `CODEX-0704` | Supplemental | `api_realtime_contracts::realtime_room_sync` | Supplemental merge instruction exists; no independent implementation scope. |
| `CODEX-0705` | Documentation | API OpenAPI source processing record | Documentation-only evidence file exists. |
| `CODEX-0706` | Documentation | OpenAPI index source processing record | Documentation-only evidence file exists. |
| `CODEX-0707` | Documentation | API and transport source processing record | Documentation-only evidence file exists. |
| `CODEX-0708` | Documentation | Realtime sync source processing record | Documentation-only evidence file exists. |
| `CODEX-0709` | Documentation | Readme source processing record | Documentation-only evidence file exists. |
| `CODEX-0710` | Documentation | Request idempotency source processing record | Documentation-only evidence file exists. |
| `CODEX-0711` | Documentation | External provider source processing record | Documentation-only evidence file exists. |
| `CODEX-0712` | Documentation | WebSocket protocol source processing record | Documentation-only evidence file exists. |
| `CODEX-0713` | Documentation | Platform API contracts traceability source processing record | Documentation-only evidence file exists. |
| `CODEX-0714` | Documentation | Platform API contracts source processing record | Documentation-only evidence file exists. |
| `CODEX-0715` | Supplemental | `api_realtime_contracts::openapi` | Supplemental merge instruction exists; no independent implementation scope. |
| `CODEX-0716` | Supplemental | `api_realtime_contracts::api_and_transport` | Supplemental merge instruction exists; no independent implementation scope. |
| `CODEX-0717` | Supplemental | `api_realtime_contracts::external_provider_contracts` | Supplemental merge instruction exists; no independent implementation scope. |
| `CODEX-0718` | Supplemental | `api_realtime_contracts::openapi_index` | Supplemental merge instruction exists; no independent implementation scope. |
| `CODEX-0719` | Supplemental | `api_realtime_contracts::readme` | Merged into `CODEX-0700` readme implementation and tests. |
| `CODEX-0720` | Supplemental | `api_realtime_contracts::realtime_sync` | Supplemental merge instruction exists; no independent implementation scope. |
| `CODEX-0721` | Supplemental | `api_realtime_contracts::request_idempotency_contract` | Supplemental merge instruction exists; no independent implementation scope. |
| `CODEX-0722` | Supplemental | `api_realtime_contracts::websocket_protocol` | Supplemental merge instruction exists; no independent implementation scope. |

## Planned Checks

1. Boundary scans for direct provider calls, private fixture leakage, and historical naming misuse in the scoped `CODEX-0700` Rust files.
2. Required Rust checks:
   - `cargo fmt --all -- --check`
   - `cargo check -p trpg-api`
   - `cargo clippy -p trpg-api --all-targets --all-features -- -D warnings`
   - `cargo test -p trpg-api --test readme_contract_tests --all-features --jobs 1`
   - `cargo test -p trpg-api --test s08_fixture_acceptance_contract_tests --all-features --jobs 1`
   - `$env:CARGO_TARGET_DIR='target\codex-b030-verify'; cargo test -p trpg-api --all-features --jobs 1`
   - `$env:CARGO_TARGET_DIR='target\codex-b030-verify'; cargo test --workspace --all-features --jobs 1`
   - `$env:CARGO_TARGET_DIR='target\codex-b030-verify'; cargo clippy --workspace --all-targets --all-features -- -D warnings`
3. S08 fixture coverage is verified by the dedicated fixture test and the package/workspace all-features runs.
