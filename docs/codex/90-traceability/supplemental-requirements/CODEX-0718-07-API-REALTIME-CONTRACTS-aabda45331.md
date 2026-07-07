# Supplemental Requirement Merge Instruction

Prompt ID: `CODEX-0718-07-API-REALTIME-CONTRACTS-aabda45331`
Primary Prompt: `CODEX-0069-07-API-REALTIME-CONTRACTS-3cc61a7d01`
Batch: `BATCH-030-07-api-realtime-contracts`
Target module: `api_realtime_contracts::openapi_index`
Source prompt: `codex-prompts/07-api-realtime-contracts/P0045.md`

## Boundary

This supplemental prompt is not an independent Rust source, test, migration, API handler, NATS subject, metric, workflow, or formal state write owner.

## Merge Instruction

- Merge OpenAPI index governance requirements into the current-safe OpenAPI index contract.
- The index must map public API surfaces to current-safe route, schema, security, visibility, and provenance sections.
- It must not adopt source provenance filenames, old source labels, or prompt hash fragments as schema, route, module, metric, test, event, or workflow names.
- It must identify formal command endpoints as workflow entry points and projection endpoints as rebuildable read-model access.
- It must preserve provider access as Agent Gateway mediated.

## Test Responsibility

Coverage remains with `openapi_index_contract_tests`, `openapi_contract_tests`, `s08_fixture_acceptance_contract_tests`, and the S08 stage command for `trpg-api`.
