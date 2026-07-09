# Supplemental Requirement: Projection Rebuild Runbook

Prompt ID: `CODEX-0941-11-OPS-MIGRATION-4d01908706`
Primary Prompt: `CODEX-0100-11-OPS-MIGRATION-fee4c9b6ba`
Shared module: `ops_migration::projection_rebuild_runbook`
Prompt file: `codex-prompts/11-ops-migration/P0039.md`

## Merge Instructions

- Projection rebuilds must prove they create no new canon events.
- Projection output remains rebuildable from event store history.
- This supplemental prompt owns no Rust src/test output in BATCH-043.
