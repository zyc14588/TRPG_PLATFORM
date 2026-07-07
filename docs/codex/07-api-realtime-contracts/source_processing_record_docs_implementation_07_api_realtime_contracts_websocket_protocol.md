# Source Processing Record - WebSocket Protocol

Prompt ID: `CODEX-0712-07-API-REALTIME-CONTRACTS-acc346fefe`
Batch: `BATCH-030-07-api-realtime-contracts`
Source prompt: `codex-prompts/07-api-realtime-contracts/P0036.md`
Output role: `documentation-or-traceability`
Current-safe module: `api_realtime_contracts::websocket_protocol`
Current output: `docs/codex/07-api-realtime-contracts/source_processing_record_docs_implementation_07_api_realtime_contracts_websocket_protocol.md`

## Boundary

This record is traceability-only. It does not create Rust source, tests, migrations, API handlers, NATS subjects, metrics, workflows, or formal state write paths.

## Current Disposition

- Retains WebSocket protocol requirements for command acknowledgement, subscription state, reconnect replay, heartbeat, filtered deltas, and visibility-safe errors.
- Implementation responsibility remains with the current-safe WebSocket protocol module and tests.
- WebSocket remains a transport boundary and not a formal state writer.

## Required Gates

- Protocol messages preserve correlation, causation, provenance, visibility, actor scope, and sequence metadata as applicable.
- Hidden Keeper material and actor-private facts must not be serialized to unauthorized principals.
- Formal decisions submitted over WebSocket still flow through tool, rules, state, and event-log gates.

## Test Responsibility

Covered by `websocket_protocol_contract_tests`, `s08_fixture_acceptance_contract_tests`, and the S08 stage command for `trpg-api`.
