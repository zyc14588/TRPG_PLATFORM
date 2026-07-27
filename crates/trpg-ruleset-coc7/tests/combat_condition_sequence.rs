use trpg_ruleset_coc7::combat_state_machine::{
    apply_damage, apply_damage_with_armor, recover_major_wound, CombatCondition, CombatState,
    CombatantState,
};
use trpg_shared_kernel::TrpgError;

#[test]
fn major_wound_survives_later_small_damage_until_explicit_recovery() {
    let first = apply_damage(12, 12, 6, CombatCondition::Able).unwrap();
    assert_eq!(first.condition, CombatCondition::MajorWound);

    let second = apply_damage(first.after_hp, 12, 1, CombatCondition::MajorWound).unwrap();
    assert_eq!(second.after_hp, 5);
    assert_eq!(second.condition, CombatCondition::MajorWound);

    assert_eq!(
        recover_major_wound(second.after_hp, second.condition, false).unwrap_err(),
        TrpgError::InvalidConfiguration("major_wound_recovery")
    );
    assert_eq!(
        recover_major_wound(second.after_hp, second.condition, true).unwrap(),
        CombatCondition::Able
    );
}

#[test]
fn armor_and_multi_character_turns_are_persistent_aggregate_state() {
    let investigator = CombatantState::new("character_ada", 70, 12, 2).unwrap();
    let ally = CombatantState::new("character_bryn", 60, 10, 0).unwrap();
    let creature = CombatantState::new("npc_salt_wight", 50, 14, 1).unwrap();
    let mut combat =
        CombatState::start("combat_tutorial_cellar", vec![creature, ally, investigator]).unwrap();

    assert_eq!(combat.current_actor(), Some("character_ada"));
    assert_eq!(combat.advance_turn().unwrap(), "character_bryn");
    assert_eq!(combat.advance_turn().unwrap(), "npc_salt_wight");
    assert_eq!(combat.advance_turn().unwrap(), "character_ada");
    assert_eq!(combat.round(), 2);

    let damage = combat.apply_damage("character_ada", 8).unwrap();
    assert_eq!(damage.armor_absorbed, 2);
    assert_eq!(damage.damage, 6);
    assert_eq!(damage.condition, CombatCondition::MajorWound);
    let follow_up = combat.apply_damage("character_ada", 2).unwrap();
    assert_eq!(follow_up.damage, 0);
    assert_eq!(follow_up.condition, CombatCondition::MajorWound);

    let direct = apply_damage_with_armor(12, 12, 4, 4, CombatCondition::Able).unwrap();
    assert_eq!(direct.after_hp, 12);
    assert_eq!(direct.damage, 0);
}

#[test]
fn a_rejected_turn_advance_does_not_mutate_the_aggregate() {
    let first = CombatantState::new("character_ada", 70, 5, 0).unwrap();
    let second = CombatantState::new("npc_salt_wight", 50, 5, 0).unwrap();
    let mut combat = CombatState::start("combat_no_actor", vec![first, second]).unwrap();
    combat.apply_damage("character_ada", 5).unwrap();
    combat.apply_damage("npc_salt_wight", 5).unwrap();
    let before = combat.clone();

    assert_eq!(
        combat.advance_turn().unwrap_err(),
        TrpgError::InvalidConfiguration("combat_no_active_participant")
    );
    assert_eq!(combat, before);
}
