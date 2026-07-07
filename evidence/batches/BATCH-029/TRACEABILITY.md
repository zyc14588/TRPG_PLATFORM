# BATCH-029 Traceability

Batch: BATCH-029-07-api-realtime-contracts
Evidence date: 2026-07-07

## Current-safe Outputs

- Root workspace updated to include crates/trpg-api.
- crates/trpg-api/src/contract_core.rs contains the shared governance contract primitives.
- crates/trpg-api/src/lib.rs exports the B029 API realtime contract surface.
- crates/trpg-api/src/api.rs
- crates/trpg-api/src/api_and_transport.rs
- crates/trpg-api/src/api_contracts.rs
- crates/trpg-api/src/api_web_socket.rs
- crates/trpg-api/src/api_web_socket_g_rpc_schema.rs
- crates/trpg-api/src/external_provider_contracts.rs
- crates/trpg-api/src/nats_subject_contracts.rs
- crates/trpg-api/src/openapi.rs
- crates/trpg-api/src/openapi_contract.rs
- crates/trpg-api/src/openapi_index.rs
- crates/trpg-api/src/provider.rs
- crates/trpg-api/src/realtime_room_sync.rs
- crates/trpg-api/src/realtime_sync.rs
- crates/trpg-api/src/request_idempotency_contract.rs
- crates/trpg-api/src/websocket_protocol.rs
- crates/trpg-api/tests/s08_fixture_acceptance_contract_tests.rs
- docs/codex/07-api-realtime-contracts/m_07_api_realtime_contracts.md
- docs/codex/90-traceability/supplemental-requirements/CODEX-0683-07-API-REALTIME-CONTRACTS-80f5c71054.md
- docs/codex/90-traceability/supplemental-requirements/CODEX-0684-07-API-REALTIME-CONTRACTS-93d7eb8719.md
- docs/codex/90-traceability/supplemental-requirements/CODEX-0690-07-API-REALTIME-CONTRACTS-b59ae9cd2b.md
- docs/codex/90-traceability/supplemental-requirements/CODEX-0691-07-API-REALTIME-CONTRACTS-cc48fbd8a0.md
- docs/codex/90-traceability/supplemental-requirements/CODEX-0692-07-API-REALTIME-CONTRACTS-e7b22d433c.md
- docs/codex/90-traceability/supplemental-requirements/CODEX-0693-07-API-REALTIME-CONTRACTS-b376214934.md
- docs/codex/90-traceability/supplemental-requirements/CODEX-0694-07-API-REALTIME-CONTRACTS-87a18e12b6.md
- docs/codex/90-traceability/supplemental-requirements/CODEX-0697-07-API-REALTIME-CONTRACTS-57f9fff7f4.md
- docs/codex/90-traceability/supplemental-requirements/CODEX-0699-07-API-REALTIME-CONTRACTS-8b3976529f.md

## Governance Gates Implemented

| Gate | Implementation evidence | Test evidence |
| --- | --- | --- |
| Authority Contract is required and immutable after creation | Contract append helpers require AuthorityContract and use shared-kernel EventStore append semantics | batch_029_api_realtime_contract_tests.rs; per-module tests |
| Formal write path only | ApiCommandPayload requires CommandEnvelope, expected version, idempotency key, and uses EventStore append | request_idempotency_contract_contract_tests.rs |
| Realtime deltas are read-model output, not source of truth | RealtimeDelta is derived from EventRecord and filtered by Visibility | realtime_sync_contract_tests.rs; realtime_room_sync_contract_tests.rs |
| Visibility labels are preserved | EventRecord visibility is copied to deltas and projections | batch governance tests; websocket protocol tests |
| Fact Provenance is preserved | EventEnvelope preserves FactProvenance from CommandEnvelope while realtime deltas expose only the provenance reference | batch governance tests |
| Provider access cannot bypass Agent Gateway | evaluate_provider_access allows AgentGateway and denies AgentRuntimeAdapter or DirectModelProvider | provider_contract_tests.rs; external_provider_contracts_contract_tests.rs |
| NATS subjects use current-safe names | validate_nats_subject checks canonical current subjects only | nats_subject_contracts_contract_tests.rs |
| OpenAPI surfaces expose governance requirements | OpenApiContractDocument requires command, event, observability, and security fields | openapi_contract_tests.rs; openapi_index_contract_tests.rs |
| Axum/utoipa contract metadata is explicit | http_api_adapter_contract records the Axum handler and utoipa schema boundary without adding production adapter dependencies | batch_029_api_realtime_contract_tests.rs |
| WebSocket/NATS fixture contract is automated | realtime_adapter_contract and s08_expected_fixture_contract bind S08 expected WebSocket/NATS values to assertions | s08_fixture_acceptance_contract_tests.rs |
| SQLx or adapter boundary is covered | persistence_adapter_contract records the SQLx event-store transaction adapter boundary and State Service/Event Store write boundary | batch_029_api_realtime_contract_tests.rs |
| OpenFGA/OPA/Tool Gate is default-deny | tool_permission_gate_contract requires OpenFGA, OPA, and default-deny Tool Permission Gate checks | batch_029_api_realtime_contract_tests.rs |

## Supplemental Requirement Merge Map

| Supplemental prompt | Merged target |
| --- | --- |
| P0009 CODEX-0683-07-API-REALTIME-CONTRACTS-80f5c71054 | api_and_transport; supplemental file CODEX-0683-07-API-REALTIME-CONTRACTS-80f5c71054.md |
| P0010 CODEX-0684-07-API-REALTIME-CONTRACTS-93d7eb8719 | external_provider_contracts; supplemental file CODEX-0684-07-API-REALTIME-CONTRACTS-93d7eb8719.md |
| P0014 CODEX-0697-07-API-REALTIME-CONTRACTS-57f9fff7f4 | api_web_socket_g_rpc_schema; supplemental file CODEX-0697-07-API-REALTIME-CONTRACTS-57f9fff7f4.md |
| P0016 CODEX-0694-07-API-REALTIME-CONTRACTS-87a18e12b6 | request_idempotency_contract; supplemental file CODEX-0694-07-API-REALTIME-CONTRACTS-87a18e12b6.md |
| P0019 CODEX-0693-07-API-REALTIME-CONTRACTS-b376214934 | websocket_protocol; supplemental file CODEX-0693-07-API-REALTIME-CONTRACTS-b376214934.md |
| P0020 CODEX-0690-07-API-REALTIME-CONTRACTS-b59ae9cd2b | openapi_index; supplemental file CODEX-0690-07-API-REALTIME-CONTRACTS-b59ae9cd2b.md |
| P0021 CODEX-0691-07-API-REALTIME-CONTRACTS-cc48fbd8a0 | api_web_socket; supplemental file CODEX-0691-07-API-REALTIME-CONTRACTS-cc48fbd8a0.md |
| P0022 CODEX-0692-07-API-REALTIME-CONTRACTS-e7b22d433c | realtime_room_sync; supplemental file CODEX-0692-07-API-REALTIME-CONTRACTS-e7b22d433c.md |
| P0025 CODEX-0699-07-API-REALTIME-CONTRACTS-8b3976529f | openapi_index; supplemental file CODEX-0699-07-API-REALTIME-CONTRACTS-8b3976529f.md |

## Source-archive Boundary

No source-archive path was used as a current implementation name. The batch implementation uses the normalized current-safe module names from the authority maps and B029 prompt mapping.

## Direct LLM Boundary

The new trpg-api crate contains no direct OpenAI, Ollama, llama.cpp, chat completion, or responses API integration. Provider policy is represented as a contract decision and only the Agent Gateway path is allowed.
