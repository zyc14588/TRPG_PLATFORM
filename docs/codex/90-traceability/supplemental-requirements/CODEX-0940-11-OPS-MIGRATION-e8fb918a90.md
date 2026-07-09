# Supplemental Requirement: Migration Upgrade Rollback

Prompt ID: `CODEX-0940-11-OPS-MIGRATION-e8fb918a90`
Primary Prompt: `CODEX-0099-11-OPS-MIGRATION-fde43a0ada`
Shared module: `ops_migration::migration_upgrade_rollback`
Prompt file: `codex-prompts/11-ops-migration/P0038.md`

## Merge Instructions

- Irreversible migrations must keep an explicit rollback runbook gate.
- Rollback verification must remain tied to command, workflow, decision, event store, and projection replay.
- This supplemental prompt owns no Rust src/test output in BATCH-043.
