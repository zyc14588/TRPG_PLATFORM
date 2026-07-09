mod common;

use trpg_ops::release_checklist::{
    append_release_checklist_event, contract, ReleaseChecklistCommand,
};

#[test]
fn release_checklist_records_governed_event() {
    common::assert_runbook_contract(
        contract(),
        ReleaseChecklistCommand::record("release checklist gate"),
        append_release_checklist_event,
    );
}
