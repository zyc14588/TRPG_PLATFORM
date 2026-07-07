# Supplemental Requirement Merge Instruction

Prompt ID: `CODEX-0722-07-API-REALTIME-CONTRACTS-258f722334`
Primary Prompt: `CODEX-0072-07-API-REALTIME-CONTRACTS-513ac60dc8`
Batch: `BATCH-030-07-api-realtime-contracts`
Target module: `api_realtime_contracts::websocket_protocol`
Source prompt: `codex-prompts/07-api-realtime-contracts/P0043.md`

## Boundary

This supplemental prompt is not an independent Rust source, test, migration, API handler, NATS subject, metric, workflow, or formal state write owner.

## Merge Instruction

- Merge additional WebSocket contract requirements for command submission acknowledgements, subscription state, reconnect replay, and projection delta delivery.
- Protocol messages must distinguish command acknowledgement, workflow decision, event delta, projection update, error, heartbeat, and replay completion surfaces.
- Each message type must preserve correlation, causation, visibility, fact provenance, sequence, and actor-scope metadata as applicable.
- Hidden Keeper material and actor-private facts must not be serialized to unauthorized principals during live delivery or replay.
- WebSocket protocol remains transport governance and must not bypass command workflow or provider gateway boundaries.

## Test Responsibility

Coverage remains with `websocket_protocol_contract_tests`, `s08_fixture_acceptance_contract_tests`, and the S08 stage command for `trpg-api`.
