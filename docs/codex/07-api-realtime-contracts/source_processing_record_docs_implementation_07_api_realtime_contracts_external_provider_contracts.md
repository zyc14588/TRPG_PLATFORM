# Source Processing Record - External Provider Contracts

Prompt ID: `CODEX-0711-07-API-REALTIME-CONTRACTS-badfeb9f49`
Batch: `BATCH-030-07-api-realtime-contracts`
Source prompt: `codex-prompts/07-api-realtime-contracts/P0037.md`
Output role: `documentation-or-traceability`
Current-safe module: `api_realtime_contracts::external_provider_contracts`
Current output: `docs/codex/07-api-realtime-contracts/source_processing_record_docs_implementation_07_api_realtime_contracts_external_provider_contracts.md`

## Boundary

This record is traceability-only. It does not create Rust source, tests, migrations, API handlers, NATS subjects, metrics, workflows, or formal state write paths.

## Current Disposition

- Retains provider boundary requirements for Agent Gateway mediated access, explicit privacy crossing, and no direct model calls from business surfaces.
- Implementation responsibility remains with the current-safe external provider contract module and tests.
- Provider output remains advisory until accepted through rules, state, and event-log gates.

## Required Gates

- Business services, API handlers, WebSocket handlers, rules code, and state services must not call providers directly.
- Local provider fallback across privacy boundaries must be explicit, configured, and audited.
- Production surfaces must not accept placeholder provider credentials.

## Test Responsibility

Covered by `external_provider_contracts_contract_tests`, `s08_fixture_acceptance_contract_tests`, and the S08 stage command for `trpg-api`.
