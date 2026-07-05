mod common;

use trpg_ruleset_coc7::readme::{
    coc7_readme_contract, record_coc7_readme_contract, validate_coc7_readme_contract,
};

#[test]
fn readme_contract_keeps_current_safe_coc7_boundaries() {
    let contract = coc7_readme_contract();

    validate_coc7_readme_contract(&contract).unwrap();
    assert_eq!(contract.ruleset_id, "coc7");
    assert_eq!(contract.command_endpoint, "/api/v1/rulesets/coc7/commands");
    assert_eq!(contract.event_store, "coc7_rule_events");
    assert!(!contract.projection_is_canon);
    assert!(!contract.direct_model_access_allowed);
}

#[test]
fn readme_contract_rejects_projection_as_canon() {
    let mut contract = coc7_readme_contract();
    contract.projection_is_canon = true;

    assert!(validate_coc7_readme_contract(&contract).is_err());
}

#[test]
fn readme_contract_is_event_logged() {
    let authority = common::human_contract();
    let mut store = common::event_store();
    let command = common::rules_command("readme");
    let readme_contract = coc7_readme_contract();

    let event =
        record_coc7_readme_contract(&authority, &mut store, &command, &readme_contract).unwrap();

    assert_eq!(event.event_type, "coc7.readme_contract_recorded");
    assert_eq!(event.payload.decision_type, "readme");
}
