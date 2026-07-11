# Traceability manifest

> BATCH-049 current-safe traceability output. This page is documentation evidence only; it is not a Rust module, migration, API contract, event schema, NATS subject, metric, workflow, product test, or formal state-write owner.

## Current-safe target

- Batch: BATCH-049-90-traceability — Strict Governance Final
- Output role: documentation-or-traceability
- Task type: traceability-maintenance
- Current output: docs/codex/90-traceability/manifest.md

| Prompt ID | Prompt file | Current crate | Current module | Source file (provenance only) | Source SHA256 |
|---|---|---|---|---|---|
| CODEX-1055-90-TRACEABILITY-964c983038 | codex-prompts/90-traceability/P0088.md | trpg-docs-governance | traceability::manifest | docs/implementation/90-traceability/per-file-code-ready/90-traceability/manifest-c4e2b7edb5.v5-code-ready.md | c2f59f850942323ebbfe47a43d5ec4df0b6259c3268abd60dd077054120f2cd1 |

## Allowed change boundary

- Maintain this current-safe Markdown manifest trace and its metadata only; this page does not assert a broader source inventory or execution history.
- Do not create or modify business Rust src, product tests, migrations, API handlers, event schemas, NATS subjects, metrics, workflow code, provider adapters, or state-write paths from this prompt.
- Treat historical version labels, hashes, and source path fragments as provenance only; do not promote source proposals or names into current implementation or acceptance.

## Governance invariants retained

- Authority Contract remains immutable and fork-only; HUMAN_KP and AI_KP remain campaign-level mutually exclusive modes.
- Business and KP services must not call a model directly; AI routes through Agent Gateway, Agent Orchestrator/Runtime, and a Model Provider Adapter.
- AI must not write the database or make unlogged formal rulings; formal decisions pass tools, rules, state services, and the event log through Command -> Workflow -> Decision -> Event Store -> Projection.
- Visibility Label and Fact Provenance remain mandatory across API, events, agent context, tool results, RAG, summaries, exports, replay, logs, and metrics.

## Batch disposition and test responsibility

- Disposition: retain as the B049 docs-only current-safe target; this record contains only its mapped prompt row and does not fabricate a source manifest or execution history.
- Test responsibility: B049 checks must verify the H1, Prompt ID, current-safe target/module, prompt and provenance paths, source SHA256, map agreement, Markdown table shape, and docs-only boundary.

<!-- BATCH-050-START -->
## BATCH-050 current-safe manifest trace

This additive section records only the current-safe B050 manifest mapping; it
does not replace the B049 record or assert a broader source inventory or
execution history.

- Batch: BATCH-050-90-traceability — Strict Governance Final
- Output role: documentation-or-traceability
- Task type: traceability-maintenance
- Current output: docs/codex/90-traceability/manifest.md

| Prompt ID | Prompt file | Current crate | Current module | Source file (provenance only) | Source SHA256 |
|---|---|---|---|---|---|
| `CODEX-1070-90-TRACEABILITY-90042976ac` | `codex-prompts/90-traceability/P0103.md` | `trpg-docs-governance` | `traceability::manifest` | `docs/implementation/90-traceability/per-file-code-ready/90-traceability/sources-v3-baseline-document-group-manifest-16c5a85699.v5-code-ready.md` | `4b6db1e7c4a42d8443d61d9cd0b03e4bf64c02310f964f983f5eed4a1b4fae1e` |

### Disposition and test responsibility

- Disposition: retain this row as additive docs-only current-safe
  traceability; historical paths, versions, and hashes remain provenance only,
  and this prompt owns no Rust, migration, API, event, NATS, metric, workflow,
  provider, product-test, or formal state-write output.
- Test responsibility: B050 checks must verify the H1, Prompt ID, current-safe
  target/module, prompt and provenance paths, source SHA256, map agreement,
  Markdown table shape, and docs-only boundary.
<!-- BATCH-050-END -->
