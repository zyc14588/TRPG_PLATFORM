# Source Processing Record - API OpenAPI Implementation

Prompt ID: `CODEX-0705-07-API-REALTIME-CONTRACTS-5a2939791f`
Batch: `BATCH-030-07-api-realtime-contracts`
Source prompt: `codex-prompts/07-api-realtime-contracts/P0033.md`
Output role: `documentation-or-traceability`
Current-safe module: `api_realtime_contracts::openapi`
Current output: `docs/codex/07-api-realtime-contracts/source_processing_record_docs_api_openapi_implementation.md`

## Boundary

This record is traceability-only. It does not create Rust source, tests, migrations, API handlers, NATS subjects, metrics, workflows, or formal state write paths.

## Current Disposition

- Retains OpenAPI contract requirements for authority mode, actor identity, visibility labels, fact provenance, idempotency, and correlation metadata.
- Implementation responsibility remains with the current-safe OpenAPI contract module and tests.
- Names are current-safe and do not use source provenance paths as current product identifiers.

## Required Gates

- Formal command endpoints remain workflow entry points.
- Projection endpoints remain rebuildable read-model access.
- Provider and model access remains Agent Gateway mediated.
- Restricted facts, Keeper-only fields, provider secrets, and hidden reasoning are not exposed in API schema surfaces.

## Test Responsibility

Covered by `openapi_contract_tests`, `openapi_index_contract_tests`, `s08_fixture_acceptance_contract_tests`, and the S08 stage command for `trpg-api`.
