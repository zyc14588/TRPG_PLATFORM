# Storage Test Harness Policy

Storage tests use two database identities.

## URL Selection

- Fresh/disposable database setup, role bootstrap, extension setup, and SQLx migrations use `TRPG_TEST_MIGRATOR_DATABASE_URL`.
- If `TRPG_TEST_MIGRATOR_DATABASE_URL` is missing or empty, the harness falls back to `TRPG_DATABASE_ADMIN_URL`.
- Application repository tests and RLS proof use `DATABASE_URL`.

`DATABASE_URL` must remain an ordinary app role such as `trpg_app`. It must not be `postgres`, superuser, or `BYPASSRLS`.

## Failure Policy

Missing or empty migrator/admin URLs must fail with an actionable error:

```text
storage migration bootstrap requires non-empty TRPG_TEST_MIGRATOR_DATABASE_URL or TRPG_DATABASE_ADMIN_URL
```

Missing or empty app URLs must fail before DB proof runs and tell the developer to dot-source:

```powershell
. .\scripts\dev\db\env.ps1
```

## Verification

```powershell
. .\scripts\dev\db\env.ps1
cargo sqlx migrate run --database-url "$env:TRPG_DATABASE_ADMIN_URL"
.\scripts\dev\db\grant-app-role.ps1
.\scripts\dev\db\verify.ps1
cargo sqlx prepare --check --workspace
cargo test -p storage
```

Do not fix migration privilege failures by changing app `DATABASE_URL` to `postgres`, granting `BYPASSRLS` to `trpg_app`, deleting privileged migrations, skipping RLS tests, or weakening policies.
