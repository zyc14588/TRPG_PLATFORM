# Supplemental Requirement Merge

- Prompt ID: `CODEX-0433-03-RUNTIME-ORCHESTRATION-06ff6db718`
- Prompt file: `codex-prompts/03-runtime-orchestration/P0102.md`
- Primary Prompt: `CODEX-0392-03-RUNTIME-ORCHESTRATION-1cb6fb735e`
- Current module: `runtime_orchestration::workflow_engine_impl`
- Status: pending merge by primary prompt; no BATCH-016 Rust output claimed.

Merge instructions for the primary prompt:

- Workflow implementation must centralize command validation before decisions and event append.
- Expected-version conflict, idempotency replay, authority denial, and policy denial must be distinct and auditable.
- Workflow implementation must not call model providers or write formal state outside the Decision Commit Pipeline.

Suggested test assertions for the primary prompt:

- Idempotency replay returns the original committed outcome without duplicating events.
- Authority and policy denials leave Event Store unchanged.
- Visibility and provenance survive workflow replay and projection rebuild.

This supplemental prompt remains traceability-only and does not declare a Rust source, test, migration, API handler, event schema, NATS subject, metric, workflow, or formal write path.
