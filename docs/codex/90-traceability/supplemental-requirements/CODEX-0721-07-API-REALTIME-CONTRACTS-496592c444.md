# Supplemental Requirement Merge Instruction

Prompt ID: `CODEX-0721-07-API-REALTIME-CONTRACTS-496592c444`
Primary Prompt: `CODEX-0071-07-API-REALTIME-CONTRACTS-3277264d0e`
Batch: `BATCH-030-07-api-realtime-contracts`
Target module: `api_realtime_contracts::request_idempotency_contract`
Source prompt: `codex-prompts/07-api-realtime-contracts/P0044.md`

## Boundary

This supplemental prompt is not an independent Rust source, test, migration, API handler, NATS subject, metric, workflow, or formal state write owner.

## Merge Instruction

- Merge additional idempotency requirements for repeated transport submissions, retry storms, reconnect replay, and outbox publishing.
- A duplicate idempotency key must not create duplicate canonical events, duplicate dice results, or duplicate workflow decisions.
- Expected-version conflict responses must preserve visibility-safe diagnostics and actor-specific auditability.
- Idempotency records must not become the source of truth for story state; Event Store remains canonical.
- API and WebSocket submissions must use aligned idempotency semantics.

## Test Responsibility

Coverage remains with `request_idempotency_contract_contract_tests`, `s08_fixture_acceptance_contract_tests`, and the S08 stage command for `trpg-api`.
