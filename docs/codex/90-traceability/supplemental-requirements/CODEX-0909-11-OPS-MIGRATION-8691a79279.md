# Supplemental Requirement: Backup Restore Runbook

Prompt ID: `CODEX-0909-11-OPS-MIGRATION-8691a79279`
Primary Prompt: `CODEX-0097-11-OPS-MIGRATION-e7c0cc1d29`
Shared module: `ops_migration::backup_restore_runbook`
Prompt file: `codex-prompts/11-ops-migration/P0007.md`

## Merge Instructions

- Restore verification must compare Event Store hashes before and after restore.
- Projection/cache/RAG outputs remain rebuildable read models.
- This supplemental prompt owns no Rust src/test output.
