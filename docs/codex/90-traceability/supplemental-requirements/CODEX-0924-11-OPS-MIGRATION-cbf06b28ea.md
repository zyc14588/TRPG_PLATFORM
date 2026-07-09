# Supplemental Requirement: Projection Rebuild Runbook

Prompt ID: `CODEX-0924-11-OPS-MIGRATION-cbf06b28ea`
Primary Prompt: `CODEX-0100-11-OPS-MIGRATION-fee4c9b6ba`
Shared module: `ops_migration::projection_rebuild_runbook`
Prompt file: `codex-prompts/11-ops-migration/P0022.md`

## Merge Instructions

- Projection checkpoints and rebuild audit records are read models.
- Rebuild operations must be replay-safe and visibility-aware.
- This supplemental prompt owns no Rust src/test output.
