# Supplemental Requirement Merge

- Prompt ID: `CODEX-0435-03-RUNTIME-ORCHESTRATION-b56967a4fb`
- Prompt file: `codex-prompts/03-runtime-orchestration/P0110.md`
- Primary Prompt: `CODEX-0033-03-RUNTIME-ORCHESTRATION-0d6882e9c6`
- Current module: `runtime_orchestration::pending_decision`
- Status: pending merge by primary prompt; no BATCH-016 Rust output claimed.

Merge instructions for the primary prompt:

- Pending decision records must capture proposal source, authority mode, required confirmer, visibility, and provenance.
- Draft decisions cannot mutate formal state until confirmed through the approved workflow path.
- Rejection, expiry, and appeal must be append-only and must not erase prior decision history.

Suggested test assertions for the primary prompt:

- HUMAN_KP confirmation is required before AI-authored draft decisions can commit.
- Rejected or expired decisions cannot be reused to commit later state.
- Appeal/review creates new events instead of editing previous decisions.

This supplemental prompt remains traceability-only and does not declare a Rust source, test, migration, API handler, event schema, NATS subject, metric, workflow, or formal write path.
