# B029 API Realtime Contracts Governance Map

## Scope

This document is the docs-governance output for `CODEX-0065-07-API-REALTIME-CONTRACTS-4a235df615` in `BATCH-029-07-api-realtime-contracts`.

Current execution is limited to the first 25 prompts in `batches/B029.md`. Prompts after `P0025` belong to later batch scope and are not executed here.

## Current-safe implementation map

| Prompt ID | Role | Current-safe output |
|---|---|---|
| CODEX-0066-07-API-REALTIME-CONTRACTS-831b0504c2 | primary | `crates/trpg-api/src/api_and_transport.rs` |
| CODEX-0067-07-API-REALTIME-CONTRACTS-1ccbeea1df | primary | `crates/trpg-api/src/external_provider_contracts.rs` |
| CODEX-0068-07-API-REALTIME-CONTRACTS-2b78603401 | primary | `crates/trpg-api/src/nats_subject_contracts.rs` |
| CODEX-0069-07-API-REALTIME-CONTRACTS-3cc61a7d01 | primary | `crates/trpg-api/src/openapi_index.rs` |
| CODEX-0070-07-API-REALTIME-CONTRACTS-40bb6959f3 | primary | `crates/trpg-api/src/realtime_sync.rs` |
| CODEX-0071-07-API-REALTIME-CONTRACTS-3277264d0e | primary | `crates/trpg-api/src/request_idempotency_contract.rs` |
| CODEX-0072-07-API-REALTIME-CONTRACTS-513ac60dc8 | primary | `crates/trpg-api/src/websocket_protocol.rs` |
| CODEX-0685-07-API-REALTIME-CONTRACTS-5d2e1fa760 | primary | `crates/trpg-api/src/api_web_socket.rs` |
| CODEX-0686-07-API-REALTIME-CONTRACTS-54d06d623d | primary | `crates/trpg-api/src/realtime_room_sync.rs` |
| CODEX-0687-07-API-REALTIME-CONTRACTS-1d88035bc8 | primary | `crates/trpg-api/src/api_contracts.rs` |
| CODEX-0688-07-API-REALTIME-CONTRACTS-991d938d5b | primary | `crates/trpg-api/src/api_web_socket_g_rpc_schema.rs` |
| CODEX-0689-07-API-REALTIME-CONTRACTS-4b17a0fb09 | primary | `crates/trpg-api/src/openapi.rs` |
| CODEX-0695-07-API-REALTIME-CONTRACTS-f602cf5008 | primary | `crates/trpg-api/src/provider.rs` |
| CODEX-0696-07-API-REALTIME-CONTRACTS-8bf63a87bb | primary | `crates/trpg-api/src/api.rs` |
| CODEX-0698-07-API-REALTIME-CONTRACTS-a5b1a48fc3 | primary | `crates/trpg-api/src/openapi_contract.rs` |

## Supplemental merge map

| Prompt ID | Role | Merge target | Evidence |
|---|---|---|---|
| CODEX-0683-07-API-REALTIME-CONTRACTS-80f5c71054 | supplemental | CODEX-0066 / `api_and_transport` | `docs/codex/90-traceability/supplemental-requirements/CODEX-0683-07-API-REALTIME-CONTRACTS-80f5c71054.md` |
| CODEX-0684-07-API-REALTIME-CONTRACTS-93d7eb8719 | supplemental | CODEX-0067 / `external_provider_contracts` | `docs/codex/90-traceability/supplemental-requirements/CODEX-0684-07-API-REALTIME-CONTRACTS-93d7eb8719.md` |
| CODEX-0690-07-API-REALTIME-CONTRACTS-b59ae9cd2b | supplemental | CODEX-0069 / `openapi_index` | `docs/codex/90-traceability/supplemental-requirements/CODEX-0690-07-API-REALTIME-CONTRACTS-b59ae9cd2b.md` |
| CODEX-0691-07-API-REALTIME-CONTRACTS-cc48fbd8a0 | supplemental | CODEX-0685 / `api_web_socket` | `docs/codex/90-traceability/supplemental-requirements/CODEX-0691-07-API-REALTIME-CONTRACTS-cc48fbd8a0.md` |
| CODEX-0692-07-API-REALTIME-CONTRACTS-e7b22d433c | supplemental | CODEX-0686 / `realtime_room_sync` | `docs/codex/90-traceability/supplemental-requirements/CODEX-0692-07-API-REALTIME-CONTRACTS-e7b22d433c.md` |
| CODEX-0693-07-API-REALTIME-CONTRACTS-b376214934 | supplemental | CODEX-0072 / `websocket_protocol` | `docs/codex/90-traceability/supplemental-requirements/CODEX-0693-07-API-REALTIME-CONTRACTS-b376214934.md` |
| CODEX-0694-07-API-REALTIME-CONTRACTS-87a18e12b6 | supplemental | CODEX-0071 / `request_idempotency_contract` | `docs/codex/90-traceability/supplemental-requirements/CODEX-0694-07-API-REALTIME-CONTRACTS-87a18e12b6.md` |
| CODEX-0697-07-API-REALTIME-CONTRACTS-57f9fff7f4 | supplemental | CODEX-0688 / `api_web_socket_g_rpc_schema` | `docs/codex/90-traceability/supplemental-requirements/CODEX-0697-07-API-REALTIME-CONTRACTS-57f9fff7f4.md` |
| CODEX-0699-07-API-REALTIME-CONTRACTS-8b3976529f | supplemental | CODEX-0069 / `openapi_index` | `docs/codex/90-traceability/supplemental-requirements/CODEX-0699-07-API-REALTIME-CONTRACTS-8b3976529f.md` |

## Governance gates

- API write paths accept governed `CommandEnvelope` values and validate `idempotency_key`, `expected_version`, actor, authority mode, visibility, fact provenance, correlation ID, causation ID, and formal write path.
- Formal API/realtime facts append through `EventStore`; projections and realtime deltas are replayable read models.
- Realtime sync uses visibility filtering before a delta is exposed to a principal.
- Provider access from the API contract layer is limited to the Agent Gateway boundary.
- NATS subjects, event schema names, metric labels, Rust module names, and test names use current-safe names, not source paths, historical version tokens, or hash fragments.
- `contract_core::http_api_adapter_contract` records the Axum handler and utoipa OpenAPI schema boundary as testable metadata without adding a production HTTP stack to this batch.
- `contract_core::realtime_adapter_contract` records the WebSocket room endpoint, NATS subjects, replay support, and visibility filtering.
- `contract_core::persistence_adapter_contract` records the SQLx Event Store transaction adapter boundary and the State Service/Event Store formal write boundary.
- `contract_core::tool_permission_gate_contract` records OpenFGA, OPA, and default-deny Tool Permission Gate requirements.
- `crates/trpg-api/tests/s08_fixture_acceptance_contract_tests.rs` converts the S08 detailed fixture into automated assertions and replaces the previous invalid cargo automation target.
