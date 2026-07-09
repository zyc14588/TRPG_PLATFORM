mod common;

use trpg_ops::backlog::{append_backlog_event, contract, BacklogCommand};

#[test]
fn backlog_records_governed_event() {
    common::assert_runbook_contract(
        contract(),
        BacklogCommand::record("backlog handoff"),
        append_backlog_event,
    );
}
