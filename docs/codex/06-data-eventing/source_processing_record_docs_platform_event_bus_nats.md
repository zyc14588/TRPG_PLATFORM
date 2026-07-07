# Source Processing Record: Platform Event Bus NATS

Batch: BATCH-027-06-data-eventing
Prompt: CODEX-0657-06-DATA-EVENTING-8cd5928199
Prompt file: codex-prompts/06-data-eventing/P0082.md
Current-safe module label: data_eventing::source_processing_record_docs_platform_event_bus_nats

## Boundary

This is a documentation-or-traceability record only.

- No Rust src/test, migration, event schema, NATS subject, metric, workflow, or API handler is owned by this prompt.
- NATS publication must stay derived from Event Store/outbox records.
- Cross-boundary visibility and provenance labels must be preserved on derived payloads.

## Test Responsibility

The batch verifies this record through Markdown/path self-checks and B027 evidence. NATS contract behavior remains under current-safe primary owners.
