# Supplemental Requirement Merge

- Prompt ID: `CODEX-0582-05-RULESET-COC7-f9d12478d9`
- Prompt file: `codex-prompts/05-ruleset-coc7/P0063.md`
- Primary Prompt: `CODEX-0054-05-RULESET-COC7-2db5ee1d9e`
- Current module: `ruleset_coc7::rules_coc7`
- Status: pending merge by primary prompt; no BATCH-023 Rust output claimed.

Merge instructions for the primary prompt:

- Ruleset dispatch must keep V1 rooms restricted to `ruleset_id=coc7` unless a later primary prompt explicitly opens extension behavior.
- Skill checks, SAN, combat, chase, and character rules must route through server rules logic and canonical event records.
- Rule metadata must not include historical source paths, version labels, or hash fragments as current product identifiers.

Suggested test assertions for the primary prompt:

- Non-COC7 room/ruleset inputs are rejected by V1 dispatch.
- Rule outcomes preserve linked roll, decision, authority, visibility, and provenance records.
- Current identifiers are stable and free of historical source-path/hash tokens.

This supplemental prompt remains traceability-only and does not declare a Rust source, test, migration, API handler, event schema, NATS subject, metric, workflow, or formal write path.

