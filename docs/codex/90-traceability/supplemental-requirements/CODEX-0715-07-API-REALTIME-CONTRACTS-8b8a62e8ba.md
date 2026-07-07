# Supplemental Requirement Merge Instruction

Prompt ID: `CODEX-0715-07-API-REALTIME-CONTRACTS-8b8a62e8ba`
Primary Prompt: `CODEX-0689-07-API-REALTIME-CONTRACTS-4b17a0fb09`
Batch: `BATCH-030-07-api-realtime-contracts`
Target module: `api_realtime_contracts::openapi`
Source prompt: `codex-prompts/07-api-realtime-contracts/P0041.md`

## Boundary

This supplemental prompt is not an independent Rust source, test, migration, API handler, NATS subject, metric, workflow, or formal state write owner.

## Merge Instruction

- Merge OpenAPI governance requirements for authority mode, actor identity, visibility labels, fact provenance, idempotency, and correlation metadata into the current-safe OpenAPI contract.
- Document formal command routes as command workflow entry points, not direct state mutation endpoints.
- API schemas must not expose private facts, Keeper-only fields, provider secrets, internal prompt text, or hidden reasoning channels.
- Provider access must be represented only through Agent Gateway and runtime adapter boundaries.
- Keep generated or documented schema names current-safe and independent of source provenance filenames.

## Test Responsibility

Coverage remains with `openapi_contract_tests`, `openapi_index_contract_tests`, `s08_fixture_acceptance_contract_tests`, and the S08 stage command for `trpg-api`.
