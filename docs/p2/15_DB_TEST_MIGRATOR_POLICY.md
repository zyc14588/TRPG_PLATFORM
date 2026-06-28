# DB Test Migrator URL Policy

This document defines the P2 database-test boundary for migrations, SQLx prepare, and RLS proof.

## Problem

Some storage tests create a fresh database state and run the full migration set. The migration `20260626021000_p1_5_auth_private_and_rag_license_rls.sql` contains privileged role bootstrap, including `ALTER ROLE trpg_app_private ... BYPASSRLS`.

That migration must not be executed through the ordinary application role. If the test runner derives the migration connection from `DATABASE_URL`, and `DATABASE_URL` correctly points to `trpg_app`, the migration fails with a privilege error. That is a real test-harness privilege bug, not a missing-DB environment blocker.

## Required URL split

Use separate URLs:

```text
TRPG_TEST_MIGRATOR_DATABASE_URL or TRPG_DATABASE_ADMIN_URL
  Purpose: local/CI migration bootstrap, role bootstrap, extension setup, fresh DB test migration harness.
  May point to postgres or a dedicated migrator role.

DATABASE_URL
  Purpose: application runtime, repository access, SQLx query checking after migrations, and RLS proof.
  Must be a non-superuser ordinary app role such as trpg_app.
```

## Rules

1. Do not set app runtime `DATABASE_URL` to `postgres://postgres...` merely to make tests pass.
2. Do not grant `BYPASSRLS` to the ordinary app role.
3. Do not weaken migrations by silently skipping privileged role setup when run as the wrong role.
4. Any test that runs all migrations against a fresh DB must use a migrator/admin URL.
5. Any test that proves repository/RLS behavior must use the ordinary app URL.
6. `cargo sqlx prepare --check --workspace` may use `DATABASE_URL` only after migrations and grants have already been applied by the migrator/admin URL.
7. After admin-run migrations create `trpg_app_private`, local bootstrap must grant that controlled role to the ordinary app role for auth-private repository queries. The ordinary app role itself must remain `NOSUPERUSER` and `NOBYPASSRLS`.

## Recommended test helper contract

Storage integration tests should expose helpers equivalent to:

```rust
fn app_database_url() -> anyhow::Result<String> {
    let url = std::env::var("DATABASE_URL")?;
    assert!(!url.contains("://postgres:") && !url.contains("://postgres@"),
        "DATABASE_URL must be an ordinary app role for RLS proof, not postgres");
    Ok(url)
}

fn migrator_database_url() -> anyhow::Result<String> {
    std::env::var("TRPG_TEST_MIGRATOR_DATABASE_URL")
        .or_else(|_| std::env::var("TRPG_DATABASE_ADMIN_URL"))
        .map_err(Into::into)
}
```

The exact error type and URL parsing style should follow the project. The important invariant is semantic: migration bootstrap uses the migrator/admin URL; app/RLS proof uses the ordinary app URL.

## Verification commands

PowerShell from repository root:

```powershell
. .\scripts\dev\db\env.ps1

Write-Host "DATABASE_URL=$($env:DATABASE_URL)"
Write-Host "TRPG_DATABASE_ADMIN_URL=$($env:TRPG_DATABASE_ADMIN_URL)"

cargo sqlx migrate run --database-url "$env:TRPG_DATABASE_ADMIN_URL"
.\scripts\dev\db\grant-app-role.ps1
.\scripts\dev\db\verify.ps1
cargo sqlx prepare --check --workspace
cargo test -p storage
```

Acceptance requires `cargo test -p storage` to stop failing on `ALTER ROLE ... BYPASSRLS` when the admin URL is present, while `DATABASE_URL` remains the ordinary app role.
