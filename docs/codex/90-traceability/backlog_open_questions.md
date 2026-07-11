# backlog open questions

> BATCH-046 current-safe traceability output. This page is documentation evidence only; it is not a Rust module, migration, API contract, NATS subject, metric, workflow, or product test owner.

## Current-safe target

- Batch: BATCH-046-90-traceability
- Output role: documentation-or-traceability
- Task type: traceability-maintenance
- Current output: docs/codex/90-traceability/backlog_open_questions.md

| Prompt ID | Prompt file | Current crate | Current module | Source file | Source SHA256 |
|---|---|---|---|---|---|
| CODEX-0986-90-TRACEABILITY-f0c84ff76f | codex-prompts/90-traceability/P0020.md | trpg-docs-governance | traceability::backlog_open_questions | docs/implementation/90-traceability/per-file-code-ready/90-traceability/docs-implementation-90-traceability-generated-from-source-docs-implementation-90-traceability-source-breakdown-75bd773325.v5-code-ready.md | 7140a771f2fdf37bd9f28452f945a09df4ed6b255e584b9d954841ea5483ecba |

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

- CODEX-0986-90-TRACEABILITY-f0c84ff76f: implemented as docs-only traceability; test responsibility is covered by B046 prompt coverage, current-safe output, docs-only boundary, and provenance checks in evidence/batches/BATCH-046/test-output.txt.

<!-- BATCH-047-START -->
## BATCH-047 current-safe trace

| Prompt ID | Prompt file | Current crate | Current module | Source file | Source SHA256 |
|---|---|---|---|---|---|
| CODEX-0998-90-TRACEABILITY-368f0cb4e9 | codex-prompts/90-traceability/P0028.md | trpg-docs-governance | traceability::backlog_open_questions | docs/implementation/90-traceability/per-file-code-ready/90-traceability/docs-implementation-90-traceability-generated-from-source-strict-docs-implementation-90-traceability-source-br-1faba8ee51.v5-code-ready.md | 92abe483b070e2889191b8a4d61b52c156d4d7ddf6d1d648f7208c9fef0c6e50 |

- CODEX-0998-90-TRACEABILITY-368f0cb4e9: implemented as docs-only traceability; no Rust, migration, API, event, NATS, metric, workflow, provider, or formal state-write output is owned by this prompt.
<!-- BATCH-047-END -->
