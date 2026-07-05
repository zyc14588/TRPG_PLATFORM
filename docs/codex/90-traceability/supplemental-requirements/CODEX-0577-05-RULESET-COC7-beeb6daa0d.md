# Supplemental Requirement Merge

- Prompt ID: `CODEX-0577-05-RULESET-COC7-beeb6daa0d`
- Prompt file: `codex-prompts/05-ruleset-coc7/P0058.md`
- Primary Prompt: `CODEX-0050-05-RULESET-COC7-732771fbed`
- Current module: `ruleset_coc7::chase_state_machine`
- Status: pending merge by primary prompt; no BATCH-023 Rust output claimed.

Merge instructions for the primary prompt:

- Chase transitions must be deterministic, replayable, authority-checked, and linked to canonical events.
- Chase consequences must preserve visibility and fact provenance for split-party and private-scene state.
- Chase obstacle, distance-band, and failure-cost handling must not make a core clue permanently unreachable from one failed roll.

Suggested test assertions for the primary prompt:

- Invalid authority, stale expected_version, and duplicate idempotency_key attempts append no chase event.
- Private chase state does not leak to unrelated scene participants or exports.
- Failed chase checks can add cost, time, exposure, or risk without blocking required progress forever.

This supplemental prompt remains traceability-only and does not declare a Rust source, test, migration, API handler, event schema, NATS subject, metric, workflow, or formal write path.

