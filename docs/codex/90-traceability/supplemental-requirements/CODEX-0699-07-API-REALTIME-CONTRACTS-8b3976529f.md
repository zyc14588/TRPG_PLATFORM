# Supplemental Requirement Merge Instruction

Prompt ID: `CODEX-0699-07-API-REALTIME-CONTRACTS-8b3976529f`
Primary Prompt: `CODEX-0069-07-API-REALTIME-CONTRACTS-3cc61a7d01`
Batch: `BATCH-029-07-api-realtime-contracts`
Target module: `api_realtime_contracts::openapi_index`
Source prompt: `codex-prompts/07-api-realtime-contracts/P0025.md`

## Boundary

This supplemental prompt is not an independent Rust source, test, migration, API handler, NATS subject, or formal state write owner.

## Merge Instruction

Merge these constraints into the primary OpenAPI index contract work:

- Keep OpenAPI route, header, schema, event, and policy metadata aligned with current-safe B029 modules.
- Document idempotency, expected version, Authority Contract version, visibility, fact provenance, and audit fields as required API contract inputs.
- Include Tool Permission Gate, OpenFGA, and OPA as required policy gate metadata.
- Do not add independent Rust outputs, migrations, handlers, or NATS subjects from this supplemental prompt.
- Keep OpenAPI output as traceability and schema contract evidence, not a runtime provider or state-write bypass.
