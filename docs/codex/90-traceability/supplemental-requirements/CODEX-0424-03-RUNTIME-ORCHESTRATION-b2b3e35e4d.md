# Supplemental Requirement Merge

- Prompt ID: `CODEX-0424-03-RUNTIME-ORCHESTRATION-b2b3e35e4d`
- Prompt file: `codex-prompts/03-runtime-orchestration/P0096.md`
- Primary Prompt: `CODEX-0037-03-RUNTIME-ORCHESTRATION-c9bd0a0635`
- Current module: `runtime_orchestration::scheduler_service`
- Status: pending merge by primary prompt; no BATCH-015 Rust output claimed.

Merge instructions for the primary prompt:

- Scheduled work must enqueue workflow commands rather than mutate state directly.
- Scheduled commands must carry actor/service identity, authority mode, idempotency key, expected version, correlation id, causation id, visibility, and provenance.
- Retries must be deterministic and must not silently cross privacy boundaries or provider boundaries.
- Reject scheduled AI work unless Agent Gateway, runtime, provider adapter, and tool gates authorize it.

Suggested test assertions for the primary prompt:

- Duplicate scheduler retries do not append duplicate events.
- Missing authority, provenance, or visibility metadata is rejected.
- Scheduler-created outputs are replayable and auditable from event-store facts.

This supplemental prompt remains traceability-only and does not declare a Rust source, test, migration, API handler, event schema, NATS subject, metric, workflow, or formal write path.
