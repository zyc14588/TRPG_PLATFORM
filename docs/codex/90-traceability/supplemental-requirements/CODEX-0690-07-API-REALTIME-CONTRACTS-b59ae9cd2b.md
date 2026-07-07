# Supplemental Requirement Merge Instruction

Prompt ID: `CODEX-0690-07-API-REALTIME-CONTRACTS-b59ae9cd2b`
Primary Prompt: `CODEX-0069-07-API-REALTIME-CONTRACTS-3cc61a7d01`
Batch: `BATCH-029-07-api-realtime-contracts`
Target module: `api_realtime_contracts::openapi_index`
Source prompt: `codex-prompts/07-api-realtime-contracts/P0020.md`

## Boundary

This supplemental prompt is not an independent Rust source, test, migration, API handler, NATS subject, or formal state write owner.

## Merge Instruction

Merge these constraints into the primary OpenAPI index contract work:

- Expose governed command fields, required headers, event schemas, visibility, fact provenance, and replay/audit metadata.
- Include the Axum handler boundary and utoipa schema boundary as contract metadata only.
- Represent OpenFGA, OPA, and Tool Permission Gate as required policy gates.
- Preserve Event Store as source of truth; OpenAPI metadata must not imply projection writes are authoritative.
- Keep all OpenAPI schema names current-safe.
