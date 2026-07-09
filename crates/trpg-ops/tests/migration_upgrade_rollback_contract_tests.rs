mod common;

use trpg_ops::migration_upgrade_rollback::{
    append_migration_upgrade_rollback_event, contract, verify_rollback_runbook,
    MigrationUpgradeRollbackCommand,
};

const S10_DETAILED_FIXTURE: &str = include_str!(
    "../../../fixtures/stages/detailed/S10_ops_migration_runbooks_expected.current.json.md"
);

#[test]
fn migration_upgrade_rollback_records_governed_event() {
    common::assert_runbook_contract(
        contract(),
        MigrationUpgradeRollbackCommand::record("migration rollback drill"),
        append_migration_upgrade_rollback_event,
    );
}

#[test]
fn migration_upgrade_rollback_requires_runbook_for_irreversible_migration() {
    assert!(S10_DETAILED_FIXTURE.contains("\"case\": \"irreversible_migration_without_runbook\""));
    assert!(S10_DETAILED_FIXTURE.contains("\"error\": \"ROLLBACK_RUNBOOK_REQUIRED\""));

    let error = verify_rollback_runbook(true, false).expect_err("rollback runbook is mandatory");
    assert_eq!(error.code(), "ROLLBACK_RUNBOOK_REQUIRED");

    verify_rollback_runbook(true, true).expect("irreversible migration has a rollback runbook");
    verify_rollback_runbook(false, false).expect("reversible migration does not need one");
}
