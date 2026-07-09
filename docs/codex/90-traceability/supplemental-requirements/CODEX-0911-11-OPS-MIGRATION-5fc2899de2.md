# Supplemental Requirement: Migration Upgrade Rollback

Prompt ID: `CODEX-0911-11-OPS-MIGRATION-5fc2899de2`
Primary Prompt: `CODEX-0099-11-OPS-MIGRATION-fde43a0ada`
Shared module: `ops_migration::migration_upgrade_rollback`
Prompt file: `codex-prompts/11-ops-migration/P0008.md`

## Merge Instructions

- Migration and rollback checks must retain idempotency and expected_version coverage.
- Historical source-derived names must not become current migration, event, metric, or test names.
- This supplemental prompt owns no Rust src/test output.
