# P2 Database Status

## Summary

- Last updated: 2026-06-29
- Batch: DB URL Scope Clean Repair
- Result: PASS for DB env scope cleanup
- DB environment contract: PASS
- Full SQLx prepare gate: BLOCKED by deferred B02 storage/source-kind work

## Scope cleanup

Removed from the current DB env repair diff:

- `crates/storage/src/lib.rs`
- `migrations/20260629010000_p2_b02_source_kind_alignment.sql`
- `docs/p2/11_DATABASE_SETUP.md`
- `docs/p2/15_DB_TEST_MIGRATOR_POLICY.md`
- `docs/p2/INDEX.md`

Deferred outside the repository:

- `C:\Users\zyc14588\_trpg_codex_deferred\20260629_033022_B02_storage_worktree.patch`
- `C:\Users\zyc14588\_trpg_codex_deferred\20260629_033022_B02_storage_index.patch` (empty; no staged storage diff existed)
- `C:\Users\zyc14588\_trpg_codex_deferred\20260629_033022_B02_out_of_scope_docs_worktree.patch`
- `C:\Users\zyc14588\_trpg_codex_deferred\20260629_033022_B02_source_kind_alignment_untracked.sql`

Reason: this batch is DB environment only. B02 storage/source-kind/migration changes must move to the next dedicated repair branch.

## DATABASE_URL contract

- `DATABASE_URL` is non-empty after dot-sourcing `scripts/dev/db/env.ps1`.
- `DATABASE_URL` uses ordinary app role `trpg_app`, not the `postgres` superuser.
- Admin, test migrator, and bootstrap URLs remain separate:
  - `TRPG_DATABASE_ADMIN_URL`
  - `TRPG_TEST_MIGRATOR_DATABASE_URL`
  - `TRPG_DATABASE_BOOTSTRAP_URL`
- `.env.example` and `scripts/dev/db/env.ps1` keep one DB variable per line.

## PowerShell execution policy

- `MachinePolicy`, `UserPolicy`, `CurrentUser`, and `LocalMachine` were `Undefined`.
- `Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass -Force` worked for validation shells.
- No `PSSecurityException` occurred during this repair.
- No persistent `CurrentUser` or `LocalMachine` policy was changed.

## Verification

| Command | Result | Notes |
|---|---|---|
| `git status --short` | PASS | Initial state included storage and migration out-of-scope changes; final state removes forbidden paths. |
| `git diff --name-only` plus `git ls-files --others --exclude-standard` forbidden-path check | PASS | No tracked or untracked forbidden paths remain. |
| Static multiline check for `.env.example` and `scripts/dev/db/env.ps1` | PASS | Required DB URLs are one assignment per line and non-empty. |
| `Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass -Force; . .\scripts\dev\db\env.ps1` | PASS | `DATABASE_URL=postgres://trpg_app:trpg_app@127.0.0.1:55432/trpg_platform`. |
| `$env:DATABASE_URL = ""; .\scripts\dev\db\verify.ps1` | PASS | Expected fail-fast: `DATABASE_URL is not set.` |
| `$env:DATABASE_URL = "postgres://postgres:postgres@127.0.0.1:55432/trpg_platform"; .\scripts\dev\db\verify.ps1` | PASS | Expected fail-fast: app URL must not use `postgres`. |
| `docker compose -f docker-compose.dev-db.yml up -d` | PASS | Existing container was running; Docker reported an unrelated orphan container warning. |
| `cargo sqlx migrate run --database-url "$env:TRPG_DATABASE_ADMIN_URL"` | BLOCKED | Local DB has applied migration `20260629010000`, but that migration file is deferred from this scope. No destructive DB reset was performed. |
| `.\scripts\dev\db\grant-app-role.ps1; .\scripts\dev\db\verify.ps1` | PASS | `trpg_app` is not superuser and not `BYPASSRLS`; RLS tables are enabled and forced. |
| `cargo sqlx prepare --check --workspace` | BLOCKED | `crates/storage/src/lib.rs` does not cover deferred `SourceKind::KpPrivateModule` and `SourceKind::SystemInternal`; fix belongs to next B02 storage/source-kind branch. |

## Remaining blockers

- Local PostgreSQL migration ledger includes deferred migration `20260629010000`; use a fresh DB or the B02 branch before rerunning the full migration gate.
- `cargo sqlx prepare --check --workspace` still needs the deferred storage/source-kind repair.

## Next repair branch

- B02 storage/source-kind/harness repair.
