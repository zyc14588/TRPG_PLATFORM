# Supplemental Requirement: Migration Upgrade Rollback

Prompt ID: `CODEX-0910-11-OPS-MIGRATION-ea7b7095ff`
Primary Prompt: `CODEX-0099-11-OPS-MIGRATION-fde43a0ada`
Shared module: `ops_migration::migration_upgrade_rollback`
Prompt file: `codex-prompts/11-ops-migration/P0009.md`

## Merge Instructions

- Rollback dry-runs must fail closed when an irreversible migration lacks a rollback runbook.
- Evidence must keep operator, command, exit_code, and evidence_path fields.
- This supplemental prompt owns no Rust src/test output.
