# Codex Batch 00 — P2 Docs Install / Prep Gate

You are in TRPG_PLATFORM. This is the compatibility entry for P2 B00. Repair only P2 documentation entry points, repo prompt references, and Git staging state. Do not implement P2 runtime features.

## Read first

- `CODEX_P2_MASTER_PROMPT.md`
- `docs/p2/INDEX.md`
- `docs/p2/00_EXECUTION_RULES.md`
- `docs/p2/02_BATCH_PLAN.md`
- `prompts/codex/P2_CHECK_COMMANDS.md`

## Tasks

1. Ensure README contains the current P2 Codex reading order.
2. Ensure `prompts/codex/P2_BATCH_*.md` points to the persisted P2 docs listed in `docs/p2/INDEX.md`.
3. Ensure legacy prompt filenames safely redirect to the current batch order and do not skip B04 Rig Agent Engine.
4. Ensure the P2 docs/status/session-start/master-prompt files are tracked or staged.
5. Run doc-only hygiene checks plus `cargo metadata --no-deps`.

## Out of scope

- Do not edit `.env.example`, env/config, auth, health, CSRF, license, database, API, frontend, or runtime files.
- Do not add RAG APIs, migrations, provider calls, OpenAPI routes, or frontend UI.

## Acceptance

- README points to `CODEX_P2_MASTER_PROMPT.md`, `docs/p2/INDEX.md`, `00_EXECUTION_RULES.md`, and `02_BATCH_PLAN.md`.
- No prompt references deleted P2 document names.
- B01-B07 prompts map to the current P2 docs.
- The required P2 document group is staged.
- Status summary lists exact commands run.
