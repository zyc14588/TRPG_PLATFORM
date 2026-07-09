mod common;

use trpg_ops::implementation_plan::{
    append_implementation_plan_event, contract, ImplementationPlanCommand,
};

#[test]
fn implementation_plan_records_governed_event() {
    common::assert_runbook_contract(
        contract(),
        ImplementationPlanCommand::record("implementation plan evidence"),
        append_implementation_plan_event,
    );
}
