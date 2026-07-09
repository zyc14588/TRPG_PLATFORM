mod common;

use trpg_ops::incident_response_runbook::{
    append_incident_response_runbook_event, contract, IncidentResponseRunbookCommand,
};

#[test]
fn incident_response_runbook_records_governed_event() {
    common::assert_runbook_contract(
        contract(),
        IncidentResponseRunbookCommand::record("incident response drill"),
        append_incident_response_runbook_event,
    );
}
