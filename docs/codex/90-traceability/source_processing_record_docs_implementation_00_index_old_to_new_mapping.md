# source processing record docs implementation 00 index old to new mapping

> BATCH-048 current-safe traceability output. This page is documentation
> evidence only; it is not a Rust module, migration, API contract, event
> schema, NATS subject, metric, workflow, provider adapter, product test, or
> formal state-write owner.

## Current-safe target

- Batch: `BATCH-048-90-traceability`
- Output role: `documentation-or-traceability`
- Task type: `traceability-maintenance`
- Current output: `docs/codex/90-traceability/source_processing_record_docs_implementation_00_index_old_to_new_mapping.md`

| Prompt ID | Prompt file | Current crate | Current module | Source file | Source SHA256 |
|---|---|---|---|---|---|
| `CODEX-1023-90-TRACEABILITY-ea4fa75543` | `codex-prompts/90-traceability/P0059.md` | `trpg-docs-governance` | `traceability::source_processing_record_docs_implementation_00_index_historical_to_current_mapping` | `docs/implementation/90-traceability/per-file-code-ready/90-traceability/docs-implementation-90-traceability-source-processing-00-index-docs-implementation-00-index-old-to-new-mapping-361476f218.v5-code-ready.md` | `cfa083f64896a233a71c13c26b576a508ad4171d4eec7ce71ae988fc95a5347e` |

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

- `CODEX-1023-90-TRACEABILITY-ea4fa75543`: implemented as docs-only traceability.
- Test responsibility: target existence, Prompt ID, current-safe map agreement,
  source path/SHA, documentation-only boundary, and S00 governance checks in
  `evidence/batches/BATCH-048/test-output.txt`.

