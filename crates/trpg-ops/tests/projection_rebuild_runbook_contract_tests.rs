mod common;

use trpg_ops::projection_rebuild_runbook::{
    append_projection_rebuild_runbook_event, contract, ProjectionRebuildRunbookCommand,
};

#[test]
fn projection_rebuild_runbook_records_governed_event() {
    common::assert_runbook_contract(
        contract(),
        ProjectionRebuildRunbookCommand::record("projection rebuild drill"),
        append_projection_rebuild_runbook_event,
    );
}
