# Source Processing Record: API Realtime NATS Subject Contracts

Batch: BATCH-027-06-data-eventing
Prompt: CODEX-0660-06-DATA-EVENTING-d6b6be75de
Prompt file: codex-prompts/06-data-eventing/P0085.md
Current-safe module label: data_eventing::source_processing_record_docs_implementation_07_api_realtime_contracts_nats_subject_contracts

## Boundary

This is a documentation-or-traceability record only.

- No Rust src/test, migration, event schema, NATS subject, metric, workflow, or API handler is owned by this prompt.
- API/realtime NATS contracts must stay Event Store/outbox derived.
- Visibility, fact provenance, correlation id, and causation id must remain auditable across derived payloads.

## Test Responsibility

The batch verifies this record through Markdown/path self-checks and B027 evidence. API/NATS contract behavior remains under current-safe primary owners.
