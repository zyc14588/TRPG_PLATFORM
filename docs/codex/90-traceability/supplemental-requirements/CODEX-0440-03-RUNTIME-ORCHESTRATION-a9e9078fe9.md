# Supplemental Requirement Merge

- Prompt ID: `CODEX-0440-03-RUNTIME-ORCHESTRATION-a9e9078fe9`
- Prompt file: `codex-prompts/03-runtime-orchestration/P0115.md`
- Primary Prompt: `CODEX-0039-03-RUNTIME-ORCHESTRATION-99d8270e66`
- Current module: `runtime_orchestration::workflow_engine`
- Status: pending merge by primary prompt; no BATCH-016 Rust output claimed.

Merge instructions for the primary prompt:

- Workflow engine must keep workflow state, pending decisions, and event commits aligned to the same command envelope.
- Formal workflow results must be explainable and audit-linked without exposing restricted facts to unauthorized principals.
- Workflow errors must distinguish authority violation, policy denial, version conflict, idempotency replay, and visibility denial.

Suggested test assertions for the primary prompt:

- Workflow decisions include player-visible explanation plus keeper/audit records with correct visibility labels.
- Version conflict and idempotency replay do not create duplicate canonical events.
- Direct LLM/provider access is absent from workflow implementation.

This supplemental prompt remains traceability-only and does not declare a Rust source, test, migration, API handler, event schema, NATS subject, metric, workflow, or formal write path.
