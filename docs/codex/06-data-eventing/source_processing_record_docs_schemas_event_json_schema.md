# Source Processing Record: Event JSON Schema

Batch: BATCH-027-06-data-eventing
Prompt: CODEX-0659-06-DATA-EVENTING-faa099023a
Prompt file: codex-prompts/06-data-eventing/P0084.md
Current-safe module label: data_eventing::source_processing_record_docs_schemas_event_json_schema

## Boundary

This is a documentation-or-traceability record only.

- No Rust src/test, migration, event schema, NATS subject, metric, workflow, or API handler is owned by this prompt.
- Event schema provenance must not introduce historical path/hash fragments into current event names.
- Event payloads must continue to carry visibility and fact provenance.

## Test Responsibility

The batch verifies this record through Markdown/path self-checks and B027 evidence. Event schema behavior remains under current-safe primary owners.
