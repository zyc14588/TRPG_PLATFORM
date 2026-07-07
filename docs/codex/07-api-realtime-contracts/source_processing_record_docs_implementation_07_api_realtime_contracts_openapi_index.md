# Source Processing Record - OpenAPI Index

Prompt ID: `CODEX-0706-07-API-REALTIME-CONTRACTS-f3f7bf8d6b`
Batch: `BATCH-030-07-api-realtime-contracts`
Source prompt: `codex-prompts/07-api-realtime-contracts/P0039.md`
Output role: `documentation-or-traceability`
Current-safe module: `api_realtime_contracts::openapi_index`
Current output: `docs/codex/07-api-realtime-contracts/source_processing_record_docs_implementation_07_api_realtime_contracts_openapi_index.md`

## Boundary

This record is traceability-only. It does not create Rust source, tests, migrations, API handlers, NATS subjects, metrics, workflows, or formal state write paths.

## Current Disposition

- Retains OpenAPI index requirements for route, schema, security, visibility, provenance, and idempotency mapping.
- Implementation responsibility remains with the current-safe OpenAPI index module and tests.
- Product identifiers remain normalized and current-safe.

## Required Gates

- The index distinguishes formal command endpoints from rebuildable projection reads.
- Security and privacy metadata must be visible in the documented contract.
- Provider access is represented only through the Agent Gateway boundary.

## Test Responsibility

Covered by `openapi_index_contract_tests`, `openapi_contract_tests`, `s08_fixture_acceptance_contract_tests`, and the S08 stage command for `trpg-api`.
