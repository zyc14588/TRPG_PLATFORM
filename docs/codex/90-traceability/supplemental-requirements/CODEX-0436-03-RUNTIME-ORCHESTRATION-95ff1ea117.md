# Supplemental Requirement Merge

- Prompt ID: `CODEX-0436-03-RUNTIME-ORCHESTRATION-95ff1ea117`
- Prompt file: `codex-prompts/03-runtime-orchestration/P0111.md`
- Primary Prompt: `CODEX-0347-03-RUNTIME-ORCHESTRATION-b0e055d98c`
- Current module: `runtime_orchestration::realtime_room_sync`
- Status: pending merge by primary prompt; no BATCH-016 Rust output claimed.

Merge instructions for the primary prompt:

- Realtime room sync must derive player-visible output from committed events and authorized projections only.
- Delta generation must apply visibility checks before serialization, logging, metrics, and broadcast.
- Reconnect cursors must preserve canonical ordering without allowing client-forged state.

Suggested test assertions for the primary prompt:

- Split-party private scene updates are visible only to authorized members and KP roles.
- Reconnect cannot request hidden event payloads by cursor manipulation.
- Broadcast metrics/logs do not include keeper-only or private payload text.

This supplemental prompt remains traceability-only and does not declare a Rust source, test, migration, API handler, event schema, NATS subject, metric, workflow, or formal write path.
