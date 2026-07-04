# Supplemental Requirement Merge

- Prompt ID: `CODEX-0429-03-RUNTIME-ORCHESTRATION-d740d8b678`
- Prompt file: `codex-prompts/03-runtime-orchestration/P0108.md`
- Primary Prompt: `CODEX-0388-03-RUNTIME-ORCHESTRATION-705a854eb2`
- Current module: `runtime_orchestration::realtime_room_sync_impl`
- Status: pending merge by primary prompt; no BATCH-016 Rust output claimed.

Merge instructions for the primary prompt:

- Realtime room sync must publish only committed-event or rebuildable-projection derived deltas.
- Room, scene, party, and private-player visibility scopes must be enforced before delivery.
- Reconnect/replay must preserve event order, correlation, causation, visibility, and provenance.

Suggested test assertions for the primary prompt:

- Private scene deltas do not cross to other groups during split-party play.
- Reconnect resumes from canonical events without creating new formal events.
- Unauthorized principals receive redacted realtime payloads.

This supplemental prompt remains traceability-only and does not declare a Rust source, test, migration, API handler, event schema, NATS subject, metric, workflow, or formal write path.
