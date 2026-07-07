# Source Processing Record - Realtime Sync

Prompt ID: `CODEX-0708-07-API-REALTIME-CONTRACTS-81ec4207bf`
Batch: `BATCH-030-07-api-realtime-contracts`
Source prompt: `codex-prompts/07-api-realtime-contracts/P0034.md`
Output role: `documentation-or-traceability`
Current-safe module: `api_realtime_contracts::realtime_sync`
Current output: `docs/codex/07-api-realtime-contracts/source_processing_record_docs_implementation_07_api_realtime_contracts_realtime_sync.md`

## Boundary

This record is traceability-only. It does not create Rust source, tests, migrations, API handlers, NATS subjects, metrics, workflows, or formal state write paths.

## Current Disposition

- Retains realtime replay, reconnect, split-party, multi-room, and projection delta requirements.
- Implementation responsibility remains with the current-safe realtime sync module and tests.
- Realtime output is a filtered read-model delivery surface, not the system of record.

## Required Gates

- Event Store remains canonical.
- Realtime payloads are filtered per principal before serialization.
- Provenance, correlation, causation, sequence, and visibility metadata remain attached.

## Test Responsibility

Covered by `realtime_sync_contract_tests`, `s08_fixture_acceptance_contract_tests`, and the S08 stage command for `trpg-api`.
