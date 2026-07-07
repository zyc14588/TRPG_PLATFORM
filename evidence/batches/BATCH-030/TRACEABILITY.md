# BATCH-030 Traceability

Batch: `BATCH-030-07-api-realtime-contracts`
Stage: `S08 - 07-api-realtime-contracts`

## Current-safe Output Set

- Primary implementation: `api_realtime_contracts::readme`.
- Primary Rust file: `crates/trpg-api/src/readme.rs`.
- Primary target test: `crates/trpg-api/tests/readme_contract_tests.rs`.
- Supplemental merge instructions: `docs/codex/90-traceability/supplemental-requirements/CODEX-0701...md` through `CODEX-0722...md` for B030 supplemental prompts.
- Source processing records: `docs/codex/07-api-realtime-contracts/source_processing_record_*.md`.
- Batch evidence: `evidence/batches/BATCH-030/`.

## Source Boundary

- `source-archive/**` remains provenance-only.
- Source provenance identifiers are not used as current Rust module, migration, event, NATS subject, metric, workflow, test, or output names.
- B030 outputs use normalized current-safe module names from the 00-index maps.

## Governance Boundary

- Business/API/realtime code does not direct-call OpenAI, Ollama, llama.cpp, or bare model endpoints.
- Provider access remains `Agent Gateway -> Agent Orchestrator/Runtime -> Model Provider Adapter`.
- Agents do not directly write canonical state, fabricate dice, bypass rules/state/event-log gates, or mutate Authority Contract.
- Formal state remains `Command -> Workflow -> Decision -> Event Store -> Projection`.
- Visibility labels and fact provenance remain required across API, realtime, tool result, event, projection, RAG, summary, export, replay, log, and metric surfaces.

## Merge Targets

| Prompt rows | Current-safe owning module | Test responsibility |
|---|---|---|
| `CODEX-0700`, `CODEX-0719` | `api_realtime_contracts::readme` | `readme_contract_tests` |
| `CODEX-0701`, `CODEX-0720` | `api_realtime_contracts::realtime_sync` | `realtime_sync_contract_tests`, S08 fixture tests |
| `CODEX-0702`, `CODEX-0721` | `api_realtime_contracts::request_idempotency_contract` | `request_idempotency_contract_contract_tests`, S08 fixture tests |
| `CODEX-0703`, `CODEX-0722` | `api_realtime_contracts::websocket_protocol` | `websocket_protocol_contract_tests`, S08 fixture tests |
| `CODEX-0704` | `api_realtime_contracts::realtime_room_sync` | Realtime package and S08 fixture tests |
| `CODEX-0715` | `api_realtime_contracts::openapi` | `openapi_contract_tests`, `openapi_index_contract_tests`, S08 fixture tests |
| `CODEX-0716` | `api_realtime_contracts::api_and_transport` | `api_and_transport_contract_tests`, S08 fixture tests |
| `CODEX-0717` | `api_realtime_contracts::external_provider_contracts` | `external_provider_contracts_contract_tests`, S08 fixture tests |
| `CODEX-0718` | `api_realtime_contracts::openapi_index` | `openapi_index_contract_tests`, S08 fixture tests |

## Scope Resolution

This repair resolves the prior `CODEX-0700` evidence gap by implementing only the current-safe readme primary and merging `CODEX-0719` into the same target. No additional primary prompt was introduced.
