# P2 DB — DATABASE_URL Scope Clean Repair

## Problem

The `DATABASE_URL` empty blocker may be fixed while the DB env repair batch still fails because its diff includes out-of-scope storage/business logic changes.

This document defines the clean repair contract for the DB environment setup batch.

## Required outcome

A clean DB environment repair branch may change only database setup/runtime environment support files and status documentation.

It must not carry B02 storage/domain/API/frontend business logic changes.

## Allowed files for this repair

Allowed:

- `.env.example`
- `docker-compose.dev-db.yml`
- `scripts/dev/db/*.ps1`
- `docs/p2/db/**`
- `docs/status/P2_DATABASE_STATUS.md`
- `docs/status/P2_DATABASE_STATUS_TEMPLATE.md`
- `CODEX_DB_MASTER_PROMPT.md`
- `.codex/DB_SESSION_START.md`
- `prompts/codex/DB_*.md`
- `README.md` only for DB reading-order links

Conditionally allowed:

- `.gitignore` only if needed for DB local artifacts.

Forbidden in this repair:

- `crates/storage/**`
- `crates/server/**`
- `crates/rag_core/**`
- `crates/document_ingestor/**`
- `crates/worker/**`
- `apps/web/**`
- `migrations/**`
- `schemas/**`
- `Cargo.toml`, `Cargo.lock`
- frontend package manifests / lockfiles

If forbidden files contain useful changes, save them as deferred patches outside the repository and remove them from this branch.

## How to defer out-of-scope tracked changes

Use a directory outside the repository so the deferred patch does not pollute the current diff:

```powershell
$RepoRoot = (Get-Location).Path
$DeferredDir = Join-Path (Split-Path $RepoRoot -Parent) "_trpg_codex_deferred"
New-Item -ItemType Directory -Force $DeferredDir | Out-Null

$stamp = Get-Date -Format "yyyyMMdd_HHmmss"
git diff -- crates/storage/src/lib.rs | Out-File -FilePath (Join-Path $DeferredDir "${stamp}_B02_storage_worktree.patch") -Encoding utf8
git diff --cached -- crates/storage/src/lib.rs | Out-File -FilePath (Join-Path $DeferredDir "${stamp}_B02_storage_index.patch") -Encoding utf8

git restore --staged -- crates/storage/src/lib.rs
git restore -- crates/storage/src/lib.rs
```

Repeat for other forbidden files only when `git diff --name-status` shows they are modified in this branch.

Do not discard patches silently. Record the deferred patch path in `docs/status/P2_DATABASE_STATUS.md`.

## If out-of-scope changes are already committed

If the storage/business changes are already committed in the current branch history, do not rewrite history automatically from Codex.

Report the branch as not clean and ask the maintainer to either:

1. create a clean DB-env branch from the target base and re-apply only allowed files, or
2. explicitly authorize an interactive rebase/cherry-pick strategy.

Codex should not silently rewrite commits.

## DATABASE_URL contract

`DATABASE_URL` is the ordinary app/runtime URL used by app code, SQLx prepare, repository checks, and RLS proof:

```text
postgres://trpg_app:trpg_app@127.0.0.1:55432/trpg_platform
```

It must not be the `postgres` superuser.

Privileged bootstrap/migration URLs are separate:

```text
TRPG_DATABASE_ADMIN_URL=postgres://postgres:postgres@127.0.0.1:55432/trpg_platform
TRPG_TEST_MIGRATOR_DATABASE_URL=postgres://postgres:postgres@127.0.0.1:55432/trpg_platform
TRPG_DATABASE_BOOTSTRAP_URL=postgres://postgres:postgres@127.0.0.1:55432/postgres
```

## PowerShell execution policy handling

Preferred per-session sequence:

```powershell
Get-ExecutionPolicy -List
Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass -Force
. .\scripts\dev\db\env.ps1
.\scripts\dev\db\verify.ps1
```

`Scope Process` is acceptable because it is temporary for the current PowerShell session and should not modify persistent machine or user policy.

Do not change `LocalMachine` or `CurrentUser` policy from Codex.

If `MachinePolicy` or `UserPolicy` blocks script execution even after process-scope bypass, classify script execution as an environment policy blocker. Then either run equivalent checks in a child shell or set the required environment variables manually for DB commands in the current session.

## Verification commands

```powershell
git status --short
git diff --name-status

git diff --name-only | ForEach-Object {
  if ($_ -match '^(crates/storage|crates/server|crates/rag_core|crates/document_ingestor|crates/worker|apps/web|migrations|schemas)/') {
    throw "Out-of-scope file in DB env repair: $_"
  }
}

Get-ExecutionPolicy -List
Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass -Force
. .\scripts\dev\db\env.ps1

if ([string]::IsNullOrWhiteSpace($env:DATABASE_URL)) { throw "DATABASE_URL is empty" }
if ($env:DATABASE_URL -match '://postgres(:|@)') { throw "DATABASE_URL uses postgres superuser" }

.\scripts\dev\db\verify.ps1
```

If local Docker/PostgreSQL is available:

```powershell
docker compose -f docker-compose.dev-db.yml up -d
cargo sqlx migrate run --database-url "$env:TRPG_DATABASE_ADMIN_URL"
.\scripts\dev\db\grant-app-role.ps1
.\scripts\dev\db\verify.ps1
cargo sqlx prepare --check --workspace
```

## Status update requirement

Create or update `docs/status/P2_DATABASE_STATUS.md` and record:

- whether `DATABASE_URL` is non-empty,
- whether it is ordinary app role,
- whether script execution needed process-scope bypass,
- whether any out-of-scope patches were deferred,
- which DB commands passed,
- which DB commands remain blocked and why.
