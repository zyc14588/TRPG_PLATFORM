# Supplemental Requirement: Backup Restore Runbook

Prompt ID: `CODEX-0913-11-OPS-MIGRATION-a691c10608`
Primary Prompt: `CODEX-0097-11-OPS-MIGRATION-e7c0cc1d29`
Shared module: `ops_migration::backup_restore_runbook`
Prompt file: `codex-prompts/11-ops-migration/P0013.md`

## Merge Instructions

- Backup manifests must include object_key, sha256, created_at, and schema_version.
- Restore hash mismatch must be reported as `RESTORE_HASH_MISMATCH`.
- This supplemental prompt owns no Rust src/test output.
