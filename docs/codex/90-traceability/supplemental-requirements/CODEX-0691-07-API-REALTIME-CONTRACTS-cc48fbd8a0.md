# Supplemental Requirement Merge Instruction

Prompt ID: `CODEX-0691-07-API-REALTIME-CONTRACTS-cc48fbd8a0`
Primary Prompt: `CODEX-0685-07-API-REALTIME-CONTRACTS-5d2e1fa760`
Batch: `BATCH-029-07-api-realtime-contracts`
Target module: `api_realtime_contracts::api_web_socket`
Source prompt: `codex-prompts/07-api-realtime-contracts/P0021.md`

## Boundary

This supplemental prompt is not an independent Rust source, test, migration, API handler, NATS subject, or formal state write owner.

## Merge Instruction

Merge these constraints into the primary API WebSocket contract work:

- WebSocket output must be derived from Event Store replay or projection data, never a formal state write bypass.
- Filter every delta through Visibility Label before player-visible delivery.
- Preserve fact provenance, sequence, correlation ID, and causation ID across reconnect and replay.
- Support room-scoped delivery without leaking keeper-only, private-to-player, or AI-internal content.
- Keep all WebSocket message, route, event, and test names current-safe.
