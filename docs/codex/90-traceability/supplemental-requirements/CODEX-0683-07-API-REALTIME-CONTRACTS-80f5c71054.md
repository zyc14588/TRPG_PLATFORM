# Supplemental Requirement Merge Instruction

Prompt ID: `CODEX-0683-07-API-REALTIME-CONTRACTS-80f5c71054`
Primary Prompt: `CODEX-0066-07-API-REALTIME-CONTRACTS-831b0504c2`
Batch: `BATCH-029-07-api-realtime-contracts`
Target module: `api_realtime_contracts::api_and_transport`
Source prompt: `codex-prompts/07-api-realtime-contracts/P0009.md`

## Boundary

This supplemental prompt is not an independent Rust source, test, migration, API handler, NATS subject, or formal state write owner.

## Merge Instruction

Merge these constraints into the primary API and transport contract work:

- Keep transport writes on the formal `Command -> Workflow -> Decision -> Event Store -> Projection` path.
- Require idempotency, expected version, actor, visibility, fact provenance, correlation ID, causation ID, and Authority Contract version on governed command input.
- Preserve Visibility Label and Fact Provenance in every event, realtime delta, audit record, replay path, and projection.
- Reject direct agent state writes and any provider or LLM path outside Agent Gateway governance.
- Keep all module, event, NATS, metric, and test names current-safe.
