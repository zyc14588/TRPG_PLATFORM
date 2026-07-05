mod common;

use trpg_ruleset_coc7::ruleset_pack_sdk::{
    coc7_ruleset_pack_sdk_contract, record_ruleset_pack_sdk_registered,
    validate_ruleset_pack_sdk_contract,
};

#[test]
fn ruleset_pack_sdk_requires_tool_gate_and_blocks_provider_access() {
    let contract = coc7_ruleset_pack_sdk_contract();

    validate_ruleset_pack_sdk_contract(&contract).unwrap();
    assert_eq!(contract.ruleset_id, "coc7");
    assert!(contract.tool_gate_required);
    assert!(!contract.extension_direct_state_write_allowed);
    assert!(!contract.provider_access_allowed);
}

#[test]
fn ruleset_pack_sdk_rejects_direct_extension_state_writes() {
    let mut contract = coc7_ruleset_pack_sdk_contract();
    contract.extension_direct_state_write_allowed = true;

    assert!(validate_ruleset_pack_sdk_contract(&contract).is_err());
}

#[test]
fn ruleset_pack_sdk_registration_is_event_logged() {
    let authority = common::human_contract();
    let mut store = common::event_store();
    let command = common::rules_command("sdk");
    let sdk_contract = coc7_ruleset_pack_sdk_contract();

    let event = record_ruleset_pack_sdk_registered(&authority, &mut store, &command, &sdk_contract)
        .unwrap();

    assert_eq!(event.event_type, "coc7.ruleset_pack_sdk_registered");
    assert_eq!(event.payload.decision_type, "ruleset_pack_sdk");
}
