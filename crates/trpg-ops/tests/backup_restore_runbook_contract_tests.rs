mod common;

use trpg_ops::backup_restore_runbook::{
    append_backup_restore_runbook_event, contract, BackupRestoreRunbookCommand,
};

#[test]
fn backup_restore_runbook_records_governed_event() {
    common::assert_runbook_contract(
        contract(),
        BackupRestoreRunbookCommand::record("backup restore drill"),
        append_backup_restore_runbook_event,
    );
}

#[test]
fn backup_restore_runbook_redacts_restricted_visibility() {
    common::assert_visibility_redaction(
        BackupRestoreRunbookCommand::record("backup restore visibility"),
        append_backup_restore_runbook_event,
    );
}
