# Source processing index

> BATCH-049 current-safe traceability output. This page is documentation evidence only; it is not a Rust module, migration, API contract, event schema, NATS subject, metric, workflow, product test, or formal state-write owner.

## Current-safe target

- Batch: BATCH-049-90-traceability — Strict Governance Final
- Output role: documentation-or-traceability
- Task type: traceability-maintenance
- Current output: docs/codex/90-traceability/source_processing_index.md

| Prompt ID | Prompt file | Current crate | Current module | Source file (provenance only) | Source SHA256 |
|---|---|---|---|---|---|
| CODEX-1048-90-TRACEABILITY-93cb79e8d2 | codex-prompts/90-traceability/P0081.md | trpg-docs-governance | traceability::source_processing_index | docs/implementation/90-traceability/per-file-code-ready/90-traceability/docs-implementation-90-traceability-source-processing-index-982176810d.v5-code-ready.md | c6801bebb38dbcafe5ed323d0a5c56f1793781bd9f590e06d92ca9b5b27f41a6 |

## Allowed change boundary

- Maintain this current-safe Markdown index and its traceability metadata only.
- Do not create or modify business Rust src, product tests, migrations, API handlers, event schemas, NATS subjects, metrics, workflow code, provider adapters, or state-write paths from this prompt.
- Treat historical version labels, hashes, and source path fragments as provenance only; do not promote source proposals or names into current implementation or acceptance.

## Governance invariants retained

- Authority Contract remains immutable and fork-only; HUMAN_KP and AI_KP remain campaign-level mutually exclusive modes.
- Business and KP services must not call a model directly; AI routes through Agent Gateway, Agent Orchestrator/Runtime, and a Model Provider Adapter.
- AI must not write the database or make unlogged formal rulings; formal decisions pass tools, rules, state services, and the event log through Command -> Workflow -> Decision -> Event Store -> Projection.
- Visibility Label and Fact Provenance remain mandatory across API, events, agent context, tool results, RAG, summaries, exports, replay, logs, and metrics.

## Batch disposition and test responsibility

- Disposition: retain as the B049 docs-only current-safe target; no historical implementation proposal is activated.
- Test responsibility: B049 checks must verify the H1, Prompt ID, current-safe target/module, prompt and provenance paths, source SHA256, map agreement, Markdown table shape, and docs-only boundary.
