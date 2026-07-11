# docs implementation 90 traceability readme strict previous

> BATCH-047 current-safe traceability output. This page is documentation evidence only; it is not a Rust module, migration, API contract, NATS subject, metric, workflow, or product test owner.

## Current-safe target

- Batch: BATCH-047-90-traceability
- Output role: documentation-or-traceability
- Task type: traceability-maintenance
- Current output: docs/codex/90-traceability/docs_implementation_90_traceability_readme_strict_previous.md

| Prompt ID | Prompt file | Current crate | Current module | Source file | Source SHA256 |
|---|---|---|---|---|---|
| CODEX-0995-90-TRACEABILITY-f38d6fb6a8 | codex-prompts/90-traceability/P0043.md | trpg-docs-governance | traceability::docs_implementation_90_traceability_readme_previous_provenance | docs/implementation/90-traceability/per-file-code-ready/90-traceability/docs-implementation-90-traceability-generated-from-source-strict-docs-implementation-90-traceability-readme-st-0e95b4cea9.v5-code-ready.md | cedd7aab1df756835943d1f3f4e12643d28280a838d740d5820ef85d93166109 |

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

- CODEX-0995-90-TRACEABILITY-f38d6fb6a8: implemented as docs-only traceability; test responsibility is covered by B047 prompt coverage, current-safe output, docs-only boundary, and provenance checks in evidence/batches/BATCH-047/test-output.txt.
