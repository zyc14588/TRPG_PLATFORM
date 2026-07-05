mod common;

use trpg_ruleset_coc7::coc7_rule_runtime::{
    coc7_runtime_governance, record_coc7_runtime_governance, validate_coc7_runtime_governance,
};

#[test]
fn runtime_governance_blocks_direct_llm_and_state_write() {
    let governance = coc7_runtime_governance();

    validate_coc7_runtime_governance(&governance).unwrap();
    assert!(governance.gateway_required);
    assert!(!governance.direct_llm_allowed);
    assert!(!governance.direct_state_write_allowed);
}

#[test]
fn runtime_governance_is_formally_recorded() {
    let contract = common::human_contract();
    let mut store = common::event_store();
    let command = common::rules_command("governance");
    let governance = coc7_runtime_governance();

    let event =
        record_coc7_runtime_governance(&contract, &mut store, &command, &governance).unwrap();

    assert_eq!(event.payload.decision_type, "coc7_rule_runtime");
}
