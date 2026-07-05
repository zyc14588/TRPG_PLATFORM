mod common;

use trpg_ruleset_coc7::rule_runtime_coc7_ruleset_pack::{
    coc7_ruleset_pack, record_ruleset_pack_loaded, validate_ruleset_pack,
};

#[test]
fn ruleset_pack_declares_strict_governance_capabilities() {
    let pack = coc7_ruleset_pack();

    validate_ruleset_pack(&pack).unwrap();
    assert!(pack.server_dice_required);
    assert!(pack.event_log_required);
    assert!(!pack.direct_llm_allowed);
}

#[test]
fn ruleset_pack_load_is_recorded() {
    let contract = common::human_contract();
    let mut store = common::event_store();
    let command = common::rules_command("pack");
    let pack = coc7_ruleset_pack();

    let event = record_ruleset_pack_loaded(&contract, &mut store, &command, &pack).unwrap();

    assert_eq!(
        event.payload.decision_type,
        "rule_runtime_coc7_ruleset_pack"
    );
}
