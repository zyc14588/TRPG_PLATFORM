# P2 Database Setup

## Goal

Provide a repeatable local PostgreSQL setup for P2 migrations, RLS tests and SQLx checks.

## Roles

Recommended local roles:

| Role | Purpose | RLS posture |
|---|---|---|
| `trpg_owner` | local bootstrap owner for database, schemas and extensions | setup only; never runtime |
| `trpg_migrator` | applies SQLx migrations | migration only; never runtime |
| `trpg_app` | ordinary application login used by server and app-level DB checks | no superuser, no broad `BYPASSRLS` |
| `trpg_rls_test` | test-only ordinary role created by storage tests | no login, no `BYPASSRLS`; used through `SET LOCAL ROLE` |
| `trpg_readonly` | optional diagnostics role | read-only diagnostics only |

Do not use `postgres` superuser as application role.

Current migrations also create a controlled `trpg_app_private` role for auth-private
tables. It has `BYPASSRLS` but only receives privileges on auth-private tables, and
repository code switches to it only for those queries. Do not treat it as the
ordinary app role.

## Database

Recommended local DB:

```text
trpg_platform
```

Example local URLs:

```powershell
$env:DATABASE_URL = "postgres://trpg_app:<password>@localhost:5432/trpg_platform"
$env:TRPG_MIGRATOR_DATABASE_URL = "postgres://trpg_migrator:<password>@localhost:5432/trpg_platform"
$env:TRPG_BOOTSTRAP_DATABASE_URL = "postgres://postgres:<local-compose-password>@localhost:5432/postgres"
```

`DATABASE_URL` is the app/runtime URL. For migration commands that only read
`DATABASE_URL`, temporarily assign it from `TRPG_MIGRATOR_DATABASE_URL`, then
switch it back to `trpg_app`.

## Extensions

The repo currently uses `pgcrypto` and pgvector:

```sql
create extension if not exists pgcrypto;
create extension if not exists vector;
```

`infra/compose/compose.yaml` already uses `pgvector/pgvector:pg17`. Extension
creation may require the local bootstrap owner or disposable compose superuser.
Ordinary app role must not require superuser privileges.

## Local bootstrap SQL sketch

Adapt passwords and DB names for local use only. Run this once against a fresh
local database cluster, usually with `TRPG_BOOTSTRAP_DATABASE_URL`:

```sql
create role trpg_owner login password 'local_trpg_owner_change_me';
create role trpg_migrator login password 'local_trpg_migrator_change_me';
create role trpg_app login password 'local_trpg_app_change_me';
create database trpg_platform owner trpg_owner;

\c trpg_platform
create extension if not exists pgcrypto;
create extension if not exists vector;
grant connect on database trpg_platform to trpg_migrator, trpg_app;
grant usage, create on schema public to trpg_migrator;
grant usage on schema public to trpg_app;
```

After migrations, grant local app privileges if your migration role owns the
objects:

```sql
grant usage on schema public, app to trpg_app;
grant select, insert, update, delete on all tables in schema public to trpg_app;
grant usage, select on all sequences in schema public to trpg_app;
grant execute on all functions in schema app to trpg_app;
grant trpg_app_private to trpg_app;
```

For future migrations, use matching `ALTER DEFAULT PRIVILEGES` for the role that
creates tables/functions in local dev.

## Environment variables

Development:

```powershell
$env:TRPG_AUTH_MODE = "development"
$env:TRPG_AUTH_SECRET = "development-secret-at-least-32-bytes-change-me"
$env:DATABASE_URL = "postgres://trpg_app:<password>@localhost:5432/trpg_platform"
$env:NEXT_PUBLIC_API_BASE_URL = "http://127.0.0.1:8080"
```

Migration session may need:

```powershell
$env:DATABASE_URL = $env:TRPG_MIGRATOR_DATABASE_URL
cargo sqlx migrate run
```

Then switch back to app role for app/RLS tests if project test harness expects ordinary role.

```powershell
$env:DATABASE_URL = "postgres://trpg_app:<password>@localhost:5432/trpg_platform"
cargo sqlx prepare --check --workspace
```

Current `cargo test -p storage` DB-backed tests use `DATABASE_URL` for setup and
migration, then prove RLS with `SET LOCAL ROLE trpg_rls_test`. If `DATABASE_URL`
is unset or cannot run migrations, DB-backed tests may skip and do not count as
an RLS proof.

## Checks

```powershell
cargo sqlx migrate run
cargo sqlx prepare --check --workspace
cargo test -p storage
cargo test --workspace
```

## RLS proof requirement

Any test proving access control must use an ordinary app-equivalent role with
trusted session context. In this repo that is `trpg_rls_test` via `SET LOCAL
ROLE`, plus `app.user_id`, `app.room_id`, `app.room_role` and, where needed,
`app.rag_access_path`. It must not rely on owner/superuser bypassing RLS.

## Troubleshooting

- `permission denied for schema public`: grant usage/create appropriately for migrator; grant usage/select/insert/update/delete for app via migrations.
- `extension vector is not available`: install pgvector or switch to documented local vector fallback.
- SQLx offline failure: regenerate offline data only after migrations are current and DB role is correct.
- RLS test sees too much data: check whether test connection uses owner/superuser or table owner without `force row level security`.
