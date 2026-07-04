# Supplemental Requirement Merge

- Prompt ID: `CODEX-0438-03-RUNTIME-ORCHESTRATION-5a764587b1`
- Prompt file: `codex-prompts/03-runtime-orchestration/P0113.md`
- Primary Prompt: `CODEX-0037-03-RUNTIME-ORCHESTRATION-c9bd0a0635`
- Current module: `runtime_orchestration::scheduler_service`
- Status: pending merge by primary prompt; no BATCH-016 Rust output claimed.

Merge instructions for the primary prompt:

- Scheduler service must schedule commands, not direct state mutations.
- Scheduled execution must pass through the same authority, policy, workflow, decision, event, visibility, and provenance path as interactive commands.
- Retry and timeout records must be append-only and auditable.

Suggested test assertions for the primary prompt:

- Scheduled work denied by authority or policy appends no formal game-state event.
- Timeout retry is idempotent and produces one final canonical outcome.
- Scheduler observability redacts restricted payload fields.

This supplemental prompt remains traceability-only and does not declare a Rust source, test, migration, API handler, event schema, NATS subject, metric, workflow, or formal write path.
