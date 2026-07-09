# Supplemental Requirement: Migration Upgrade Rollback

Prompt ID: `CODEX-0915-11-OPS-MIGRATION-768ee7bafa`
Primary Prompt: `CODEX-0099-11-OPS-MIGRATION-fde43a0ada`
Shared module: `ops_migration::migration_upgrade_rollback`
Prompt file: `codex-prompts/11-ops-migration/P0011.md`

## Merge Instructions

- Preserve Authority Contract immutability during migration and rollback records.
- Migration evidence must not become the canon source; Event Store remains canon.
- This supplemental prompt owns no Rust src/test output.
