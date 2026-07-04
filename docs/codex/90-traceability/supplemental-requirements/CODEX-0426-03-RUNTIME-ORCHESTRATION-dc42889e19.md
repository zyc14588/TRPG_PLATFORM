# Supplemental Requirement Merge

- Prompt ID: `CODEX-0426-03-RUNTIME-ORCHESTRATION-dc42889e19`
- Prompt file: `codex-prompts/03-runtime-orchestration/P0099.md`
- Primary Prompt: `CODEX-0039-03-RUNTIME-ORCHESTRATION-99d8270e66`
- Current module: `runtime_orchestration::workflow_engine`
- Status: pending merge by primary prompt; no BATCH-016 Rust output claimed.

Merge instructions for the primary prompt:

- Workflow commands must enter formal state only through Workflow -> Decision -> Event Store -> Projection.
- Reject authority-mode mismatches before event append; no event may be appended for denied workflow commands.
- Preserve idempotency key, expected version, actor, correlation id, causation id, Visibility Label, and Fact Provenance on every workflow transition.

Suggested test assertions for the primary prompt:

- HUMAN_KP AI workflow output remains draft-only and requires human confirmation.
- AI_KP formal workflow decisions still pass rules, tool, state, and event-log checks before commit.
- Keeper-only and private facts remain redacted during replay, projection, and realtime deltas.

This supplemental prompt remains traceability-only and does not declare a Rust source, test, migration, API handler, event schema, NATS subject, metric, workflow, or formal write path.
