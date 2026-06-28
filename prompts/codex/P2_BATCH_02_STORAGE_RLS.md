# Codex Batch 02 — Storage, Migrations, and RLS

Start only after Batch 01 is green.

## Read first

- `CODEX_P2_MASTER_PROMPT.md`
- `docs/p2/INDEX.md`
- `docs/p2/00_EXECUTION_RULES.md`
- `docs/p2/02_BATCH_PLAN.md`
- `docs/p2/04_STORAGE_RLS_DATABASE.md`
- `docs/p2/11_DATABASE_SETUP.md`
- existing migrations in `migrations/`

## Tasks

1. Inspect existing RAG migrations and storage code. Do not duplicate tables if they already exist.
2. Add additive migration(s) for missing columns, constraints, indexes, and RLS policies.
3. Implement storage repository contracts for sources, documents, chunks, ingest jobs, review, and retrieval.
4. Make ingest writes transactional and idempotent.
5. Add direct DB/RLS tests for license and visibility boundaries.
6. Ensure retrieval queries filter license and visibility before scoring.

## Constraints

- No route implementation except test helper code if absolutely necessary.
- Do not use `postgres` superuser as app login in tests that claim to verify RLS.
- Keep migration reversible only if project convention supports down migrations; otherwise document forward-only behavior.

## Checks

```powershell
cargo fmt --all --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p storage
cargo sqlx migrate run
cargo sqlx prepare --check --workspace
```

## Completion response

List migration names, repository methods, RLS tests, and any DB setup assumptions.
