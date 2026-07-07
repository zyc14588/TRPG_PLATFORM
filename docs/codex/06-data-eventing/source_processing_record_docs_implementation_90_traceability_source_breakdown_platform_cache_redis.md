# Source Processing Record: Platform Redis Cache Breakdown

Batch: BATCH-027-06-data-eventing
Prompt: CODEX-0655-06-DATA-EVENTING-39e1d5396f
Prompt file: codex-prompts/06-data-eventing/P0071.md
Current-safe module label: data_eventing::source_processing_record_docs_implementation_90_traceability_source_breakdown_platform_cache_redis

## Boundary

This is a documentation-or-traceability record only.

- No Rust src/test, migration, event schema, NATS subject, metric, workflow, or API handler is owned by this prompt.
- Redis cache and presence records are derived read models, not canonical state.
- Visibility and fact provenance must not be lost when cache entries are rebuilt from Event Store data.

## Test Responsibility

The batch verifies this record through Markdown/path self-checks and B027 evidence. Cache behavior remains covered by current-safe primary owners and S03 checks.
