# Supplemental Requirement Merge

- Prompt ID: `CODEX-0580-05-RULESET-COC7-60e84aa01c`
- Prompt file: `codex-prompts/05-ruleset-coc7/P0061.md`
- Primary Prompt: `CODEX-0053-05-RULESET-COC7-a7b7514fc9`
- Current module: `ruleset_coc7::investigation_clue_npc_time`
- Status: pending merge by primary prompt; no BATCH-023 Rust output claimed.

Merge instructions for the primary prompt:

- Core clues must remain reachable through fail-forward alternatives after failed checks.
- NPC secrets, keeper truth, private scene state, and timed events must carry visibility labels and fact provenance.
- Scene time and threat timers must be event-derived and replayable; projections are not canonical.

Suggested test assertions for the primary prompt:

- A failed clue check changes cost, time, exposure, or danger but does not permanently orphan a core clue.
- NPC private facts are redacted from player-facing scene deltas, summaries, exports, RAG, and replay.
- Timer and scheduled-event replay rebuilds the same state from canonical events.

This supplemental prompt remains traceability-only and does not declare a Rust source, test, migration, API handler, event schema, NATS subject, metric, workflow, or formal write path.

