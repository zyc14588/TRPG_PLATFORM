# open source reference matrix

> BATCH-047 current-safe traceability output. This page is documentation evidence only; it is not a Rust module, migration, API contract, NATS subject, metric, workflow, or product test owner.

## Current-safe target

- Batch: BATCH-047-90-traceability
- Output role: documentation-or-traceability
- Task type: traceability-maintenance
- Current output: docs/codex/90-traceability/open_source_reference_matrix.md

| Prompt ID | Prompt file | Current crate | Current module | Source file | Source SHA256 |
|---|---|---|---|---|---|
| CODEX-1007-90-TRACEABILITY-13bd257921 | codex-prompts/90-traceability/P0026.md | trpg-docs-governance | traceability::open_source_reference_matrix | docs/implementation/90-traceability/per-file-code-ready/90-traceability/docs-implementation-90-traceability-generated-from-source-strict-docs-implementation-90-traceability-source-br-cbc732454d.v5-code-ready.md | be1eafa3021e9b22c0e1c2254c693b35e5b38932b642af5137c36f4434717106 |

## Allowed change boundary

- Maintain Markdown traceability, provenance, indexes, matrices, reports, validation notes, and batch evidence only.
- Do not create or modify business Rust src, product tests, migrations, API handlers, event schemas, NATS subjects, metrics, workflow code, provider adapters, or state-write paths from this batch.
- Treat historical V3 / V4 / V5 / V6, hashes, and source path fragments as provenance only. Current naming must come from CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md.

## Batch disposition

- CODEX-1007-90-TRACEABILITY-13bd257921: implemented as docs-only traceability; test responsibility is covered by B047 prompt coverage, current-safe output, docs-only boundary, and provenance checks in evidence/batches/BATCH-047/test-output.txt.
