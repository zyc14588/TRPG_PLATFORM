# Supplemental Requirement Merge Instruction

Prompt ID: `CODEX-0697-07-API-REALTIME-CONTRACTS-57f9fff7f4`
Primary Prompt: `CODEX-0688-07-API-REALTIME-CONTRACTS-991d938d5b`
Batch: `BATCH-029-07-api-realtime-contracts`
Target module: `api_realtime_contracts::api_web_socket_g_rpc_schema`
Source prompt: `codex-prompts/07-api-realtime-contracts/P0014.md`

## Boundary

This supplemental prompt is not an independent Rust source, test, migration, API handler, NATS subject, or formal state write owner.

## Merge Instruction

Merge these constraints into the primary API WebSocket/gRPC schema contract work:

- Schema metadata must include visibility, fact provenance, idempotency, expected version, and correlation fields.
- WebSocket and gRPC schema contracts are delivery/transport contracts only; they cannot bypass State Service or Event Store writes.
- Provider or AI calls remain outside schema handling except through Agent Gateway governed contracts.
- Reject historical version tokens and source-path names in schema, event, NATS, metric, and test identifiers.
- Keep schema compatibility checks aligned with OpenAPI and realtime contract metadata.
