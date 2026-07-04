# Supplemental Requirement Merge

- Prompt ID: `CODEX-0423-03-RUNTIME-ORCHESTRATION-b2135fac7b`
- Prompt file: `codex-prompts/03-runtime-orchestration/P0095.md`
- Primary Prompt: `CODEX-0036-03-RUNTIME-ORCHESTRATION-12a9414c48`
- Current module: `runtime_orchestration::saga_transaction`
- Status: pending merge by primary prompt; no BATCH-015 Rust output claimed.

Merge instructions for the primary prompt:

- Keep saga steps and compensations idempotent and event-backed.
- Do not let compensation write projections or databases as canonical state.
- Carry Authority Contract, visibility, provenance, idempotency, expected version, correlation id, and causation id through saga steps.
- Reject agent-driven formal writes unless workflow, rules, state, and tool gates approve them.

Suggested test assertions for the primary prompt:

- Retry and compensation paths do not duplicate canonical events.
- Failed saga branches expose auditable rejection facts without leaking restricted content.
- Replayed saga events reconstruct the same transaction outcome.

This supplemental prompt remains traceability-only and does not declare a Rust source, test, migration, API handler, event schema, NATS subject, metric, workflow, or formal write path.
