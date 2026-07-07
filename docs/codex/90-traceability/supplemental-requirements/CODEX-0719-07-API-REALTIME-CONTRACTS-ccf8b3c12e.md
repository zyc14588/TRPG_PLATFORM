# Supplemental Requirement Merge Instruction

Prompt ID: `CODEX-0719-07-API-REALTIME-CONTRACTS-ccf8b3c12e`
Primary Prompt: `CODEX-0700-07-API-REALTIME-CONTRACTS-32445eadff`
Batch: `BATCH-030-07-api-realtime-contracts`
Target module: `api_realtime_contracts::readme`
Source prompt: `codex-prompts/07-api-realtime-contracts/P0046.md`

## Boundary

This supplemental prompt is not an independent Rust source, test, migration, API handler, NATS subject, metric, workflow, or formal state write owner.

## Merge Instruction

- Merge README governance requirements into owning primary prompt `CODEX-0700-07-API-REALTIME-CONTRACTS-32445eadff`.
- The owning primary is in scope for B030 repair and implements the current-safe `api_realtime_contracts::readme` module boundary.
- This supplemental prompt does not independently own Rust source, tests, migrations, API handlers, NATS subjects, metrics, workflows, or formal state write paths.
- The merged primary retains API, realtime, Agent Gateway, visibility, fact provenance, idempotency, and Event Store authority constraints.

## Test Responsibility

No independent test is introduced by this supplemental prompt. Merged coverage is provided by the owning primary readme contract tests.
