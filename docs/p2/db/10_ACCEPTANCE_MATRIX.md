# P2 DB Build Acceptance Matrix

| Requirement | Evidence |
|---|---|
| `.env.example` is valid multiline dotenv | one variable per line; app URL is `postgres://trpg_app:trpg_app@127.0.0.1:55432/trpg_platform` |
| compose target is pgvector | `docker-compose.dev-db.yml` uses `pgvector/pgvector:pg16` |
| host port avoids local 5432 | compose maps `55432:5432` |
| stable container name | `trpg-platform-pgvector` |
| env script sets all DB URLs | dot-source `scripts/dev/db/env.ps1` and inspect env |
| app role is ordinary | `verify.ps1` proves `trpg_app` is not superuser and not BYPASSRLS |
| extensions installed | `verify.ps1` proves both `pgcrypto` and `vector` exist |
| key RLS tables are enabled and forced | `verify.ps1` checks `documents`, `chunks`, `document_sources`, `ingest_jobs` |
| migrations use admin/migrator URL | `cargo sqlx migrate run --database-url "$env:TRPG_DATABASE_ADMIN_URL"` |
| storage migration harness uses admin/migrator URL | `cargo test -p storage`; helper reads `TRPG_TEST_MIGRATOR_DATABASE_URL` then `TRPG_DATABASE_ADMIN_URL` |
| repository/RLS proof uses app URL | storage helper guards `DATABASE_URL` against postgres/superuser/BYPASSRLS |
| SQLx prepare works after migration/grants | `cargo sqlx prepare --check --workspace` |
| full workspace tests are DB-backed | `cargo test --workspace` with env variables set |

PASS requires every runnable command to pass. If Docker or DB access is unavailable, result is BLOCKED, not PASS.

