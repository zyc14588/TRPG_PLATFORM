mod common;

use trpg_ruleset_coc7::combat_state_machine::{
    apply_damage, record_combat_transition, CombatCondition,
};

#[test]
fn combat_damage_marks_major_wound_before_zero_hp() {
    let transition = apply_damage(12, 12, 6).unwrap();

    assert_eq!(transition.after_hp, 6);
    assert_eq!(transition.condition, CombatCondition::MajorWound);
}

#[test]
fn combat_transition_records_formal_event() {
    let contract = common::human_contract();
    let mut store = common::event_store();
    let command = common::rules_command("combat");
    let transition = apply_damage(12, 12, 4).unwrap();

    let event = record_combat_transition(&contract, &mut store, &command, &transition).unwrap();

    assert_eq!(event.event_type, "coc7.combat_transition_recorded");
}
