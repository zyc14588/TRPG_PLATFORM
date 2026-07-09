# BATCH-043 Work Plan

Batch: `BATCH-043-11-ops-migration`
Stage: `S10`
Scope: current batch only, `trpg-ops` rollback runbook modules plus traceability evidence.

## Metadata Note

The user-provided batch fact says the recognized primary prompt count is `0`. The authoritative normalized/current-safe maps and `batches/B043.md` identify two primary rows in this batch: `CODEX-0929-11-OPS-MIGRATION-02f99d0dd9` and `CODEX-0945-11-OPS-MIGRATION-fab61f7e5e`. Execution follows the normalized current-safe mapping and records the mismatch as an upstream metadata risk.

## Prompt Map

| Prompt ID | Prompt file | Role | Current-safe target | Allowed change | Test responsibility |
|---|---|---|---|---|---|
| `CODEX-0928-11-OPS-MIGRATION-3c8839468c` | `codex-prompts/11-ops-migration/P0026.md` | supplemental | `ops_migration::migration_upgrade_rollback` | Supplemental note only | Covered by `migration_upgrade_rollback_contract_tests` |
| `CODEX-0929-11-OPS-MIGRATION-02f99d0dd9` | `codex-prompts/11-ops-migration/P0027.md` | primary | `crates/trpg-ops/src/upgrade_rollback_impl.rs` | Rust module + contract test | `upgrade_rollback_impl_contract_tests` |
| `CODEX-0930-11-OPS-MIGRATION-32eaccd817` | `codex-prompts/11-ops-migration/P0028.md` | traceability | `docs/codex/11-ops-migration/source_processing_record_docs_implementation_11_ops_migration_backup_restore_runbook.md` | Markdown record only | Prompt traceability review |
| `CODEX-0933-11-OPS-MIGRATION-41626d642f` | `codex-prompts/11-ops-migration/P0029.md` | traceability | `docs/codex/11-ops-migration/source_processing_record_docs_implementation_11_ops_migration_projection_rebuild_runbook.md` | Markdown record only | Prompt traceability review |
| `CODEX-0934-11-OPS-MIGRATION-4fed223a48` | `codex-prompts/11-ops-migration/P0030.md` | traceability | `docs/codex/11-ops-migration/source_processing_record_docs_implementation_11_ops_migration_release_checklist.md` | Markdown record only | Prompt traceability review |
| `CODEX-0935-11-OPS-MIGRATION-6ff24c0d80` | `codex-prompts/11-ops-migration/P0031.md` | traceability | `docs/codex/11-ops-migration/source_processing_record_docs_implementation_11_ops_migration_readme.md` | Markdown record only | Prompt traceability review |
| `CODEX-0936-11-OPS-MIGRATION-7e5cde6a26` | `codex-prompts/11-ops-migration/P0032.md` | traceability | `docs/codex/11-ops-migration/source_processing_record_docs_implementation_90_traceability_source_breakdown_migration_upgrade_rollback.md` | Markdown record only | Prompt traceability review |
| `CODEX-0932-11-OPS-MIGRATION-d0434b6a16` | `codex-prompts/11-ops-migration/P0033.md` | traceability | `docs/codex/11-ops-migration/source_processing_record_docs_implementation_11_ops_migration_migration_upgrade_rollback.md` | Markdown record only | Prompt traceability review |
| `CODEX-0931-11-OPS-MIGRATION-fc7d9b925c` | `codex-prompts/11-ops-migration/P0034.md` | traceability | `docs/codex/11-ops-migration/source_processing_record_docs_implementation_11_ops_migration_incident_response_runbook.md` | Markdown record only | Prompt traceability review |
| `CODEX-0937-11-OPS-MIGRATION-9000915565` | `codex-prompts/11-ops-migration/P0035.md` | traceability | `docs/codex/11-ops-migration/source_processing_record_docs_migration_upgrade_rollback.md` | Markdown record only | Prompt traceability review |
| `CODEX-0938-11-OPS-MIGRATION-9ecfe69fb4` | `codex-prompts/11-ops-migration/P0036.md` | supplemental | `ops_migration::backup_restore_runbook` | Supplemental note only | Covered by `backup_restore_runbook_contract_tests` |
| `CODEX-0939-11-OPS-MIGRATION-eb460e9f63` | `codex-prompts/11-ops-migration/P0037.md` | supplemental | `ops_migration::incident_response_runbook` | Supplemental note only | Covered by `incident_response_runbook_contract_tests` |
| `CODEX-0940-11-OPS-MIGRATION-e8fb918a90` | `codex-prompts/11-ops-migration/P0038.md` | supplemental | `ops_migration::migration_upgrade_rollback` | Supplemental note only | Covered by `migration_upgrade_rollback_contract_tests` |
| `CODEX-0941-11-OPS-MIGRATION-4d01908706` | `codex-prompts/11-ops-migration/P0039.md` | supplemental | `ops_migration::projection_rebuild_runbook` | Supplemental note only | Covered by `projection_rebuild_runbook_contract_tests` |
| `CODEX-0942-11-OPS-MIGRATION-fa38058aef` | `codex-prompts/11-ops-migration/P0040.md` | supplemental | `ops_migration::readme` | Supplemental note only | Covered by `readme_contract_tests` |
| `CODEX-0943-11-OPS-MIGRATION-fcce5e5977` | `codex-prompts/11-ops-migration/P0041.md` | supplemental | `ops_migration::release_checklist` | Supplemental note only | Covered by `release_checklist_contract_tests` |
| `CODEX-0944-11-OPS-MIGRATION-a1d363e292` | `codex-prompts/11-ops-migration/P0042.md` | supplemental | `ops_migration::upgrade_rollback_impl` | Supplemental note only | Covered by `upgrade_rollback_impl_contract_tests` |
| `CODEX-0945-11-OPS-MIGRATION-fab61f7e5e` | `codex-prompts/11-ops-migration/P0043.md` | primary | `crates/trpg-ops/src/upgrade_rollback.rs` | Rust module + contract test | `upgrade_rollback_contract_tests` |

## Checks

Minimum related checks:
- `cargo test -p trpg-ops --test upgrade_rollback_impl_contract_tests`
- `cargo test -p trpg-ops --test upgrade_rollback_contract_tests`

Stage checks:
- `cargo fmt --all -- --check`
- `cargo test -p trpg-ops --all-features`
- `cargo check --workspace --all-features`

S10 shell script checks now exist and must be run:
- `scripts/backup_restore/smoke.sh`
- `scripts/projection_rebuild/verify.sh`
