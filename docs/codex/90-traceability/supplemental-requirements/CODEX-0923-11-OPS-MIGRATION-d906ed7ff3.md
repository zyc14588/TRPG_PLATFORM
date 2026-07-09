# Supplemental Requirement: Migration Upgrade Rollback

Prompt ID: `CODEX-0923-11-OPS-MIGRATION-d906ed7ff3`
Primary Prompt: `CODEX-0099-11-OPS-MIGRATION-fde43a0ada`
Shared module: `ops_migration::migration_upgrade_rollback`
Prompt file: `codex-prompts/11-ops-migration/P0021.md`

## Merge Instructions

- Expected-version conflicts and duplicate idempotency keys must have negative tests.
- Rollback records must preserve fact provenance and correlation identifiers.
- This supplemental prompt owns no Rust src/test output.
