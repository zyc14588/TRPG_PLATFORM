# Supplemental Requirement Merge

- Prompt ID: `CODEX-0437-03-RUNTIME-ORCHESTRATION-4c408c3ac7`
- Prompt file: `codex-prompts/03-runtime-orchestration/P0112.md`
- Primary Prompt: `CODEX-0036-03-RUNTIME-ORCHESTRATION-12a9414c48`
- Current module: `runtime_orchestration::saga_transaction`
- Status: pending merge by primary prompt; no BATCH-016 Rust output claimed.

Merge instructions for the primary prompt:

- Saga transaction boundaries must keep event append, outbox intent, and compensation audit consistent.
- Compensation must never rewrite prior canonical events; it must append correction or rollback events.
- DLQ/retry handling must keep idempotency, visibility, and provenance intact.

Suggested test assertions for the primary prompt:

- Mid-saga failure either commits no formal state or commits only approved compensation events.
- Retried saga steps do not duplicate outbox or canonical events.
- Restricted facts remain redacted in saga failure logs and compensation summaries.

This supplemental prompt remains traceability-only and does not declare a Rust source, test, migration, API handler, event schema, NATS subject, metric, workflow, or formal write path.
