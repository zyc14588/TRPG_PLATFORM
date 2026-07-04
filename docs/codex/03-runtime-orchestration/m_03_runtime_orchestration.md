# m_03_runtime_orchestration

Prompt ID: `CODEX-0031-03-RUNTIME-ORCHESTRATION-cac012cf70`

Role: `documentation-or-traceability`.

Current-safe module: `runtime_orchestration::m_03_runtime_orchestration`

Current output: `docs/codex/03-runtime-orchestration/m_03_runtime_orchestration.md`

## Scope

This document records the BATCH-012 runtime orchestration documentation landing point. It does not own Rust `src/`, Rust `tests/`, migrations, API handlers, NATS subjects, metrics, or formal state write paths.

## Governance

- Authority Contract remains immutable after creation; authority changes require forked campaign lineage.
- AI access remains constrained to Agent Gateway, Agent Orchestrator/Runtime, and Model Provider Adapter layers.
- Formal state changes remain on the Command -> Workflow -> Decision -> Event Store -> Projection path.
- Event Store remains canon; projections, cache, RAG indexes, and summaries remain rebuildable read models.
- Visibility labels and fact provenance must propagate through runtime events, replay, summaries, exports, logs, and metrics.

## Batch 012 Trace

`BATCH-012-03-runtime-orchestration` implements runtime prompt rows in current-safe files under `crates/trpg-runtime/` and supplemental traceability files under `docs/codex/90-traceability/supplemental-requirements/`.

This file closes the docs-governance row for `CODEX-0031-03-RUNTIME-ORCHESTRATION-cac012cf70`.
