# Source Processing Record - Platform API Contracts

Prompt ID: `CODEX-0714-07-API-REALTIME-CONTRACTS-23edceb239`
Batch: `BATCH-030-07-api-realtime-contracts`
Source prompt: `codex-prompts/07-api-realtime-contracts/P0031.md`
Output role: `documentation-or-traceability`
Current-safe module: `api_realtime_contracts::traceability`
Current output: `docs/codex/07-api-realtime-contracts/source_processing_record_docs_platform_api_contracts.md`

## Boundary

This record is traceability-only. It does not create Rust source, tests, migrations, API handlers, NATS subjects, metrics, workflows, or formal state write paths.

## Current Disposition

- Retains platform API contract requirements for HTTP, WebSocket, OpenAPI, external provider boundaries, idempotency, and realtime delivery.
- The requirements are recorded as traceability and supplemental merge guidance for current-safe modules.
- No implementation scope is expanded beyond B030.

## Required Gates

- API and realtime contracts must enforce authority mode, visibility labels, fact provenance, idempotency, correlation, and Event Store authority.
- Provider/model use remains behind Agent Gateway, runtime, and adapter boundaries.
- Formal state writes remain Command -> Workflow -> Decision -> Event Store -> Projection.

## Test Responsibility

Covered by existing S08 contract tests and documentation/current-safe scans.
