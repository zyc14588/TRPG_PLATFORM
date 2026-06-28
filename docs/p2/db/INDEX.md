# P2 DB Build Gate Index

Read this directory for every P2 B02/B07 DB-backed implementation or acceptance session.

## Reading Order

1. `../../../CODEX_P2_MASTER_PROMPT.md`
2. `../INDEX.md`
3. `02_DATABASE_URL_CONTRACT.md`
4. `08_B02_DATABASE_BUILD_RUNBOOK.md`
5. `09_TROUBLESHOOTING_DATABASE_URL_EMPTY.md`
6. `10_ACCEPTANCE_MATRIX.md`
7. `../11_DATABASE_SETUP.md`
8. `../15_DB_TEST_MIGRATOR_POLICY.md`

If a prompt mentions `CODEX_DB_MASTER_PROMPT.md`, use this directory plus `../../../CODEX_P2_MASTER_PROMPT.md`; this repository does not currently carry a separate root DB master prompt.

## Stable Local Target

```text
PostgreSQL image: pgvector/pgvector:pg16
Container: trpg-platform-pgvector
Host: 127.0.0.1
Port: 55432
Database: trpg_platform
App role: trpg_app
Admin role: postgres
```
