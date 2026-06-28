# B02 Database Build Runbook

Run from the repository root in PowerShell:

```powershell
git status --short
git branch --show-current
cargo metadata --no-deps

. .\scripts\dev\db\env.ps1
Get-ChildItem Env:DATABASE_URL, Env:TRPG_DATABASE_ADMIN_URL, Env:TRPG_TEST_MIGRATOR_DATABASE_URL, Env:TRPG_DATABASE_BOOTSTRAP_URL

docker compose -f docker-compose.dev-db.yml up -d
cargo sqlx migrate run --database-url "$env:TRPG_DATABASE_ADMIN_URL"
.\scripts\dev\db\grant-app-role.ps1
.\scripts\dev\db\verify.ps1
cargo sqlx prepare --check --workspace
cargo test -p storage
cargo test --workspace
```

`scripts/dev/db/start.ps1` may be used instead of the direct compose command. It starts the container, waits for health, and creates or updates `trpg_app` as `LOGIN NOSUPERUSER NOBYPASSRLS CREATEDB`.

## Expected Split

- Migrations and grants use `TRPG_DATABASE_ADMIN_URL` or `TRPG_TEST_MIGRATOR_DATABASE_URL`.
- App runtime, SQLx prepare, repository tests, and RLS proof use `DATABASE_URL`.
- Storage tests must fail clearly if required DB URLs are absent. They must not silently skip DB coverage.

## Docker Unavailable

If Docker or the DB daemon is unavailable, finish static file repair and mark the DB-backed gate `BLOCKED`. Do not report PASS.

