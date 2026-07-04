# Supplemental Requirement Merge

- Prompt ID: `CODEX-0421-03-RUNTIME-ORCHESTRATION-69905634c4`
- Prompt file: `codex-prompts/03-runtime-orchestration/P0094.md`
- Primary Prompt: `CODEX-0034-03-RUNTIME-ORCHESTRATION-20e1521d8e`
- Current module: `runtime_orchestration::realtime_runtime_binding`
- Status: pending merge by primary prompt; no BATCH-015 Rust output claimed.

Merge instructions for the primary prompt:

- Treat realtime binding as a projection/event consumer, not a formal state writer.
- Apply visibility filtering before room fan-out, reconnect replay, and delta delivery.
- Preserve Fact Provenance, correlation id, and causation id on realtime envelopes.
- Reject realtime updates that bypass workflow decisions or event-store append evidence.

Suggested test assertions for the primary prompt:

- Keeper-only or private deltas are withheld from unauthorized room participants.
- Reconnect replay is deterministic from event/projection state and does not invent state.
- Realtime envelopes expose provenance without exposing restricted content.

This supplemental prompt remains traceability-only and does not declare a Rust source, test, migration, API handler, event schema, NATS subject, metric, workflow, or formal write path.
