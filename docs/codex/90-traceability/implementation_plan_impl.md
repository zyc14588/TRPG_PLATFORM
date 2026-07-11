# implementation plan impl

> BATCH-048 current-safe traceability output. This page is documentation
> evidence only; it is not a Rust module, migration, API contract, event
> schema, NATS subject, metric, workflow, provider adapter, product test, or
> formal state-write owner.

## Current-safe target

- Batch: `BATCH-048-90-traceability`
- Output role: `documentation-or-traceability`
- Task type: `traceability-maintenance`
- Current output: `docs/codex/90-traceability/implementation_plan_impl.md`

| Prompt ID | Prompt file | Current crate | Current module | Source file | Source SHA256 |
|---|---|---|---|---|---|
| `CODEX-1017-90-TRACEABILITY-6780a72e42` | `codex-prompts/90-traceability/P0050.md` | `trpg-docs-governance` | `traceability::implementation_plan_impl` | `docs/implementation/90-traceability/per-file-code-ready/90-traceability/docs-implementation-90-traceability-source-breakdown-roadmap-implementation-plan-impl-ac3b920b92.v5-code-ready.md` | `ce430d4706bf8ac6bd8950f009319d10ab4100f97f50c5533307d7170fb301ae` |

## Allowed change boundary

- Maintain Markdown traceability, provenance, indexes, matrices, reports,
  validation notes, and batch evidence only.
- Do not create or modify business Rust `src/`, product tests, migrations,
  API handlers, event schemas, NATS subjects, metrics, workflow code, provider
  adapters, or formal state-write paths from this prompt.
- Historical version labels, hashes, and source path fragments above are
  provenance only. Current module and output ownership come from
  `CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md`.
- This record is not a current acceptance entry and does not replace the
  top-level design, normalized maps, stage gates, or V1 evidence matrix.

## Governance invariants retained

- Authority Contract remains immutable and fork-only.
- AI capabilities must route through Agent Gateway, Orchestrator/Runtime, and
  Model Provider Adapter.
- Formal state writes must pass Command -> Workflow -> Decision -> Event Store
  -> Projection.
- Visibility Label and Fact Provenance remain required across API, events,
  agent context, RAG, summary, export, replay, logs, and metrics.

## Batch disposition

- `CODEX-1017-90-TRACEABILITY-6780a72e42`: implemented as docs-only traceability.
- Test responsibility: target existence, Prompt ID, current-safe map agreement,
  source path/SHA, documentation-only boundary, and S00 governance checks in
  `evidence/batches/BATCH-048/test-output.txt`.

<!-- BATCH-050-START -->
## BATCH-050 current-safe trace

| Prompt ID | Prompt file | Current crate | Current module | Source file | Source SHA256 |
|---|---|---|---|---|---|
| `CODEX-1067-90-TRACEABILITY-9bb3354c34` | `codex-prompts/90-traceability/P0099.md` | `trpg-docs-governance` | `traceability::implementation_plan_impl` | `docs/implementation/90-traceability/per-file-code-ready/90-traceability/sources-v3-baseline-document-group-docs-implementation-90-traceability-source-breakdown-roadmap-implementation-6d0ed24630.v5-code-ready.md` | `c1e243dd361b6f89ae2b75b03a8ab366b5758cd1afc6c24cfd9667ac83c42465` |

- Disposition: implemented as additive docs-only traceability; this prompt owns
  no Rust `src/` or product-test output, migration, API, event schema, NATS
  subject, metric, workflow, provider adapter, or formal state-write path.
- Test responsibility: target existence, Prompt ID uniqueness, current-safe map
  and source path/SHA agreement, documentation-only boundary, and applicable
  S00 governance checks in `evidence/batches/BATCH-050/test-output.txt`.
<!-- BATCH-050-END -->
