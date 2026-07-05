# Supplemental Requirement Merge

- Prompt ID: `CODEX-0576-05-RULESET-COC7-c4db17f4ae`
- Prompt file: `codex-prompts/05-ruleset-coc7/P0057.md`
- Primary Prompt: `CODEX-0049-05-RULESET-COC7-abe85bf6eb`
- Current module: `ruleset_coc7::character_combat_san_chase`
- Status: pending merge by primary prompt; no BATCH-023 Rust output claimed.

Merge instructions for the primary prompt:

- Character, combat, SAN, and chase decisions must preserve Authority Contract mode checks and event-store-only canonical writes.
- Visibility labels and fact provenance must survive replay, projection rebuild, summaries, exports, and audit output.
- Historical source snippets, hashes, and version labels must not become current test, event, metric, NATS, workflow, or module names.

Suggested test assertions for the primary prompt:

- HUMAN_KP and AI_KP authority violations return the expected domain error and append no canonical event.
- Keeper-only or private facts are not exposed to unauthorized player, summary, RAG, export, or replay readers.
- Idempotency replay and expected-version conflict do not duplicate character/combat/SAN/chase events.

This supplemental prompt remains traceability-only and does not declare a Rust source, test, migration, API handler, event schema, NATS subject, metric, workflow, or formal write path.

