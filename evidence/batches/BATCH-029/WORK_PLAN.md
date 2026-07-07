# BATCH-029-07 API Realtime Contracts Work Plan

Batch: BATCH-029-07-api-realtime-contracts
Mode: Strict Governance Final
Evidence date: 2026-07-07

## Authority Inputs Read

- AGENTS.md
- CODEX_STANDALONE_BOOTSTRAP_PROMPT.md
- SOURCE_BUNDLE_INTEGRATION_GUIDE.md
- docs/top-level-design/CURRENT_TOP_LEVEL_DESIGN.md
- docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md
- docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md
- docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md
- CODEX_MASTER_EXECUTION_GUIDE.md
- CODEX_START_ACCEPT_TEST_RELEASE_GUIDE.md
- CODEX_STRICT_OPERATION_CHECKLIST.md
- codex-operator-guides/README.md
- batches/B029.md
- stages/s08-api-realtime-contracts/README.md
- stages/s08-api-realtime-contracts/START_PROMPT.md
- stages/s08-api-realtime-contracts/TEST_PLAN.md
- stages/s08-api-realtime-contracts/TEST_DATA.md
- stages/s08-api-realtime-contracts/ACCEPTANCE_PROMPT.md
- stages/s08-api-realtime-contracts/REPAIR_PROMPT.md
- docs/codex/07-api-realtime-contracts/AGENTS.md
- docs/codex/07-api-realtime-contracts/README.md
- docs/codex/07-api-realtime-contracts/per-file-prompt-manifest.md
- codex-prompts/07-api-realtime-contracts/P0001.md through P0025.md

## Batch Fact Reconciliation

The user-provided runtime fact says B029 has 25 declared prompts and 0 primary prompts. The repository authority files read for this run show 25 prompts with 15 primary-implementation prompts, 9 supplemental-requirement prompts, and 1 documentation-or-traceability prompt. This run followed the local repository authority after applying the normalized prompt map and current-safe module/output map.

## Scope Guard

- Only P0001 through P0025 were executed.
- No later batch prompts were opened for implementation scope.
- No source-archive path or historical V3/V4/V5/V6 name was promoted to a current module, event, workflow, migration, metric, test, or output name.
- Supplemental prompts were merged into current primary contracts as requirements, tests, and traceability only.
- All formal API write behavior remains constrained to Command -> Workflow -> Decision -> Event Store -> Projection.
- Provider access is represented only through the Agent Gateway policy path.

## Prompt Work Plan

| Prompt | Prompt ID | Type | Current-safe target | Allowed change range | Test responsibility |
| --- | --- | --- | --- | --- | --- |
| P0001 | CODEX-0066-07-API-REALTIME-CONTRACTS-831b0504c2 | primary-implementation | crates/trpg-api/src/api_and_transport.rs | API transport contract fields, event append guard, visibility provenance guard | crates/trpg-api/tests/api_and_transport_contract_tests.rs; batch governance tests |
| P0002 | CODEX-0067-07-API-REALTIME-CONTRACTS-1ccbeea1df | primary-implementation | crates/trpg-api/src/external_provider_contracts.rs | External provider access policy contract; gateway-only provider boundary | crates/trpg-api/tests/external_provider_contracts_contract_tests.rs; provider policy tests |
| P0003 | CODEX-0068-07-API-REALTIME-CONTRACTS-2b78603401 | primary-implementation | crates/trpg-api/src/nats_subject_contracts.rs | Current-safe NATS subject contract names and validation | crates/trpg-api/tests/nats_subject_contracts_contract_tests.rs; subject validation tests |
| P0004 | CODEX-0069-07-API-REALTIME-CONTRACTS-3cc61a7d01 | primary-implementation | crates/trpg-api/src/openapi_index.rs | OpenAPI index contract metadata and governance fields | crates/trpg-api/tests/openapi_index_contract_tests.rs; OpenAPI metadata tests |
| P0005 | CODEX-0065-07-API-REALTIME-CONTRACTS-4a235df615 | documentation-or-traceability | docs/codex/07-api-realtime-contracts/m_07_api_realtime_contracts.md | Batch contract documentation and traceability only | Evidence review; clippy/test commands confirm code references compile |
| P0006 | CODEX-0070-07-API-REALTIME-CONTRACTS-40bb6959f3 | primary-implementation | crates/trpg-api/src/realtime_sync.rs | Realtime delta visibility, projection rebuild, replay contract | crates/trpg-api/tests/realtime_sync_contract_tests.rs; projection tests |
| P0007 | CODEX-0071-07-API-REALTIME-CONTRACTS-3277264d0e | primary-implementation | crates/trpg-api/src/request_idempotency_contract.rs | Request idempotency and expected-version contract | crates/trpg-api/tests/request_idempotency_contract_contract_tests.rs; duplicate command tests |
| P0008 | CODEX-0072-07-API-REALTIME-CONTRACTS-513ac60dc8 | primary-implementation | crates/trpg-api/src/websocket_protocol.rs | WebSocket protocol contract metadata and visibility-safe deltas | crates/trpg-api/tests/websocket_protocol_contract_tests.rs |
| P0009 | CODEX-0683-07-API-REALTIME-CONTRACTS-80f5c71054 | supplemental-requirement | docs/codex/90-traceability/supplemental-requirements/CODEX-0683-07-API-REALTIME-CONTRACTS-80f5c71054.md; merged into api_and_transport | Merge instruction only; no independent Rust output | Covered by api_and_transport and batch tests |
| P0010 | CODEX-0684-07-API-REALTIME-CONTRACTS-93d7eb8719 | supplemental-requirement | docs/codex/90-traceability/supplemental-requirements/CODEX-0684-07-API-REALTIME-CONTRACTS-93d7eb8719.md; merged into external_provider_contracts | Merge instruction only; no independent Rust output | Covered by external_provider_contracts and provider tests |
| P0011 | CODEX-0687-07-API-REALTIME-CONTRACTS-1d88035bc8 | primary-implementation | crates/trpg-api/src/api_contracts.rs | API contract aggregate metadata and formal write-flow guard | crates/trpg-api/tests/api_contracts_contract_tests.rs |
| P0012 | CODEX-0689-07-API-REALTIME-CONTRACTS-4b17a0fb09 | primary-implementation | crates/trpg-api/src/openapi.rs | OpenAPI document contract and required governance fields | crates/trpg-api/tests/openapi_contract_tests.rs |
| P0013 | CODEX-0686-07-API-REALTIME-CONTRACTS-54d06d623d | primary-implementation | crates/trpg-api/src/realtime_room_sync.rs | Room-scoped realtime sync contract and visibility replay | crates/trpg-api/tests/realtime_room_sync_contract_tests.rs |
| P0014 | CODEX-0697-07-API-REALTIME-CONTRACTS-57f9fff7f4 | supplemental-requirement | docs/codex/90-traceability/supplemental-requirements/CODEX-0697-07-API-REALTIME-CONTRACTS-57f9fff7f4.md; merged into api_web_socket_g_rpc_schema | Merge instruction only; no independent Rust output | Covered by api_web_socket_g_rpc_schema tests |
| P0015 | CODEX-0685-07-API-REALTIME-CONTRACTS-5d2e1fa760 | primary-implementation | crates/trpg-api/src/api_web_socket.rs | WebSocket API contract and realtime governance metadata | crates/trpg-api/tests/api_web_socket_contract_tests.rs |
| P0016 | CODEX-0694-07-API-REALTIME-CONTRACTS-87a18e12b6 | supplemental-requirement | docs/codex/90-traceability/supplemental-requirements/CODEX-0694-07-API-REALTIME-CONTRACTS-87a18e12b6.md; merged into request_idempotency_contract | Merge instruction only; no independent Rust output | Covered by request idempotency tests |
| P0017 | CODEX-0696-07-API-REALTIME-CONTRACTS-8bf63a87bb | primary-implementation | crates/trpg-api/src/api.rs | API facade contract and formal write-flow guard | crates/trpg-api/tests/api_contract_tests.rs |
| P0018 | CODEX-0688-07-API-REALTIME-CONTRACTS-991d938d5b | primary-implementation | crates/trpg-api/src/api_web_socket_g_rpc_schema.rs | WebSocket/gRPC schema contract metadata and visibility provenance | crates/trpg-api/tests/api_web_socket_g_rpc_schema_contract_tests.rs |
| P0019 | CODEX-0693-07-API-REALTIME-CONTRACTS-b376214934 | supplemental-requirement | docs/codex/90-traceability/supplemental-requirements/CODEX-0693-07-API-REALTIME-CONTRACTS-b376214934.md; merged into websocket_protocol | Merge instruction only; no independent Rust output | Covered by websocket protocol tests |
| P0020 | CODEX-0690-07-API-REALTIME-CONTRACTS-b59ae9cd2b | supplemental-requirement | docs/codex/90-traceability/supplemental-requirements/CODEX-0690-07-API-REALTIME-CONTRACTS-b59ae9cd2b.md; merged into openapi_index | Merge instruction only; no independent Rust output | Covered by openapi index tests |
| P0021 | CODEX-0691-07-API-REALTIME-CONTRACTS-cc48fbd8a0 | supplemental-requirement | docs/codex/90-traceability/supplemental-requirements/CODEX-0691-07-API-REALTIME-CONTRACTS-cc48fbd8a0.md; merged into api_web_socket | Merge instruction only; no independent Rust output | Covered by api_web_socket tests |
| P0022 | CODEX-0692-07-API-REALTIME-CONTRACTS-e7b22d433c | supplemental-requirement | docs/codex/90-traceability/supplemental-requirements/CODEX-0692-07-API-REALTIME-CONTRACTS-e7b22d433c.md; merged into realtime_room_sync | Merge instruction only; no independent Rust output | Covered by realtime room sync tests |
| P0023 | CODEX-0695-07-API-REALTIME-CONTRACTS-f602cf5008 | primary-implementation | crates/trpg-api/src/provider.rs | Provider boundary contract; direct model and runtime adapter denial | crates/trpg-api/tests/provider_contract_tests.rs |
| P0024 | CODEX-0698-07-API-REALTIME-CONTRACTS-a5b1a48fc3 | primary-implementation | crates/trpg-api/src/openapi_contract.rs | OpenAPI contract facade and governance-required metadata | crates/trpg-api/tests/openapi_contract_contract_tests.rs |
| P0025 | CODEX-0699-07-API-REALTIME-CONTRACTS-8b3976529f | supplemental-requirement | docs/codex/90-traceability/supplemental-requirements/CODEX-0699-07-API-REALTIME-CONTRACTS-8b3976529f.md; merged into openapi_index | Merge instruction only; no independent Rust output | Covered by openapi index tests |
