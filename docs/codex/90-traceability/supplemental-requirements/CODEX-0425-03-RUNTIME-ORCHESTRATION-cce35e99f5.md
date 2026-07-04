# Supplemental Requirement Merge

- Prompt ID: `CODEX-0425-03-RUNTIME-ORCHESTRATION-cce35e99f5`
- Prompt file: `codex-prompts/03-runtime-orchestration/P0097.md`
- Primary Prompt: `CODEX-0038-03-RUNTIME-ORCHESTRATION-ec0e699332`
- Current module: `runtime_orchestration::session_runtime`
- Status: pending merge by primary prompt; no BATCH-015 Rust output claimed.

Merge instructions for the primary prompt:

- Session lifecycle changes must flow through command, workflow decision, event store, and projection.
- Session runtime must enforce immutable Authority Contract and campaign-level HUMAN_KP / AI_KP exclusivity.
- Visibility Label and Fact Provenance must survive session resume, export, replay, realtime sync, and agent context construction.
- Reject direct database writes, direct LLM/provider calls, and agent-authored formal state changes outside the approved runtime path.

Suggested test assertions for the primary prompt:

- Session resume reconstructs state from canonical events and rebuildable projections.
- Authority mode mismatch prevents formal session changes without mutating state.
- Restricted session facts are excluded from unauthorized realtime and agent-context views.

This supplemental prompt remains traceability-only and does not declare a Rust source, test, migration, API handler, event schema, NATS subject, metric, workflow, or formal write path.
