# domain model

> BATCH-047 current-safe traceability output. This page is documentation evidence only; it is not a Rust module, migration, API contract, NATS subject, metric, workflow, or product test owner.

## Current-safe target

- Batch: BATCH-047-90-traceability
- Output role: documentation-or-traceability
- Task type: traceability-maintenance
- Current output: docs/codex/90-traceability/domain_model.md

| Prompt ID | Prompt file | Current crate | Current module | Source file | Source SHA256 |
|---|---|---|---|---|---|
| CODEX-1003-90-TRACEABILITY-18e59dc953 | codex-prompts/90-traceability/P0027.md | trpg-docs-governance | traceability::domain_model | docs/implementation/90-traceability/per-file-code-ready/90-traceability/docs-implementation-90-traceability-generated-from-source-strict-docs-implementation-90-traceability-source-br-6a23f31300.v5-code-ready.md | a01c81f64391182ac4eddac1817694d1abba792c019e15637eabe8019174b832 |

## Allowed change boundary

- Maintain Markdown traceability, provenance, indexes, matrices, reports, validation notes, and batch evidence only.
- Do not create or modify business Rust src, product tests, migrations, API handlers, event schemas, NATS subjects, metrics, workflow code, provider adapters, or state-write paths from this batch.
- Treat historical V3 / V4 / V5 / V6, hashes, and source path fragments as provenance only. Current naming must come from CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md.

## Batch disposition

- CODEX-1003-90-TRACEABILITY-18e59dc953: implemented as docs-only traceability; test responsibility is covered by B047 prompt coverage, current-safe output, docs-only boundary, and provenance checks in evidence/batches/BATCH-047/test-output.txt.
