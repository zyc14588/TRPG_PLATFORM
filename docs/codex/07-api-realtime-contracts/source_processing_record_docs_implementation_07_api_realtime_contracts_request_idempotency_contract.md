# Source Processing Record - Request Idempotency Contract

Prompt ID: `CODEX-0710-07-API-REALTIME-CONTRACTS-fbb1ebb04f`
Batch: `BATCH-030-07-api-realtime-contracts`
Source prompt: `codex-prompts/07-api-realtime-contracts/P0040.md`
Output role: `documentation-or-traceability`
Current-safe module: `api_realtime_contracts::request_idempotency_contract`
Current output: `docs/codex/07-api-realtime-contracts/source_processing_record_docs_implementation_07_api_realtime_contracts_request_idempotency_contract.md`

## Boundary

This record is traceability-only. It does not create Rust source, tests, migrations, API handlers, NATS subjects, metrics, workflows, or formal state write paths.

## Current Disposition

- Retains idempotency requirements for retries, reconnect submissions, expected-version conflicts, and duplicate suppression.
- Implementation responsibility remains with the current-safe request idempotency contract module and tests.
- Idempotency records do not replace Event Store as canonical story state.

## Required Gates

- Duplicate submissions must not duplicate events, dice, decisions, or outbox effects.
- Replayed responses must preserve actor-specific visibility and audit metadata.
- API and WebSocket command submission semantics stay aligned.

## Test Responsibility

Covered by `request_idempotency_contract_contract_tests`, `s08_fixture_acceptance_contract_tests`, and the S08 stage command for `trpg-api`.
