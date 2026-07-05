# Supplemental Requirement Merge

- Prompt ID: `CODEX-0584-05-RULESET-COC7-14b91df550`
- Prompt file: `codex-prompts/05-ruleset-coc7/P0065.md`
- Primary Prompt: `CODEX-0557-05-RULESET-COC7-beb3672ec7`
- Current module: `ruleset_coc7::ruleset_pack_sdk`
- Status: pending merge by primary prompt; no BATCH-023 Rust output claimed.

Merge instructions for the primary prompt:

- Ruleset pack SDK extensions must not bypass Tool Grant, Policy Gate, Authority Contract, server dice, or Event Store.
- SDK-provided rules metadata must carry stable current-safe identifiers and must not promote historical source paths or hashes.
- Extension behavior must preserve visibility and fact provenance across validation, replay, export, and audit surfaces.

Suggested test assertions for the primary prompt:

- Extension hooks cannot directly append canonical events or mutate Authority Contract.
- SDK rule outputs preserve visibility and provenance labels.
- Historical source-derived identifiers are rejected from current rule pack IDs, event names, metrics, and workflow names.

This supplemental prompt remains traceability-only and does not declare a Rust source, test, migration, API handler, event schema, NATS subject, metric, workflow, or formal write path.

