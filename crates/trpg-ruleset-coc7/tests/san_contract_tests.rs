mod common;

use trpg_ruleset_coc7::san::{record_san_decision, resolve_san_check, san_check_succeeds};
use trpg_ruleset_coc7::sanity_madness_state_machine::MadnessState;

#[test]
fn san_check_applies_success_and_failure_loss() {
    assert!(san_check_succeeds(20, 60).unwrap());

    let transition = resolve_san_check(80, 60, 1, 6, 0).unwrap();

    assert_eq!(transition.loss, 6);
    assert_eq!(transition.state, MadnessState::TemporaryInsanity);
}

#[test]
fn san_decision_keeps_event_visibility_and_provenance() {
    let contract = common::human_contract();
    let mut store = common::event_store();
    let command = common::rules_command("san");
    let transition = resolve_san_check(20, 60, 1, 6, 0).unwrap();

    let event = record_san_decision(&contract, &mut store, &command, &transition).unwrap();

    assert_eq!(event.payload.ruleset_id, "coc7");
    assert_eq!(event.payload.visibility_label, "system_only");
}
