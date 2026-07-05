# Supplemental Requirement Merge

- Prompt ID: `CODEX-0579-05-RULESET-COC7-ad43ae3bfd`
- Prompt file: `codex-prompts/05-ruleset-coc7/P0060.md`
- Primary Prompt: `CODEX-0052-05-RULESET-COC7-93d0bf85e3`
- Current module: `ruleset_coc7::dice_roll_contract`
- Status: pending merge by primary prompt; no BATCH-023 Rust output claimed.

Merge instructions for the primary prompt:

- All formal rolls must be server-generated and recorded with roll identity, formula, raw result, final result, visibility, and linked decision.
- AI, KP, and frontend inputs may request rolls but must not supply official roll results.
- Bonus, penalty, pushed, luck, opposed, and hidden rolls must be replayable and auditable without leaking restricted visibility.

Suggested test assertions for the primary prompt:

- Caller-supplied official roll results are rejected.
- Roll replay uses stored roll records rather than regenerating dice.
- Hidden or keeper-only rolls redact output while preserving audit evidence.

This supplemental prompt remains traceability-only and does not declare a Rust source, test, migration, API handler, event schema, NATS subject, metric, workflow, or formal write path.

