crate::define_ops_runbook_module!(
    MigrationUpgradeRollbackCommand,
    MigrationUpgradeRollbackService,
    MigrationUpgradeRollbackRepository,
    MigrationUpgradeRollbackError,
    append_migration_upgrade_rollback_event,
    "CODEX-0099-11-OPS-MIGRATION-fde43a0ada",
    "migration_upgrade_rollback",
    "OpsMigrationUpgradeRollbackRecorded",
    crate::OpsRunbookOperation::MigrationUpgradeRollback,
    ["migration_ledger", "rollback_plan", "event_store_hash"],
    "evidence/batches/BATCH-042/migration-upgrade-rollback.md"
);

pub fn verify_rollback_runbook(
    irreversible_migration: bool,
    has_rollback_runbook: bool,
) -> Result<(), crate::OpsRunbookError> {
    if irreversible_migration && !has_rollback_runbook {
        Err(crate::OpsRunbookError::RollbackRunbookRequired)
    } else {
        Ok(())
    }
}
