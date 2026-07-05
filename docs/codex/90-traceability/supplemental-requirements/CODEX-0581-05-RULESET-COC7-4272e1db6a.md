# Supplemental Requirement Merge

- Prompt ID: `CODEX-0581-05-RULESET-COC7-4272e1db6a`
- Prompt file: `codex-prompts/05-ruleset-coc7/P0062.md`
- Primary Prompt: `CODEX-0554-05-RULESET-COC7-f26507ac8a`
- Current module: `ruleset_coc7::readme`
- Status: pending merge by primary prompt; no BATCH-023 Rust output claimed.

Merge instructions for the primary prompt:

- README-derived ruleset contract must remain an executable governance summary, not a second source of game-state truth.
- COC7-specific fields must stay inside the COC7 ruleset crate and must not pollute ruleset-agnostic core types.
- The module must continue documenting server dice, fail-forward clues, SAN, combat, chase, and replay boundaries.

Suggested test assertions for the primary prompt:

- README contract exposes COC7 scope and rejects non-COC7 room assumptions for V1.
- README contract does not authorize direct provider, database, or event-store bypass paths.
- Documentation-derived checks map to existing ruleset contract tests.

This supplemental prompt remains traceability-only and does not declare a Rust source, test, migration, API handler, event schema, NATS subject, metric, workflow, or formal write path.

