mod common;

use trpg_ruleset_coc7::chase_state_machine::{advance_chase, record_chase_transition, ChaseStatus};

#[test]
fn chase_obstacle_can_move_quarry_to_caught_state() {
    let transition = advance_chase(1, false, true, 1).unwrap();

    assert_eq!(transition.after_range, 0);
    assert_eq!(transition.status, ChaseStatus::Caught);
}

#[test]
fn chase_transition_records_event_store_history() {
    let contract = common::human_contract();
    let mut store = common::event_store();
    let command = common::rules_command("chase");
    let transition = advance_chase(3, true, false, 0).unwrap();

    let event = record_chase_transition(&contract, &mut store, &command, &transition).unwrap();

    assert_eq!(event.payload.decision_type, "chase_state_machine");
}
