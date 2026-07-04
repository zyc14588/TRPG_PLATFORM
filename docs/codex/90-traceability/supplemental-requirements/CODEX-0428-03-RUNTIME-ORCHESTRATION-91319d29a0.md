# Supplemental Requirement Merge

- Prompt ID: `CODEX-0428-03-RUNTIME-ORCHESTRATION-91319d29a0`
- Prompt file: `codex-prompts/03-runtime-orchestration/P0105.md`
- Primary Prompt: `CODEX-0387-03-RUNTIME-ORCHESTRATION-ff36c2cdcf`
- Current module: `runtime_orchestration::pending_decision_impl`
- Status: pending merge by primary prompt; no BATCH-016 Rust output claimed.

Merge instructions for the primary prompt:

- Pending decisions are the gate between AI/KP proposals and canonical event commits.
- Confirmation, rejection, expiry, and retry must be idempotent and version-checked.
- HUMAN_KP AI proposals must remain draft-only until the human KP confirms them.

Suggested test assertions for the primary prompt:

- Duplicate confirmation attempts do not append duplicate canonical events.
- Expired or rejected pending decisions cannot later mutate formal state.
- Private, keeper-only, and system-only decision facts are redacted from unauthorized views.

This supplemental prompt remains traceability-only and does not declare a Rust source, test, migration, API handler, event schema, NATS subject, metric, workflow, or formal write path.
