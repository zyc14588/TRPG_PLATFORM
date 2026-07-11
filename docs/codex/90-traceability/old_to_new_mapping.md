# old to new mapping

> BATCH-046 current-safe traceability output. This page is documentation evidence only; it is not a Rust module, migration, API contract, NATS subject, metric, workflow, or product test owner.

## Current-safe target

- Batch: BATCH-046-90-traceability
- Output role: documentation-or-traceability
- Task type: traceability-maintenance
- Current output: docs/codex/90-traceability/old_to_new_mapping.md

| Prompt ID | Prompt file | Current crate | Current module | Source file | Source SHA256 |
|---|---|---|---|---|---|
| CODEX-0113-90-TRACEABILITY-9629d02bc9 | codex-prompts/90-traceability/P0004.md | trpg-docs-governance | traceability::historical_to_current_mapping | docs/implementation/90-traceability/old-to-new-mapping.md | e7e0425c0d33ef62f330b6afbaaa5055a0a1cc0204ec5971b64d9ba8d8a68eff |
| CODEX-0978-90-TRACEABILITY-4c2114e61c | codex-prompts/90-traceability/P0011.md | trpg-docs-governance | traceability::historical_to_current_mapping | docs/implementation/90-traceability/per-file-code-ready/90-traceability/docs-implementation-00-index-old-to-new-mapping-095ebb0021.v5-code-ready.md | be6965a3dc9960b24d3de317c14898a7f1e8bd2cfecca527b085c44e929ae022 |

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

- CODEX-0113-90-TRACEABILITY-9629d02bc9: implemented as docs-only traceability; test responsibility is covered by B046 prompt coverage, current-safe output, docs-only boundary, and provenance checks in evidence/batches/BATCH-046/test-output.txt.
- CODEX-0978-90-TRACEABILITY-4c2114e61c: implemented as docs-only traceability; test responsibility is covered by B046 prompt coverage, current-safe output, docs-only boundary, and provenance checks in evidence/batches/BATCH-046/test-output.txt.

<!-- BATCH-047-START -->
## BATCH-047 current-safe trace

| Prompt ID | Prompt file | Current crate | Current module | Source file | Source SHA256 |
|---|---|---|---|---|---|
| CODEX-1011-90-TRACEABILITY-32e59e6840 | codex-prompts/90-traceability/P0044.md | trpg-docs-governance | traceability::historical_to_current_mapping | docs/implementation/90-traceability/per-file-code-ready/90-traceability/docs-implementation-90-traceability-old-to-new-mapping-0d62147f3d.v5-code-ready.md | 44b91e3eda21c4364c44a055e56d7b2151f3040740a58786fed54d8a86389d32 |

- CODEX-1011-90-TRACEABILITY-32e59e6840: implemented as docs-only traceability under current-safe target `docs/codex/90-traceability/old_to_new_mapping.md`; no Rust, migration, API, event, NATS, metric, workflow, provider, or formal state-write output is owned by this prompt.
<!-- BATCH-047-END -->
