# docs implementation 90 traceability adr trace strict previous

> BATCH-047 current-safe traceability output. This page is documentation evidence only; it is not a Rust module, migration, API contract, NATS subject, metric, workflow, or product test owner.

## Current-safe target

- Batch: BATCH-047-90-traceability
- Output role: documentation-or-traceability
- Task type: traceability-maintenance
- Current output: docs/codex/90-traceability/docs_implementation_90_traceability_adr_trace_strict_previous.md

| Prompt ID | Prompt file | Current crate | Current module | Source file | Source SHA256 |
|---|---|---|---|---|---|
| CODEX-0990-90-TRACEABILITY-a87cf26263 | codex-prompts/90-traceability/P0036.md | trpg-docs-governance | traceability::docs_implementation_90_traceability_adr_trace_previous_provenance | docs/implementation/90-traceability/per-file-code-ready/90-traceability/docs-implementation-90-traceability-generated-from-source-strict-docs-implementation-90-traceability-adr-trace-2d7aa81de4.v5-code-ready.md | fd7d8d4252c8e95bdd0f21fa78689fa0a3106ae82997b5016cfb6cfb2b55104b |

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

- CODEX-0990-90-TRACEABILITY-a87cf26263: implemented as docs-only traceability; test responsibility is covered by B047 prompt coverage, current-safe output, docs-only boundary, and provenance checks in evidence/batches/BATCH-047/test-output.txt.
