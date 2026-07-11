# completion matrix

> BATCH-046 current-safe traceability output. This page is documentation evidence only; it is not a Rust module, migration, API contract, NATS subject, metric, workflow, or product test owner.

## Current-safe target

- Batch: BATCH-046-90-traceability
- Output role: documentation-or-traceability
- Task type: traceability-maintenance
- Current output: docs/codex/90-traceability/completion_matrix.md

| Prompt ID | Prompt file | Current crate | Current module | Source file | Source SHA256 |
|---|---|---|---|---|---|
| CODEX-0110-90-TRACEABILITY-2341b21c7e | codex-prompts/90-traceability/P0002.md | trpg-docs-governance | traceability::completion_matrix | docs/implementation/90-traceability/completion-matrix.md | 6bce275aa0f1c8352977d4e16cf819c5d6170829d41ee880ac282dba0de3229b |
| CODEX-0982-90-TRACEABILITY-f9341893bc | codex-prompts/90-traceability/P0015.md | trpg-docs-governance | traceability::completion_matrix | docs/implementation/90-traceability/per-file-code-ready/90-traceability/docs-implementation-90-traceability-completion-matrix-6e9fa4f9ef.v5-code-ready.md | c2f3e80b4614ce40ee9989f17528b8afec3857700dac35c64d324462ea577c57 |

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

- CODEX-0110-90-TRACEABILITY-2341b21c7e: implemented as docs-only traceability; test responsibility is covered by B046 prompt coverage, current-safe output, docs-only boundary, and provenance checks in evidence/batches/BATCH-046/test-output.txt.
- CODEX-0982-90-TRACEABILITY-f9341893bc: implemented as docs-only traceability; test responsibility is covered by B046 prompt coverage, current-safe output, docs-only boundary, and provenance checks in evidence/batches/BATCH-046/test-output.txt.

<!-- BATCH-049-START -->
## BATCH-049 current-safe trace

| Prompt ID | Prompt file | Current crate | Current module | Source file | Source SHA256 |
|---|---|---|---|---|---|
| `CODEX-1060-90-TRACEABILITY-c30586dd61` | `codex-prompts/90-traceability/P0093.md` | `trpg-docs-governance` | `traceability::completion_matrix` | `docs/implementation/90-traceability/per-file-code-ready/90-traceability/sources-v3-baseline-document-group-docs-implementation-90-traceability-completion-matrix-da38d7ba71.v5-code-ready.md` | `be97612a90bcb9502486a66f4d15516075d2de39c2db1df99f317f9d1ddcd1ef` |

- `CODEX-1060-90-TRACEABILITY-c30586dd61`: implemented as additive
  docs-only matrix traceability. Historical tokens are provenance only; no
  product implementation is owned. Test evidence:
  `evidence/batches/BATCH-049/test-output.txt`.
<!-- BATCH-049-END -->
