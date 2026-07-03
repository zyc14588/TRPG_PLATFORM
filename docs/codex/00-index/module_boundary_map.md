# Module Boundary Map

> Prompt ID: CODEX-0012-00-INDEX-9f38048f59
> Role: documentation-or-traceability
> Current module: docs_governance::module_boundary_map

The `docs_governance` module boundary is documentation-only for BATCH-001 and
BATCH-002.

## Allowed

- Markdown indexes.
- Prompt and batch traceability.
- Validation checklists.
- Evidence summaries.

## Not Allowed

- Business Rust code.
- Database migrations.
- API, WebSocket, event, NATS, workflow, or metric definitions.
- LLM/provider integration.

## Test Responsibility

Validate that BATCH-001 changes stay inside Markdown governance paths.

For BATCH-002, validate that all 23 prompt outputs resolve to
`docs/codex/00-index/**` or `evidence/batches/BATCH-002/**`, and that no
implementation module, migration, event, metric, workflow, provider adapter, or
runtime test is created.
