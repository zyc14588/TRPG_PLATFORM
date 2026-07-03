# 00-index Prompt Manifest

> Batch: BATCH-001-00-index
> Role: documentation-or-traceability
> Current crate: trpg-docs-governance
> Current module: docs_governance::manifest

This file records the current-safe manifest responsibility for
`CODEX-0001-00-INDEX-996d963665`.

## Boundary

- Maintains documentation, indexes, matrices, reports, validation checklists, and traceability evidence.
- Does not create Rust `src/`, tests, migrations, API handlers, event schemas, NATS subjects, metrics, workflows, or provider calls.
- Treats historical source names, version labels, hashes, and old paths as provenance only.

## Current Responsibilities

- Keep Prompt ID coverage aligned with `batches/B001.md`.
- Prefer `CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`,
  `CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md`, and `CURRENT_TOKEN_REWRITE_TABLE.md`
  over source-derived suggested names.
- Record evidence in `evidence/batches/BATCH-001/`.

## Test Responsibility

- Verify B001 has 25 rows and 25 referenced prompt files.
- Verify all 25 rows are `documentation-or-traceability`.
- Verify no B001 output creates business implementation artifacts.
