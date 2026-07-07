# Supplemental Requirement Merge Instruction

Prompt ID: `CODEX-0692-07-API-REALTIME-CONTRACTS-e7b22d433c`
Primary Prompt: `CODEX-0686-07-API-REALTIME-CONTRACTS-54d06d623d`
Batch: `BATCH-029-07-api-realtime-contracts`
Target module: `api_realtime_contracts::realtime_room_sync`
Source prompt: `codex-prompts/07-api-realtime-contracts/P0022.md`

## Boundary

This supplemental prompt is not an independent Rust source, test, migration, API handler, NATS subject, or formal state write owner.

## Merge Instruction

Merge these constraints into the primary realtime room sync contract work:

- Room sync must be a read-model delivery contract over ordered Event Store events.
- Reconnect and multi-room behavior must preserve sequence, visibility, fact provenance, and correlation metadata.
- Do not allow player-visible output to include keeper-only, private-to-player for another player, or AI-internal content.
- NATS and WebSocket subjects must be current-safe and reject wildcard subject contracts.
- Keep formal game state writes on State Service and Event Store boundaries.
