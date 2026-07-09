# 11 Ops Migration Module Map

Prompt ID: `CODEX-0096-11-OPS-MIGRATION-bbe2ac850c`
Batch: `BATCH-042-11-ops-migration`
Current crate: `trpg-ops`
Current module prefix: `ops_migration`

## Current-Safe Outputs

The BATCH-042 implementation owns only current-safe Rust module names under `crates/trpg-ops/src/` and contract tests under `crates/trpg-ops/tests/`.

Primary outputs:
- `backup_restore_runbook`
- `incident_response_runbook`
- `migration_upgrade_rollback`
- `projection_rebuild_runbook`
- `release_checklist`
- `readme`
- `implementation_plan`
- `backlog`
- `upgrade_backup_replay_runbooks`

## Governance Boundary

- Event Store remains canon.
- Projection rebuild does not append canon events.
- Restore verification requires matching event hashes.
- Supplemental prompts merge into their primary prompt and do not own Rust src/test output.
