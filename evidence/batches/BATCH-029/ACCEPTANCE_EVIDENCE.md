# BATCH-029 Acceptance Evidence

Batch: BATCH-029-07-api-realtime-contracts
Evidence date: 2026-07-07

## Strict Acceptance Status

Result: PASS for the repaired B029 scope P0001-P0025 only. No P0026+ prompts were executed.

| Acceptance item | Status | Evidence |
| --- | --- | --- |
| Required authority files read before implementation | PASS | WORK_PLAN.md authority input list |
| Every P0001-P0025 prompt has an acceptance conclusion | PASS | Per-prompt acceptance matrix below |
| Every primary prompt has implementation evidence and at least one target test | PASS | Matrix rows for P0001-P0004, P0006-P0008, P0011-P0013, P0015, P0017-P0018, P0023-P0024 |
| Documentation-only prompt has a non-code acceptance reason | PASS | P0005 row |
| Supplemental prompts use real B029/current-map IDs and do not own Rust outputs | PASS | P0009, P0010, P0014, P0016, P0019-P0022, P0025 rows; supplemental requirement files |
| Current-safe module/output naming applied | PASS | TRACEABILITY.md current-safe outputs; rust tests assert current-safe names |
| Authority Contract, Agent Gateway, Tool Gate, Visibility, Fact Provenance, Event Log, and V1 boundaries preserved | PASS | batch_029_api_realtime_contract_tests.rs; s08_fixture_acceptance_contract_tests.rs |
| No direct LLM/provider call path outside Agent Runtime / Provider Adapter | PASS | provider policy tests; direct-model text scan in TEST_RESULTS.md |
| No formal game state bypasses State Service / Event Store | PASS | EventStore append tests; DirectAgent write-path denial |
| No keeper_only/private_to_player/ai_internal fixture leakage to player-visible output | PASS | visibility replay tests and S08 fixture visibility assertions |
| Stage cargo, pnpm, docker, fixture checks addressed | PASS | TEST_RESULTS.md command table and N/A rationale |

## Per-Prompt Acceptance Matrix

| Prompt | Prompt ID | Role | Conclusion | Implementation or merge evidence | Target test or non-code reason |
| --- | --- | --- | --- | --- | --- |
| P0001 | CODEX-0066-07-API-REALTIME-CONTRACTS-831b0504c2 | primary | PASS | `crates/trpg-api/src/api_and_transport.rs`; shared `contract_core.rs` | `api_and_transport_contract_tests.rs`; batch governance tests |
| P0002 | CODEX-0067-07-API-REALTIME-CONTRACTS-1ccbeea1df | primary | PASS | `crates/trpg-api/src/external_provider_contracts.rs`; provider boundary helpers | `external_provider_contracts_contract_tests.rs`; provider gateway tests |
| P0003 | CODEX-0068-07-API-REALTIME-CONTRACTS-2b78603401 | primary | PASS | `crates/trpg-api/src/nats_subject_contracts.rs`; canonical/domain subject validation | `nats_subject_contracts_contract_tests.rs`; S08 fixture NATS assertions |
| P0004 | CODEX-0069-07-API-REALTIME-CONTRACTS-3cc61a7d01 | primary | PASS | `crates/trpg-api/src/openapi_index.rs`; OpenAPI metadata contract | `openapi_index_contract_tests.rs`; OpenAPI metadata assertions |
| P0005 | CODEX-0065-07-API-REALTIME-CONTRACTS-4a235df615 | documentation-or-traceability | PASS | `docs/codex/07-api-realtime-contracts/m_07_api_realtime_contracts.md` | Non-code docs-governance output; verified by evidence review and scans |
| P0006 | CODEX-0070-07-API-REALTIME-CONTRACTS-40bb6959f3 | primary | PASS | `crates/trpg-api/src/realtime_sync.rs`; replay/visibility helpers | `realtime_sync_contract_tests.rs`; batch visibility replay tests |
| P0007 | CODEX-0071-07-API-REALTIME-CONTRACTS-3277264d0e | primary | PASS | `crates/trpg-api/src/request_idempotency_contract.rs`; idempotency EventStore guard | `request_idempotency_contract_contract_tests.rs`; duplicate/missing idempotency tests |
| P0008 | CODEX-0072-07-API-REALTIME-CONTRACTS-513ac60dc8 | primary | PASS | `crates/trpg-api/src/websocket_protocol.rs`; realtime delta contract | `websocket_protocol_contract_tests.rs`; S08 private replay test |
| P0009 | CODEX-0683-07-API-REALTIME-CONTRACTS-80f5c71054 | supplemental | PASS | `docs/codex/90-traceability/supplemental-requirements/CODEX-0683-07-API-REALTIME-CONTRACTS-80f5c71054.md`; merged to P0001 | No independent Rust output; covered by P0001 tests |
| P0010 | CODEX-0684-07-API-REALTIME-CONTRACTS-93d7eb8719 | supplemental | PASS | `docs/codex/90-traceability/supplemental-requirements/CODEX-0684-07-API-REALTIME-CONTRACTS-93d7eb8719.md`; merged to P0002 | No independent Rust output; covered by P0002/provider tests |
| P0011 | CODEX-0687-07-API-REALTIME-CONTRACTS-1d88035bc8 | primary | PASS | `crates/trpg-api/src/api_contracts.rs`; aggregate formal write-flow metadata | `api_contracts_contract_tests.rs` |
| P0012 | CODEX-0689-07-API-REALTIME-CONTRACTS-4b17a0fb09 | primary | PASS | `crates/trpg-api/src/openapi.rs`; OpenAPI contract facade | `openapi_contract_tests.rs` |
| P0013 | CODEX-0686-07-API-REALTIME-CONTRACTS-54d06d623d | primary | PASS | `crates/trpg-api/src/realtime_room_sync.rs`; room replay metadata | `realtime_room_sync_contract_tests.rs`; S08 room fixture assertions |
| P0014 | CODEX-0697-07-API-REALTIME-CONTRACTS-57f9fff7f4 | supplemental | PASS | `docs/codex/90-traceability/supplemental-requirements/CODEX-0697-07-API-REALTIME-CONTRACTS-57f9fff7f4.md`; merged to P0018 | No independent Rust output; covered by P0018 tests |
| P0015 | CODEX-0685-07-API-REALTIME-CONTRACTS-5d2e1fa760 | primary | PASS | `crates/trpg-api/src/api_web_socket.rs`; WebSocket API metadata | `api_web_socket_contract_tests.rs`; S08 fixture assertions |
| P0016 | CODEX-0694-07-API-REALTIME-CONTRACTS-87a18e12b6 | supplemental | PASS | `docs/codex/90-traceability/supplemental-requirements/CODEX-0694-07-API-REALTIME-CONTRACTS-87a18e12b6.md`; merged to P0007 | No independent Rust output; covered by P0007 tests |
| P0017 | CODEX-0696-07-API-REALTIME-CONTRACTS-8bf63a87bb | primary | PASS | `crates/trpg-api/src/api.rs`; API facade contract | `api_contract_tests.rs` |
| P0018 | CODEX-0688-07-API-REALTIME-CONTRACTS-991d938d5b | primary | PASS | `crates/trpg-api/src/api_web_socket_g_rpc_schema.rs`; WebSocket/gRPC schema contract | `api_web_socket_g_rpc_schema_contract_tests.rs` |
| P0019 | CODEX-0693-07-API-REALTIME-CONTRACTS-b376214934 | supplemental | PASS | `docs/codex/90-traceability/supplemental-requirements/CODEX-0693-07-API-REALTIME-CONTRACTS-b376214934.md`; merged to P0008 | No independent Rust output; covered by P0008 tests |
| P0020 | CODEX-0690-07-API-REALTIME-CONTRACTS-b59ae9cd2b | supplemental | PASS | `docs/codex/90-traceability/supplemental-requirements/CODEX-0690-07-API-REALTIME-CONTRACTS-b59ae9cd2b.md`; merged to P0004 | No independent Rust output; covered by P0004/OpenAPI tests |
| P0021 | CODEX-0691-07-API-REALTIME-CONTRACTS-cc48fbd8a0 | supplemental | PASS | `docs/codex/90-traceability/supplemental-requirements/CODEX-0691-07-API-REALTIME-CONTRACTS-cc48fbd8a0.md`; merged to P0015 | No independent Rust output; covered by P0015/S08 tests |
| P0022 | CODEX-0692-07-API-REALTIME-CONTRACTS-e7b22d433c | supplemental | PASS | `docs/codex/90-traceability/supplemental-requirements/CODEX-0692-07-API-REALTIME-CONTRACTS-e7b22d433c.md`; merged to P0013 | No independent Rust output; covered by P0013/S08 tests |
| P0023 | CODEX-0695-07-API-REALTIME-CONTRACTS-f602cf5008 | primary | PASS | `crates/trpg-api/src/provider.rs`; Agent Gateway-only provider decision | `provider_contract_tests.rs`; direct provider denial tests |
| P0024 | CODEX-0698-07-API-REALTIME-CONTRACTS-a5b1a48fc3 | primary | PASS | `crates/trpg-api/src/openapi_contract.rs`; OpenAPI contract metadata | `openapi_contract_contract_tests.rs`; batch OpenAPI tests |
| P0025 | CODEX-0699-07-API-REALTIME-CONTRACTS-8b3976529f | supplemental | PASS | `docs/codex/90-traceability/supplemental-requirements/CODEX-0699-07-API-REALTIME-CONTRACTS-8b3976529f.md`; merged to P0004 | No independent Rust output; covered by P0004/OpenAPI tests |

## Files Changed For Repair

- `crates/trpg-api/src/contract_core.rs`
- `crates/trpg-api/tests/batch_029_api_realtime_contract_tests.rs`
- `crates/trpg-api/tests/s08_fixture_acceptance_contract_tests.rs`
- `fixtures/stages/detailed/S08_api_ws_nats_expected.current.json.md`
- `docs/codex/07-api-realtime-contracts/m_07_api_realtime_contracts.md`
- `docs/codex/90-traceability/supplemental-requirements/*.md`
- `evidence/batches/BATCH-029/WORK_PLAN.md`
- `evidence/batches/BATCH-029/TRACEABILITY.md`
- `evidence/batches/BATCH-029/TEST_RESULTS.md`
- `evidence/batches/BATCH-029/ACCEPTANCE_EVIDENCE.md`

## Non-Code Boundary Rationale

B029 records Axum, utoipa/OpenAPI, WebSocket/NATS, SQLx adapter boundary, OpenFGA/OPA, and Tool Permission Gate as compile-tested contract metadata because the current workspace does not contain production adapter layers for these systems. Adding new production servers, clients, or migrations would expand beyond the strict B029 repair. The implemented contract helpers and tests make the future adapter boundary explicit while preserving Event Store, visibility, provenance, and Agent Gateway constraints.
