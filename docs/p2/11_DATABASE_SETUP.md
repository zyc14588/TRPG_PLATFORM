# P2 Database Setup — Local PostgreSQL + pgvector

This document defines the local DB target required before P2 Storage/RLS, migrations, SQLx prepare, and true DB-backed RLS proof can pass.

## Policy

If the shell has no `DATABASE_URL`, or there is no confirmed PostgreSQL instance with pgvector, the correct result is `BLOCKED`, not `PASS`.

For local development, keep the URLs separate:

| Variable | Role | Use |
|---|---|---|
| `TRPG_DATABASE_ADMIN_URL` | `postgres` local admin | Local bootstrap and `sqlx migrate run` only. |
| `TRPG_TEST_MIGRATOR_DATABASE_URL` | `postgres` local admin or dedicated migrator | Storage migration bootstrap and fresh DB migration tests. Falls back to `TRPG_DATABASE_ADMIN_URL`. |
| `DATABASE_URL` | `trpg_app` non-superuser | App runtime, SQLx prepare, storage tests, and DB-backed RLS proof. |

Do not use the `postgres` superuser as the application role.

The dev-only `trpg_app` role has `CREATEDB` so `cargo test -p storage` can create a disposable database for migration idempotence. `grant-app-role.ps1` also installs `pgcrypto` and `vector` into local `template1` using `postgres`, so the disposable database inherits required extensions without making `trpg_app` a superuser. `trpg_app` must still be `NOSUPERUSER` and `NOBYPASSRLS`; production app roles should not copy the local `CREATEDB` grant.

Storage tests that run SQLx migrations or create fresh disposable databases use `TRPG_TEST_MIGRATOR_DATABASE_URL`, falling back to `TRPG_DATABASE_ADMIN_URL`. They must not use the ordinary app `DATABASE_URL` for privileged role bootstrap such as `ALTER ROLE ... BYPASSRLS`. Repository and RLS proof queries still use `DATABASE_URL`.

## Recommended local target

Use the provided Docker Compose file:

```powershell
docker compose -f docker-compose.dev-db.yml up -d
```

This starts PostgreSQL + pgvector on:

```text
127.0.0.1:55432
database: trpg_platform
admin user: postgres
admin password: postgres
app user: trpg_app
app password: trpg_app
```

These credentials are local-only.

## Setup sequence

From the repository root:

```powershell
Set-ExecutionPolicy -Scope Process Bypass -Force
.\scripts\dev\db\start.ps1
. .\scripts\dev\db\env.ps1
cargo sqlx migrate run --database-url "$env:TRPG_DATABASE_ADMIN_URL"
.\scripts\dev\db\grant-app-role.ps1
.\scripts\dev\db\verify.ps1
cargo sqlx prepare --check --workspace
cargo test -p storage
```

If the project has DB integration tests requiring `DATABASE_URL`, keep using:

```powershell
$env:DATABASE_URL = "postgres://trpg_app:trpg_app@127.0.0.1:55432/trpg_platform"
```

If a migration requires elevated privileges, run only the migration command with:

```powershell
cargo sqlx migrate run --database-url "$env:TRPG_DATABASE_ADMIN_URL"
```

`Set-ExecutionPolicy -Scope Process Bypass -Force` affects only the current PowerShell process. It is needed on Windows shells that block unsigned local `.ps1` scripts.

## Acceptance

DB setup is acceptable only when all of these are true:

- `DATABASE_URL` is set in the same shell used by Codex/check commands.
- PostgreSQL is reachable.
- `CREATE EXTENSION vector` succeeds, or the extension already exists.
- `cargo sqlx migrate run` succeeds against the confirmed DB.
- `cargo sqlx prepare --check --workspace` succeeds, or any failure is a real code/query issue rather than missing env.
- `cargo test -p storage` succeeds, or any failure is a real storage/migration privilege issue rather than missing DB env.
- Storage migration bootstrap uses `TRPG_TEST_MIGRATOR_DATABASE_URL` or `TRPG_DATABASE_ADMIN_URL`, never the app `DATABASE_URL`.
- RLS proof tests use a non-superuser ordinary role, not `postgres`. The local proof roles are `trpg_app` for the connection and `trpg_rls_test` for `SET LOCAL ROLE`.
- The final report records the exact URLs in redacted form, for example `postgres://trpg_app:***@127.0.0.1:55432/trpg_platform`.

If `cargo test -p storage` fails in migration bootstrap, check that `TRPG_TEST_MIGRATOR_DATABASE_URL` or `TRPG_DATABASE_ADMIN_URL` is set to an admin/migrator role. Do not make `DATABASE_URL` point at `postgres` to hide migration privilege failures, and do not weaken RLS migrations.

## Forbidden

- Do not commit `.env` containing real or local passwords.
- Do not use `postgres` as the application `DATABASE_URL` for RLS proof.
- Do not mark P2 B02 PASS while migrations or DB-backed RLS proof are unrun due to environment.
