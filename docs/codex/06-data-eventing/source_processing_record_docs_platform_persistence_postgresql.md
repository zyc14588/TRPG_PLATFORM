# Source Processing Record: Platform Persistence PostgreSQL

Batch: BATCH-027-06-data-eventing
Prompt: CODEX-0658-06-DATA-EVENTING-7a272232f2
Prompt file: codex-prompts/06-data-eventing/P0081.md
Current-safe module label: data_eventing::source_processing_record_docs_platform_persistence_postgresql

## Boundary

This is a documentation-or-traceability record only.

- No Rust src/test, migration, event schema, NATS subject, metric, workflow, or API handler is owned by this prompt.
- PostgreSQL persistence is current-safe only when formal writes go through Event Store append plus outbox.
- Projection/cache/RAG records remain rebuildable read models.

## Test Responsibility

The batch verifies this record through Markdown/path self-checks and B027 evidence. SQLx migration behavior remains under S03 checks and owning primary prompts.
