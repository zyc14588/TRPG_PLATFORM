# implementation plan

> BATCH-046 current-safe traceability output. This page is documentation evidence only; it is not a Rust module, migration, API contract, NATS subject, metric, workflow, or product test owner.

## Current-safe target

- Batch: BATCH-046-90-traceability
- Output role: documentation-or-traceability
- Task type: traceability-maintenance
- Current output: docs/codex/90-traceability/implementation_plan.md

| Prompt ID | Prompt file | Current crate | Current module | Source file | Source SHA256 |
|---|---|---|---|---|---|
| CODEX-0987-90-TRACEABILITY-d38b2018ac | codex-prompts/90-traceability/P0019.md | trpg-docs-governance | traceability::implementation_plan | docs/implementation/90-traceability/per-file-code-ready/90-traceability/docs-implementation-90-traceability-generated-from-source-docs-implementation-90-traceability-source-breakdown-ab4d9b7975.v5-code-ready.md | 779e33ce0ab14acd2f09910fcc14319f99d4b3a73118f49e8bddd0e59db4355e |

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

- CODEX-0987-90-TRACEABILITY-d38b2018ac: implemented as docs-only traceability; test responsibility is covered by B046 prompt coverage, current-safe output, docs-only boundary, and provenance checks in evidence/batches/BATCH-046/test-output.txt.

<!-- BATCH-047-START -->
## BATCH-047 current-safe trace

| Prompt ID | Prompt file | Current crate | Current module | Source file | Source SHA256 |
|---|---|---|---|---|---|
| CODEX-1002-90-TRACEABILITY-4de2f4b503 | codex-prompts/90-traceability/P0029.md | trpg-docs-governance | traceability::implementation_plan | docs/implementation/90-traceability/per-file-code-ready/90-traceability/docs-implementation-90-traceability-generated-from-source-strict-docs-implementation-90-traceability-source-br-54406a5f59.v5-code-ready.md | 2e32e00182b642c10e2dcb4375634a1aa51c429f0cdf56d220bc0221f1a3863e |

- CODEX-1002-90-TRACEABILITY-4de2f4b503: implemented as docs-only traceability; no Rust, migration, API, event, NATS, metric, workflow, provider, or formal state-write output is owned by this prompt.
<!-- BATCH-047-END -->
