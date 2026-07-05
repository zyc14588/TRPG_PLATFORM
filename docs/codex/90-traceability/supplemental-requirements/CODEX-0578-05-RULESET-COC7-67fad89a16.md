# Supplemental Requirement Merge

- Prompt ID: `CODEX-0578-05-RULESET-COC7-67fad89a16`
- Prompt file: `codex-prompts/05-ruleset-coc7/P0059.md`
- Primary Prompt: `CODEX-0051-05-RULESET-COC7-2af517794b`
- Current module: `ruleset_coc7::combat_state_machine`
- Status: pending merge by primary prompt; no BATCH-023 Rust output claimed.

Merge instructions for the primary prompt:

- Combat turns, damage, armor, dodge, counterattack, and defeat state must be derived through governed decisions.
- Combat state changes must carry authority, visibility, fact provenance, idempotency, expected version, correlation, and causation metadata.
- Combat narration or AI suggestions must not create facts without rule/tool/state/event confirmation.

Suggested test assertions for the primary prompt:

- Unauthorized damage or turn mutation returns an authority or policy error and appends no event.
- Replay rebuilds the same combat state and redacts restricted facts for unauthorized principals.
- Dice-derived combat outcomes are linked to server-side roll records.

This supplemental prompt remains traceability-only and does not declare a Rust source, test, migration, API handler, event schema, NATS subject, metric, workflow, or formal write path.

