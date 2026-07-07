# Supplemental Requirement Merge Instruction

Prompt ID: `CODEX-0693-07-API-REALTIME-CONTRACTS-b376214934`
Primary Prompt: `CODEX-0072-07-API-REALTIME-CONTRACTS-513ac60dc8`
Batch: `BATCH-029-07-api-realtime-contracts`
Target module: `api_realtime_contracts::websocket_protocol`
Source prompt: `codex-prompts/07-api-realtime-contracts/P0019.md`

## Boundary

This supplemental prompt is not an independent Rust source, test, migration, API handler, NATS subject, or formal state write owner.

## Merge Instruction

Merge these constraints into the primary WebSocket protocol contract work:

- Define protocol metadata as governed read-model delivery, not a state mutation path.
- Require sequence, visibility label, fact provenance reference, correlation ID, and causation ID on deltas.
- Enforce reconnect and replay behavior through Event Store derived data.
- Reject direct AI/provider calls and direct state writes from protocol handlers.
- Keep protocol names current-safe and free of historical version tokens.
