# Supplemental Requirement: Backup Restore Runbook

Prompt ID: `CODEX-0938-11-OPS-MIGRATION-9ecfe69fb4`
Primary Prompt: `CODEX-0097-11-OPS-MIGRATION-e7c0cc1d29`
Shared module: `ops_migration::backup_restore_runbook`
Prompt file: `codex-prompts/11-ops-migration/P0036.md`

## Merge Instructions

- Preserve backup manifest, restore hash, and visibility redaction requirements.
- Backup and restore evidence remains event-store-backed, not projection-authored.
- This supplemental prompt owns no Rust src/test output in BATCH-043.
