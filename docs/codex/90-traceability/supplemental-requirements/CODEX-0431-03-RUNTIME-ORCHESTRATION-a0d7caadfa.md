# Supplemental Requirement Merge

- Prompt ID: `CODEX-0431-03-RUNTIME-ORCHESTRATION-a0d7caadfa`
- Prompt file: `codex-prompts/03-runtime-orchestration/P0106.md`
- Primary Prompt: `CODEX-0390-03-RUNTIME-ORCHESTRATION-12323c9bd9`
- Current module: `runtime_orchestration::scheduler_service_impl`
- Status: pending merge by primary prompt; no BATCH-016 Rust output claimed.

Merge instructions for the primary prompt:

- Scheduled work must re-check authority, policy, visibility, and expected version at execution time.
- Scheduler timeout/retry paths must be idempotent and must not create duplicate canonical events.
- Scheduled commands must carry actor, correlation id, causation id, visibility, and provenance from request through commit.

Suggested test assertions for the primary prompt:

- A command scheduled under one authority mode is denied if the current campaign authority does not match at execution.
- Timeout and retry produce one canonical result and auditable retry records.
- Scheduler-generated realtime/projection updates redact restricted facts.

This supplemental prompt remains traceability-only and does not declare a Rust source, test, migration, API handler, event schema, NATS subject, metric, workflow, or formal write path.
