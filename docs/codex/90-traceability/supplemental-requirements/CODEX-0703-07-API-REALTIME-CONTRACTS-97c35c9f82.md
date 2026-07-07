# Supplemental Requirement Merge Instruction

Prompt ID: `CODEX-0703-07-API-REALTIME-CONTRACTS-97c35c9f82`
Primary Prompt: `CODEX-0072-07-API-REALTIME-CONTRACTS-513ac60dc8`
Batch: `BATCH-030-07-api-realtime-contracts`
Target module: `api_realtime_contracts::websocket_protocol`
Source prompt: `codex-prompts/07-api-realtime-contracts/P0029.md`

## Boundary

This supplemental prompt is not an independent Rust source, test, migration, API handler, NATS subject, metric, workflow, or formal state write owner.

## Merge Instruction

- Merge WebSocket reconnect, heartbeat, replay cursor, room membership, and filtered event delivery requirements into the existing current-safe protocol contract.
- Require each emitted protocol message to carry enough correlation, provenance, visibility, and sequence metadata for replay and audit.
- Enforce principal-specific filtering before serialization; restricted facts, Keeper-only material, and private actor state must not leak through shared channels.
- WebSocket handlers must remain transport surfaces only and must not perform direct LLM calls, direct database writes, dice fabrication, or Authority Contract mutation.
- Formal decisions received over WebSocket must still pass through tool, rules, state, and event-log gates.

## Test Responsibility

Coverage remains with `websocket_protocol_contract_tests`, `s08_fixture_acceptance_contract_tests`, and the S08 stage command for `trpg-api`.
