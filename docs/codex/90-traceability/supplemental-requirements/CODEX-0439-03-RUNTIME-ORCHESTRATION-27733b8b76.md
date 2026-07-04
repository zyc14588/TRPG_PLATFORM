# Supplemental Requirement Merge

- Prompt ID: `CODEX-0439-03-RUNTIME-ORCHESTRATION-27733b8b76`
- Prompt file: `codex-prompts/03-runtime-orchestration/P0114.md`
- Primary Prompt: `CODEX-0038-03-RUNTIME-ORCHESTRATION-ec0e699332`
- Current module: `runtime_orchestration::session_runtime`
- Status: pending merge by primary prompt; no BATCH-016 Rust output claimed.

Merge instructions for the primary prompt:

- Session runtime must enforce immutable campaign Authority Contract for session start, resume, pause, and close.
- Formal session state changes must be command-driven and event-backed.
- AI, plugins, handlers, and providers must not bypass runtime workflow or directly write session state.

Suggested test assertions for the primary prompt:

- Authority-mode mismatch prevents formal session mutation and leaves Event Store unchanged.
- Session close/resume replay produces deterministic projection state.
- Restricted session facts do not leak into player-visible session summaries or realtime updates.

This supplemental prompt remains traceability-only and does not declare a Rust source, test, migration, API handler, event schema, NATS subject, metric, workflow, or formal write path.
