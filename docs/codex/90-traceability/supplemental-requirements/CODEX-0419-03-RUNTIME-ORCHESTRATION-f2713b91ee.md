# Supplemental Requirement Merge

- Prompt ID: `CODEX-0419-03-RUNTIME-ORCHESTRATION-f2713b91ee`
- Prompt file: `codex-prompts/03-runtime-orchestration/P0100.md`
- Primary Prompt: `CODEX-0033-03-RUNTIME-ORCHESTRATION-0d6882e9c6`
- Current module: `runtime_orchestration::pending_decision`
- Status: pending merge by primary prompt; no BATCH-015 Rust output claimed.

Merge instructions for the primary prompt:

- Preserve pending decision as a workflow-owned gate between AI/KP proposals and event-store commits.
- In HUMAN_KP mode, AI output must remain draft-only and require explicit human confirmation.
- In AI_KP mode, formal decisions still require rules/tool/state checks before event append.
- Keep idempotency key, expected version, actor, correlation id, causation id, Visibility Label, and Fact Provenance on every commit path.

Suggested test assertions for the primary prompt:

- Draft decisions cannot mutate formal state.
- Duplicate confirmation attempts are idempotent and do not append duplicate canonical events.
- Private, keeper-only, and system-only facts do not leak into public decision views.

This supplemental prompt remains traceability-only and does not declare a Rust source, test, migration, API handler, event schema, NATS subject, metric, workflow, or formal write path.
