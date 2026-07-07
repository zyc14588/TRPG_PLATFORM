# Supplemental Requirement Merge Instruction

Prompt ID: `CODEX-0694-07-API-REALTIME-CONTRACTS-87a18e12b6`
Primary Prompt: `CODEX-0071-07-API-REALTIME-CONTRACTS-3277264d0e`
Batch: `BATCH-029-07-api-realtime-contracts`
Target module: `api_realtime_contracts::request_idempotency_contract`
Source prompt: `codex-prompts/07-api-realtime-contracts/P0016.md`

## Boundary

This supplemental prompt is not an independent Rust source, test, migration, API handler, NATS subject, or formal state write owner.

## Merge Instruction

Merge these constraints into the primary request idempotency contract work:

- Require idempotency key and expected version before any formal event append.
- Reject duplicate commands without appending additional Event Store records.
- Preserve actor, Authority Contract version, visibility, fact provenance, correlation ID, and causation ID across idempotent handling.
- Treat idempotency as part of the command workflow boundary, not a projection-only concern.
- Keep all error, metric, and test names current-safe.
