# Supplemental Requirement Merge Instruction

Prompt ID: `CODEX-0716-07-API-REALTIME-CONTRACTS-1dcbfff315`
Primary Prompt: `CODEX-0066-07-API-REALTIME-CONTRACTS-831b0504c2`
Batch: `BATCH-030-07-api-realtime-contracts`
Target module: `api_realtime_contracts::api_and_transport`
Source prompt: `codex-prompts/07-api-realtime-contracts/P0042.md`

## Boundary

This supplemental prompt is not an independent Rust source, test, migration, API handler, NATS subject, metric, workflow, or formal state write owner.

## Merge Instruction

- Merge transport contract requirements for command envelopes, request headers, response metadata, traceability, and visibility-safe error surfaces into the current-safe API and transport module.
- Every formal command transport path must carry actor, authority mode, campaign, idempotency, expected version, visibility label, fact provenance, correlation id, and causation id metadata as applicable.
- Transport handlers must not direct-call providers, fabricate dice, mutate Authority Contract, or write canonical state without the formal workflow.
- Error responses must be auditable and must not reveal restricted content across visibility boundaries.
- Realtime and HTTP contract metadata must remain aligned where they represent the same formal command or projection read.

## Test Responsibility

Coverage remains with `api_and_transport_contract_tests`, `s08_fixture_acceptance_contract_tests`, and the S08 stage command for `trpg-api`.
