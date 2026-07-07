# Supplemental Requirement Merge Instruction

Prompt ID: `CODEX-0704-07-API-REALTIME-CONTRACTS-113ebfe8d3`
Primary Prompt: `CODEX-0686-07-API-REALTIME-CONTRACTS-54d06d623d`
Batch: `BATCH-030-07-api-realtime-contracts`
Target module: `api_realtime_contracts::realtime_room_sync`
Source prompt: `codex-prompts/07-api-realtime-contracts/P0030.md`

## Boundary

This supplemental prompt is not an independent Rust source, test, migration, API handler, NATS subject, metric, workflow, or formal state write owner.

## Merge Instruction

- Merge room, scene, party split, spectator, and reconnect synchronization requirements into the current-safe realtime room sync contract.
- Room fanout must be derived from canonical event order and visibility-filtered projections, not from mutable transport-local state.
- Cross-room and split-party delivery must preserve isolation between private scenes, Keeper-only facts, player-private state, and public facts.
- Reconnect replay must respect the reconnecting principal's visibility labels at replay time and must keep fact provenance attached.
- No room sync path may modify Authority Contract or write formal state outside the command workflow.

## Test Responsibility

Coverage remains with realtime room sync contract coverage merged under the existing S08 `trpg-api` realtime tests and fixture acceptance checks.
