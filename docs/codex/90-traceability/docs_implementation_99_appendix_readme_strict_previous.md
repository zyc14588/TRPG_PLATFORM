# Docs implementation 99 appendix README — previous provenance

> BATCH-049 current-safe provenance output. This page is documentation evidence only and is explicitly not a current acceptance entry point, Rust module, migration, API contract, event schema, NATS subject, metric, workflow, product test, or formal state-write owner.

## Current-safe target

- Batch: BATCH-049-90-traceability — Strict Governance Final
- Output role: documentation-or-traceability
- Task type: traceability-maintenance
- Current output: docs/codex/90-traceability/docs_implementation_99_appendix_readme_strict_previous.md

| Prompt ID | Prompt file | Current crate | Current module | Source file (provenance only) | Source SHA256 |
|---|---|---|---|---|---|
| CODEX-1052-90-TRACEABILITY-abac7952b1 | codex-prompts/90-traceability/P0085.md | trpg-docs-governance | traceability::docs_implementation_99_appendix_readme_previous_provenance | docs/implementation/90-traceability/per-file-code-ready/90-traceability/docs-implementation-99-appendix-generated-from-source-strict-docs-implementation-99-appendix-readme-strict-v4-ab1a433c8b.v5-code-ready.md | c78c59b982e82c5c219e31389198a23c0dbd51996d392792cd84a4813a8f5cdc |

## Allowed change boundary

- Maintain this normalized previous-provenance Markdown record and its traceability metadata only.
- The historical labels and path tokens in the source-file cell are provenance only. They are not current module, output, implementation, test, workflow, or acceptance names.
- This page must not be used as a current acceptance entry point. Current acceptance authority remains with the repository's current top-level design, normalized maps, stage acceptance prompts, and V1 acceptance evidence matrix.
- Do not create or modify business Rust src, product tests, migrations, API handlers, event schemas, NATS subjects, metrics, workflow code, provider adapters, or state-write paths from this prompt.

## Governance invariants retained

- Authority Contract remains immutable and fork-only; HUMAN_KP and AI_KP remain campaign-level mutually exclusive modes.
- Business and KP services must not call a model directly; AI routes through Agent Gateway, Agent Orchestrator/Runtime, and a Model Provider Adapter.
- AI must not write the database or make unlogged formal rulings; formal decisions pass tools, rules, state services, and the event log through Command -> Workflow -> Decision -> Event Store -> Projection.
- Visibility Label and Fact Provenance remain mandatory across API, events, agent context, tool results, RAG, summaries, exports, replay, logs, and metrics.

## Batch disposition and test responsibility

- Disposition: retain only as previous provenance under the normalized override; it is not a current acceptance entry point and activates no historical implementation proposal.
- Test responsibility: B049 checks must verify the exact overridden target and `previous_provenance` module, Prompt ID, prompt and provenance paths, source SHA256, non-current acceptance boundary, map agreement, and Markdown table shape.
