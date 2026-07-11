# adr trace

> BATCH-046 current-safe traceability output. This page is documentation evidence only; it is not a Rust module, migration, API contract, NATS subject, metric, workflow, or product test owner.

## Current-safe target

- Batch: BATCH-046-90-traceability
- Output role: documentation-or-traceability
- Task type: traceability-maintenance
- Current output: docs/codex/90-traceability/adr_trace.md

| Prompt ID | Prompt file | Current crate | Current module | Source file | Source SHA256 |
|---|---|---|---|---|---|
| CODEX-0109-90-TRACEABILITY-e2a8e2ca1a | codex-prompts/90-traceability/P0001.md | trpg-docs-governance | traceability::adr_trace | docs/implementation/90-traceability/adr-trace.md | 8700dfac272adc633871977fdbc81fd95b98a2a0fa2d7ee79f1280a5d580fec1 |
| CODEX-0981-90-TRACEABILITY-9bb68616a1 | codex-prompts/90-traceability/P0014.md | trpg-docs-governance | traceability::adr_trace | docs/implementation/90-traceability/per-file-code-ready/90-traceability/docs-implementation-90-traceability-adr-trace-3206c72099.v5-code-ready.md | 075383860f69053637d5b2eeb2e8a8ecf6bb85ac0e3b618b8edb67d5a26aada4 |

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

- CODEX-0109-90-TRACEABILITY-e2a8e2ca1a: implemented as docs-only traceability; test responsibility is covered by B046 prompt coverage, current-safe output, docs-only boundary, and provenance checks in evidence/batches/BATCH-046/test-output.txt.
- CODEX-0981-90-TRACEABILITY-9bb68616a1: implemented as docs-only traceability; test responsibility is covered by B046 prompt coverage, current-safe output, docs-only boundary, and provenance checks in evidence/batches/BATCH-046/test-output.txt.

<!-- BATCH-049-START -->
## BATCH-049 current-safe trace

| Prompt ID | Prompt file | Current crate | Current module | Source file | Source SHA256 |
|---|---|---|---|---|---|
| `CODEX-1059-90-TRACEABILITY-1427a2ad0e` | `codex-prompts/90-traceability/P0092.md` | `trpg-docs-governance` | `traceability::adr_trace` | `docs/implementation/90-traceability/per-file-code-ready/90-traceability/sources-v3-baseline-document-group-docs-implementation-90-traceability-adr-trace-59c5a3e7b6.v5-code-ready.md` | `fa0de70de057a616a9f7179400e93856617d92c15b0247e4017aeebcadcb9390` |

- `CODEX-1059-90-TRACEABILITY-1427a2ad0e`: implemented as additive
  docs-only traceability. Historical tokens are provenance only; no product
  implementation is owned. Test evidence:
  `evidence/batches/BATCH-049/test-output.txt`.
<!-- BATCH-049-END -->
