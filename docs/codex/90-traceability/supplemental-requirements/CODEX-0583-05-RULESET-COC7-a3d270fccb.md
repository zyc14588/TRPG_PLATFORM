# Supplemental Requirement Merge

- Prompt ID: `CODEX-0583-05-RULESET-COC7-a3d270fccb`
- Prompt file: `codex-prompts/05-ruleset-coc7/P0064.md`
- Primary Prompt: `CODEX-0055-05-RULESET-COC7-1dab95f953`
- Current module: `ruleset_coc7::sanity_madness_state_machine`
- Status: pending merge by primary prompt; no BATCH-023 Rust output claimed.

Merge instructions for the primary prompt:

- SAN loss, temporary madness, indefinite madness, phobias, manias, and recovery transitions must be event-derived and auditable.
- Hallucination or madness narration must not reveal keeper-only facts unless a governed event has made them visible.
- SAN state changes must link to server-side rolls and preserve visibility/fact provenance.

Suggested test assertions for the primary prompt:

- SAN loss cannot be applied from caller-supplied official dice values.
- Madness state replay rebuilds identical public and restricted views.
- Private madness notes are redacted from unauthorized exports, summaries, RAG, and replay.

This supplemental prompt remains traceability-only and does not declare a Rust source, test, migration, API handler, event schema, NATS subject, metric, workflow, or formal write path.

