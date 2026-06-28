# Database URL Contract

P2 DB-backed checks require four explicit environment variables. Each variable must be set on its own line in dotenv files and PowerShell scripts.

```text
DATABASE_URL=postgres://trpg_app:trpg_app@127.0.0.1:55432/trpg_platform
TRPG_DATABASE_ADMIN_URL=postgres://postgres:postgres@127.0.0.1:55432/trpg_platform
TRPG_TEST_MIGRATOR_DATABASE_URL=postgres://postgres:postgres@127.0.0.1:55432/trpg_platform
TRPG_DATABASE_BOOTSTRAP_URL=postgres://postgres:postgres@127.0.0.1:55432/postgres
```

## Roles

| Variable | Role | Allowed use |
|---|---|---|
| `DATABASE_URL` | `trpg_app` | App runtime, SQLx prepare after migrations, repository tests, RLS proof. |
| `TRPG_DATABASE_ADMIN_URL` | `postgres` local admin | Local migrations, extension setup, grants. |
| `TRPG_TEST_MIGRATOR_DATABASE_URL` | admin or migrator | Storage fresh DB migration harness. |
| `TRPG_DATABASE_BOOTSTRAP_URL` | admin maintenance DB | Local bootstrap if the target database must be created. |

## Forbidden

- Do not point `DATABASE_URL` at `postgres`.
- Do not grant `BYPASSRLS` to `trpg_app`.
- Do not weaken RLS policies to make app-role tests pass.
- Do not run privileged migrations through the ordinary app URL.

Local `trpg_app` may have `CREATEDB` so storage tests can create disposable databases. Production app roles should not copy that grant.

