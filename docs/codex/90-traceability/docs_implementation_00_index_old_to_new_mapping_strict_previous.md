# docs implementation 00 index old to new mapping strict previous

> BATCH-046 current-safe traceability output. This page is documentation evidence only; it is not a Rust module, migration, API contract, NATS subject, metric, workflow, or product test owner.

## Current-safe target

- Batch: BATCH-046-90-traceability
- Output role: documentation-or-traceability
- Task type: traceability-maintenance
- Current output: docs/codex/90-traceability/docs_implementation_00_index_old_to_new_mapping_strict_previous.md

| Prompt ID | Prompt file | Current crate | Current module | Source file | Source SHA256 |
|---|---|---|---|---|---|
| CODEX-0972-90-TRACEABILITY-b78073d66f | codex-prompts/90-traceability/P0007.md | trpg-docs-governance | traceability::docs_implementation_00_index_historical_to_current_mapping_previous_provenance | docs/implementation/90-traceability/per-file-code-ready/90-traceability/docs-implementation-00-index-generated-from-source-strict-docs-implementation-00-index-old-to-new-mapping-stri-607173325f.v5-code-ready.md | 254a7b122068d63fd4c5ce599ac24605fc91fe8e84ec80398d023dd1ed72929c |

## Allowed change boundary

- Maintain Markdown traceability, provenance, indexes, matrices, reports, validation notes, and batch evidence only.
- Do not create or modify business Rust src, product tests, migrations, API handlers, event schemas, NATS subjects, metrics, workflow code, provider adapters, or state-write paths from this batch.
- Treat historical V3 / V4 / V5 / V6, hashes, and source path fragments as provenance only. Current naming must come from CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md.

## Governance invariants retained

- Authority Contract remains immutable and fork-only.
- AI capabilities must route through Agent Gateway, Orchestrator/Runtime, and Model Provider Adapter.
- Formal state writes must pass Command -> Workflow -> Decision -> Event Store -> Projection.
- Visibility Label and Fact Provenance remain required across API, events, agent context, RAG, summary, export, replay, logs, and metrics.

## Batch disposition

- CODEX-0972-90-TRACEABILITY-b78073d66f: implemented as docs-only traceability; test responsibility is covered by B046 prompt coverage, current-safe output, docs-only boundary, and provenance checks in evidence/batches/BATCH-046/test-output.txt.
