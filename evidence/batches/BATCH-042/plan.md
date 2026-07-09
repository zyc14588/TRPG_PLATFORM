# BATCH-042 Work Plan

Batch: `BATCH-042-11-ops-migration`
Stage: `S10`
Scope: current batch only, `trpg-ops` plus traceability evidence.

## Metadata Note

`batch-prompts/start/B042.md` says "recognized primary prompt count: 0", but `batches/B042.md`, `docs/codex/11-ops-migration/per-file-prompt-manifest.md`, and the current-safe maps identify 9 primary rows, 15 supplemental rows, and 1 documentation row in this batch. Execution follows the normalized current-safe rows and records this mismatch as upstream metadata risk.

## Prompt Map

| Prompt ID | Role | Current-safe target | Allowed change | Test responsibility |
|---|---|---|---|---|
| CODEX-0097-11-OPS-MIGRATION-e7c0cc1d29 | primary | `crates/trpg-ops/src/backup_restore_runbook.rs` | Rust module + contract test | `backup_restore_runbook_contract_tests` |
| CODEX-0098-11-OPS-MIGRATION-feb9c54dda | primary | `crates/trpg-ops/src/incident_response_runbook.rs` | Rust module + contract test | `incident_response_runbook_contract_tests` |
| CODEX-0099-11-OPS-MIGRATION-fde43a0ada | primary | `crates/trpg-ops/src/migration_upgrade_rollback.rs` | Rust module + contract test | `migration_upgrade_rollback_contract_tests` |
| CODEX-0100-11-OPS-MIGRATION-fee4c9b6ba | primary | `crates/trpg-ops/src/projection_rebuild_runbook.rs` | Rust module + contract test | `projection_rebuild_runbook_contract_tests` |
| CODEX-0101-11-OPS-MIGRATION-57b0f58ae0 | primary | `crates/trpg-ops/src/release_checklist.rs` | Rust module + contract test | `release_checklist_contract_tests` |
| CODEX-0917-11-OPS-MIGRATION-9f27ade2d3 | primary | `crates/trpg-ops/src/readme.rs` | Shared ops contract + contract test | `readme_contract_tests` |
| CODEX-0921-11-OPS-MIGRATION-7457f82a14 | primary | `crates/trpg-ops/src/implementation_plan.rs` | Rust module + contract test | `implementation_plan_contract_tests` |
| CODEX-0920-11-OPS-MIGRATION-ebe3f221d7 | primary | `crates/trpg-ops/src/backlog.rs` | Rust module + contract test | `backlog_contract_tests` |
| CODEX-0927-11-OPS-MIGRATION-5a238036dd | primary | `crates/trpg-ops/src/upgrade_backup_replay_runbooks.rs` | Rust module + contract test | `upgrade_backup_replay_runbooks_contract_tests` |
| CODEX-0096-11-OPS-MIGRATION-bbe2ac850c | documentation | `docs/codex/11-ops-migration/m_11_ops_migration.md` | Module map only | Markdown traceability |
| CODEX-0909/CODEX-0910/CODEX-0911/CODEX-0913/CODEX-0915/CODEX-0923 | supplemental | backup/migration merge notes | Supplemental Markdown only | Covered by primary tests |
| CODEX-0914/CODEX-0922 | supplemental | incident merge notes | Supplemental Markdown only | Covered by primary tests |
| CODEX-0916/CODEX-0924 | supplemental | projection merge notes | Supplemental Markdown only | Covered by primary tests |
| CODEX-0918/CODEX-0926 | supplemental | release merge notes | Supplemental Markdown only | Covered by primary tests |
| CODEX-0912/CODEX-0919 | supplemental | `upgrade_rollback` owned by BATCH-043 | Deferred with reason | No BATCH-042 Rust output |
| CODEX-0925 | supplemental | readme merge note | Supplemental Markdown only | Covered by readme tests |

## Checks

Minimum related:
- `cargo test -p trpg-ops --all-features`
- `cargo fmt --all -- --check`

Stage-related:
- `cargo check --workspace --all-features`
- S10 script checks are recorded as not run unless scripts exist and are executable in this workspace.
