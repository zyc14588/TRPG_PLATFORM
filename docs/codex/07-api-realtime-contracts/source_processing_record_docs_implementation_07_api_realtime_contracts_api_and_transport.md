# Source Processing Record - API And Transport

Prompt ID: `CODEX-0707-07-API-REALTIME-CONTRACTS-a266aa4068`
Batch: `BATCH-030-07-api-realtime-contracts`
Source prompt: `codex-prompts/07-api-realtime-contracts/P0035.md`
Output role: `documentation-or-traceability`
Current-safe module: `api_realtime_contracts::api_and_transport`
Current output: `docs/codex/07-api-realtime-contracts/source_processing_record_docs_implementation_07_api_realtime_contracts_api_and_transport.md`

## Boundary

This record is traceability-only. It does not create Rust source, tests, migrations, API handlers, NATS subjects, metrics, workflows, or formal state write paths.

## Current Disposition

- Retains transport envelope requirements for actor identity, authority mode, campaign, idempotency, expected version, visibility label, fact provenance, correlation id, and causation id.
- Implementation responsibility remains with the current-safe API and transport module and tests.
- Transport surfaces remain separate from formal state mutation.

## Required Gates

- API handlers must not direct-call model providers, fabricate dice, mutate Authority Contract, or write canonical state outside the formal workflow.
- Error surfaces must be visibility-safe.
- HTTP and realtime metadata stay aligned where they represent the same formal operation.

## Test Responsibility

Covered by `api_and_transport_contract_tests`, `s08_fixture_acceptance_contract_tests`, and the S08 stage command for `trpg-api`.
