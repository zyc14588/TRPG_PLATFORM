# Supplemental Requirement: Projection Rebuild Runbook

Prompt ID: `CODEX-0916-11-OPS-MIGRATION-e17b29109d`
Primary Prompt: `CODEX-0100-11-OPS-MIGRATION-fee4c9b6ba`
Shared module: `ops_migration::projection_rebuild_runbook`
Prompt file: `codex-prompts/11-ops-migration/P0016.md`

## Merge Instructions

- Projection rebuild must be deterministic and append zero canon events.
- Projection hash mismatch must be reported as `PROJECTION_REBUILD_HASH_MISMATCH`.
- This supplemental prompt owns no Rust src/test output.
