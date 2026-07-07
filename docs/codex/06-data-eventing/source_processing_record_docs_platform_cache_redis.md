# Source Processing Record: Platform Cache Redis

Batch: BATCH-027-06-data-eventing
Prompt: CODEX-0656-06-DATA-EVENTING-8cf7c65caf
Prompt file: codex-prompts/06-data-eventing/P0083.md
Current-safe module label: data_eventing::source_processing_record_docs_platform_cache_redis

## Boundary

This is a documentation-or-traceability record only.

- No Rust src/test, migration, event schema, NATS subject, metric, workflow, or API handler is owned by this prompt.
- Cache output must remain rebuildable from Event Store events and may not become a fact source.
- Historical source names remain provenance only.

## Test Responsibility

The batch verifies this record through Markdown/path self-checks and B027 evidence. Runtime cache assertions remain under the owning primary implementation prompts.
