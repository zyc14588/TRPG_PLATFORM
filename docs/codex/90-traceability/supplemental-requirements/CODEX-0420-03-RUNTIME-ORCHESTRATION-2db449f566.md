# Supplemental Requirement Merge

- Prompt ID: `CODEX-0420-03-RUNTIME-ORCHESTRATION-2db449f566`
- Prompt file: `codex-prompts/03-runtime-orchestration/P0093.md`
- Primary Prompt: `CODEX-0358-03-RUNTIME-ORCHESTRATION-5626fcbd5c`
- Current module: `runtime_orchestration::readme`
- Status: pending merge by primary prompt; no BATCH-015 Rust output claimed.

Merge instructions for the primary prompt:

- Document runtime orchestration as the current-safe owner for Command -> Workflow -> Decision -> Event Store -> Projection flow.
- State that projections, cache, RAG index, summary, realtime views, and exports are rebuildable read models.
- State that AI capabilities must pass through Agent Gateway, Agent Runtime, and Model Provider Adapter boundaries.
- State that source-archive paths are provenance-only and cannot define current module, workflow, event, metric, or test names.

Suggested test assertions for the primary prompt:

- README examples do not imply direct LLM calls, direct agent writes, or projection-as-canon behavior.
- Runtime examples include authority mode, visibility, provenance, idempotency, and event-log evidence.
- Cross-references point to current-safe names rather than legacy V3/V4/V5/V6 tokens.

This supplemental prompt remains traceability-only and does not declare a Rust source, test, migration, API handler, event schema, NATS subject, metric, workflow, or formal write path.
