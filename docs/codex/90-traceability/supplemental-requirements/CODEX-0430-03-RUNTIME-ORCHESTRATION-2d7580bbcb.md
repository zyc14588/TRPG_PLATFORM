# Supplemental Requirement Merge

- Prompt ID: `CODEX-0430-03-RUNTIME-ORCHESTRATION-2d7580bbcb`
- Prompt file: `codex-prompts/03-runtime-orchestration/P0103.md`
- Primary Prompt: `CODEX-0389-03-RUNTIME-ORCHESTRATION-1b60a8b386`
- Current module: `runtime_orchestration::saga_transaction_impl`
- Status: pending merge by primary prompt; no BATCH-016 Rust output claimed.

Merge instructions for the primary prompt:

- Saga steps must not treat projections, caches, or external side effects as canon.
- Compensation and rollback must be evented, idempotent, and auditable.
- Retry and failure handling must preserve actor, expected version, correlation id, causation id, visibility, and provenance.

Suggested test assertions for the primary prompt:

- Failed saga steps append only approved compensation/audit events.
- Retried compensation does not duplicate canonical state changes.
- Policy or authority denial stops the saga before formal state mutation.

This supplemental prompt remains traceability-only and does not declare a Rust source, test, migration, API handler, event schema, NATS subject, metric, workflow, or formal write path.
