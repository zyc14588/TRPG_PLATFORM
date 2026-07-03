# Reorganization Plan

> Prompt IDs: CODEX-0134-00-INDEX-6076a4e30c, CODEX-0142-00-INDEX-580b588920
> Role: documentation-or-traceability
> Current module: docs_governance::reorganization_plan
> Current output: `docs/codex/00-index/reorganization_plan.md`

This plan records the allowed 00-index reorganization for Batch-002.

## Current-Safe Plan

1. Keep active governance outputs under `docs/codex/00-index/`.
2. Keep batch execution scope under `batches/B002.md` and
   `batch-prompts/**/B002.md`.
3. Keep historical source names as provenance through inventory and source maps.
4. Record execution evidence under `evidence/batches/BATCH-002/`.

## Not In Scope

- Moving implementation code.
- Creating crates, migrations, API handlers, workflow code, event schemas,
  metrics, tests, or provider adapters.
- Reclassifying supplemental prompts as primary implementation prompts.

## Test Responsibility

Validate that every Batch-002 target stays in a documentation, traceability, or
evidence path.
