mod common;

use trpg_ops::upgrade_backup_replay_runbooks::{
    append_upgrade_backup_replay_runbooks_event, contract, UpgradeBackupReplayRunbooksCommand,
};

#[test]
fn upgrade_backup_replay_runbooks_records_governed_event() {
    common::assert_runbook_contract(
        contract(),
        UpgradeBackupReplayRunbooksCommand::record("upgrade backup replay drill"),
        append_upgrade_backup_replay_runbooks_event,
    );
}
