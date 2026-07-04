# Supplemental Requirement Merge

- Prompt ID: `CODEX-0432-03-RUNTIME-ORCHESTRATION-b0e45095dc`
- Prompt file: `codex-prompts/03-runtime-orchestration/P0107.md`
- Primary Prompt: `CODEX-0391-03-RUNTIME-ORCHESTRATION-daba262944`
- Current module: `runtime_orchestration::session_runtime_impl`
- Status: pending merge by primary prompt; no BATCH-016 Rust output claimed.

Merge instructions for the primary prompt:

- Session lifecycle changes must flow through command, workflow decision, event store, and projection.
- Session resume must rebuild from canonical events and must not treat cached or projected state as canon.
- Runtime output must preserve immutable Authority Contract, KP-mode exclusivity, visibility, and provenance.

Suggested test assertions for the primary prompt:

- Session resume reconstructs state from events without appending new formal events.
- HUMAN_KP AI output is draft-only across session start, resume, and pending-decision flows.
- Restricted facts are excluded from unauthorized session, realtime, and agent-context views.

This supplemental prompt remains traceability-only and does not declare a Rust source, test, migration, API handler, event schema, NATS subject, metric, workflow, or formal write path.
