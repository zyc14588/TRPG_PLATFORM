# Supplemental Requirement Merge

- Prompt ID: `CODEX-0422-03-RUNTIME-ORCHESTRATION-fefdae1a01`
- Prompt file: `codex-prompts/03-runtime-orchestration/P0101.md`
- Primary Prompt: `CODEX-0035-03-RUNTIME-ORCHESTRATION-2f52cb37ae`
- Current module: `runtime_orchestration::runtime_state_machines`
- Status: pending merge by primary prompt; no BATCH-015 Rust output claimed.

Merge instructions for the primary prompt:

- Model runtime state transitions as validated workflow outcomes, not direct mutations.
- Reject illegal transitions, stale expected versions, authority mismatches, and missing provenance.
- Keep projections rebuildable from event-store facts.
- Keep AI/KP outputs subject to rules, tool, state, event-log, visibility, and provenance gates.

Suggested test assertions for the primary prompt:

- Illegal transition attempts return structured rejection without appending events.
- Valid transitions include idempotency key, actor, expected version, correlation id, causation id, visibility, and provenance.
- Replay reconstructs the same state machine state from canonical events.

This supplemental prompt remains traceability-only and does not declare a Rust source, test, migration, API handler, event schema, NATS subject, metric, workflow, or formal write path.
