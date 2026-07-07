# Source Processing Record: Platform PostgreSQL Breakdown

Batch: BATCH-027-06-data-eventing
Prompt: CODEX-0654-06-DATA-EVENTING-c650c81a4b
Prompt file: codex-prompts/06-data-eventing/P0079.md
Current-safe module label: data_eventing::source_processing_record_docs_implementation_90_traceability_source_breakdown_platform_persistence_postgresql

## Boundary

This is a documentation-or-traceability record only.

- No Rust src/test, migration, event schema, NATS subject, metric, workflow, or API handler is owned by this prompt.
- PostgreSQL guidance is provenance for Event Store, outbox, and projection owners.
- Formal writes must stay inside transaction-backed Event Store append paths.

## Test Responsibility

The batch verifies this record through Markdown/path self-checks and B027 evidence. Migration and persistence behavior remains covered by current-safe primary owners and S03 checks.
