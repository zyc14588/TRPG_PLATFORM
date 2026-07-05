mod common;

use trpg_ruleset_coc7::sanity_madness_state_machine::{
    apply_sanity_loss, record_sanity_madness_transition, MadnessState,
};

#[test]
fn sanity_loss_promotes_day_threshold_to_indefinite_madness() {
    let transition = apply_sanity_loss(50, 7, 3).unwrap();

    assert_eq!(transition.after, 43);
    assert_eq!(transition.state, MadnessState::IndefiniteInsanity);
}

#[test]
fn sanity_transition_is_written_through_event_store() {
    let contract = common::human_contract();
    let mut store = common::event_store();
    let command = common::rules_command("sanity");
    let transition = apply_sanity_loss(70, 5, 0).unwrap();

    let event =
        record_sanity_madness_transition(&contract, &mut store, &command, &transition).unwrap();

    assert_eq!(event.event_type, "coc7.sanity_transition_recorded");
    assert_eq!(store.events().len(), 1);
}
