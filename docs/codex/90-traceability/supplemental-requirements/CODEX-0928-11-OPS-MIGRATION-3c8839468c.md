# Supplemental Requirement: Migration Upgrade Rollback

Prompt ID: `CODEX-0928-11-OPS-MIGRATION-3c8839468c`
Primary Prompt: `CODEX-0099-11-OPS-MIGRATION-fde43a0ada`
Shared module: `ops_migration::migration_upgrade_rollback`
Prompt file: `codex-prompts/11-ops-migration/P0026.md`

## Merge Instructions

- Preserve rollback proof, migration ledger, and event store hash checks.
- Treat historic source paths as provenance only.
- This supplemental prompt owns no Rust src/test output in BATCH-043.
