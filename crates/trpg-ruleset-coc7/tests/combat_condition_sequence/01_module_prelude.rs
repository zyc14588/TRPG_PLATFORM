use trpg_ruleset_coc7::combat_state_machine::{
    apply_damage, apply_damage_with_armor, recover_major_wound, CombatActionKind, CombatCondition,
    CombatDamageFormula, CombatDefense, CombatHealth, CombatMedicalSkill, CombatSkillTargets,
    CombatState, CombatWeapon, CombatWeaponLoadout, CombatantState,
};
use trpg_ruleset_coc7::dice_roll_contract::{success_level, SuccessLevel};
use trpg_shared_kernel::{
    server_damage_roll, server_percentile_roll, ServerDamageRoll, ServerPercentileRoll, TrpgError,
};

fn percentile_with_result(target: u8, succeeds: bool) -> ServerPercentileRoll {
    loop {
        let roll = server_percentile_roll().unwrap();
        let outcome = success_level(roll.value(), target).unwrap();
        let actual = matches!(
            outcome,
            SuccessLevel::Critical
                | SuccessLevel::Extreme
                | SuccessLevel::Hard
                | SuccessLevel::Regular
        );
        if actual == succeeds {
            return roll;
        }
    }
}

fn percentile_with_level(target: u8, expected: SuccessLevel) -> ServerPercentileRoll {
    loop {
        let roll = server_percentile_roll().unwrap();
        if success_level(roll.value(), target).unwrap() == expected {
            return roll;
        }
    }
}

fn combatant(
    participant_id: &str,
    dexterity: u8,
    max_hp: u8,
    armor: u8,
    melee: u8,
    firearm: u8,
    dodge: u8,
) -> CombatantState {
    CombatantState::new(
        participant_id,
        dexterity,
        CombatHealth::new(max_hp, max_hp, CombatCondition::Able).unwrap(),
        armor,
        CombatSkillTargets::new(melee, firearm, dodge, 30, 10).unwrap(),
        standard_weapon_loadout(),
    )
    .unwrap()
}

fn weapon_loadout(melee_bonus: i8, firearm_bonus: i8) -> CombatWeaponLoadout {
    CombatWeaponLoadout::new(
        CombatWeapon::new(
            "selected_melee_weapon",
            CombatDamageFormula::new(1, 6, melee_bonus).unwrap(),
        )
        .unwrap(),
        CombatWeapon::new(
            "selected_firearm",
            CombatDamageFormula::new(1, 6, firearm_bonus).unwrap(),
        )
        .unwrap(),
    )
    .unwrap()
}

fn standard_weapon_loadout() -> CombatWeaponLoadout {
    weapon_loadout(0, 5)
}

fn firearm_damage_with_value(value: u8) -> ServerDamageRoll {
    loop {
        let roll = server_damage_roll(1, 6, 5).unwrap();
        if roll.value() == value {
            return roll;
        }
    }
}

fn melee_damage_at_most(value: u8) -> ServerDamageRoll {
    loop {
        let roll = server_damage_roll(1, 6, 0).unwrap();
        if roll.value() <= value {
            return roll;
        }
    }
}

fn melee_damage_with_value(value: u8) -> ServerDamageRoll {
    loop {
        let roll = server_damage_roll(1, 6, 0).unwrap();
        if roll.value() == value {
            return roll;
        }
    }
}

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
fn a_new_encounter_preserves_persisted_health_and_condition() {
    let previous_wounded = CombatantState::new(
        "character_wounded_previous_encounter",
        70,
        CombatHealth::new(5, 12, CombatCondition::MajorWound).unwrap(),
        1,
        CombatSkillTargets::new(45, 35, 40, 30, 10).unwrap(),
        standard_weapon_loadout(),
    )
    .unwrap();
    let wounded = CombatantState::new(
        "character_wounded_between_encounters",
        70,
        previous_wounded.health(),
        1,
        CombatSkillTargets::new(45, 35, 40, 30, 10).unwrap(),
        standard_weapon_loadout(),
    )
    .unwrap();
    let dying = CombatantState::new(
        "character_dying_between_encounters",
        60,
        CombatHealth::new(0, 10, CombatCondition::Dying).unwrap(),
        0,
        CombatSkillTargets::new(40, 30, 35, 25, 10).unwrap(),
        standard_weapon_loadout(),
    )
    .unwrap();
    let encounter = CombatState::start("combat_follow_up_encounter", vec![wounded, dying]).unwrap();

    assert_eq!(encounter.participants()[0].current_hp(), 5);
    assert_eq!(
        encounter.participants()[0].condition(),
        CombatCondition::MajorWound
    );
    assert_eq!(encounter.participants()[1].current_hp(), 0);
    assert_eq!(
        encounter.participants()[1].condition(),
        CombatCondition::Dying
    );
    encounter.validate_persistence_transition(None).unwrap();

    assert!(CombatHealth::new(0, 10, CombatCondition::Able).is_err());
}

#[test]
fn attacks_reject_dead_targets_before_roll_resolution() {
    let attacker = CombatantState::new(
        "character_living_attacker",
        80,
        CombatHealth::new(10, 10, CombatCondition::Able).unwrap(),
        0,
        CombatSkillTargets::new(60, 50, 40, 30, 10).unwrap(),
        standard_weapon_loadout(),
    )
    .unwrap();
    let dead_target = CombatantState::new(
        "character_dead_target",
        60,
        CombatHealth::new(0, 10, CombatCondition::Dead).unwrap(),
        0,
        CombatSkillTargets::new(40, 30, 35, 20, 10).unwrap(),
        standard_weapon_loadout(),
    )
    .unwrap();
    let mut combat =
        CombatState::start("combat_dead_target_guard", vec![attacker, dead_target]).unwrap();
    let unchanged = combat.persistence_json().unwrap();

    assert_eq!(
        combat
            .apply_damage(
                "character_dead_target",
                CombatActionKind::Melee,
                CombatDefense::None,
                &percentile_with_result(60, false),
                None,
                None,
            )
            .unwrap_err(),
        TrpgError::InvalidConfiguration("combat_target_dead")
    );
    assert_eq!(combat.persistence_json().unwrap(), unchanged);

    assert_eq!(
        combat
            .apply_damage(
                "character_dead_target",
                CombatActionKind::Melee,
                CombatDefense::None,
                &percentile_with_result(60, true),
                None,
                Some(&melee_damage_with_value(1)),
            )
            .unwrap_err(),
        TrpgError::InvalidConfiguration("combat_target_dead")
    );
    assert_eq!(
        combat.persistence_json().unwrap(),
        unchanged,
        "hit and miss rolls must both reject a dead target without consuming the turn"
    );
}

#[test]
fn first_aid_stabilizes_a_dying_investigator_without_erasing_the_major_wound() {
    let healer = CombatantState::new(
        "character_first_aid_healer",
        80,
        CombatHealth::new(10, 10, CombatCondition::Able).unwrap(),
        0,
        CombatSkillTargets::new(40, 30, 35, 60, 10).unwrap(),
        standard_weapon_loadout(),
    )
    .unwrap();
    let patient = CombatantState::new(
        "character_dying_patient",
        60,
        CombatHealth::new(0, 12, CombatCondition::Dying).unwrap(),
        0,
        CombatSkillTargets::new(40, 30, 35, 20, 10).unwrap(),
        standard_weapon_loadout(),
    )
    .unwrap();
    let mut combat = CombatState::start("combat_dying_first_aid", vec![healer, patient]).unwrap();
    let initial = combat.persistence_json().unwrap();

    assert_eq!(
        combat
            .recover_major_wound(
                "character_first_aid_healer",
                "character_dying_patient",
                CombatMedicalSkill::Medicine,
                &percentile_with_result(10, true),
            )
            .unwrap_err(),
        TrpgError::InvalidConfiguration("major_wound_recovery"),
        "Medicine cannot replace the immediate First Aid stabilization step"
    );
    assert_eq!(combat.persistence_json().unwrap(), initial);

    assert_eq!(
        combat
            .recover_major_wound(
                "character_first_aid_healer",
                "character_dying_patient",
                CombatMedicalSkill::FirstAid,
                &percentile_with_result(60, true),
            )
            .unwrap(),
        CombatCondition::MajorWound
    );
    let stabilized = combat
        .participants()
        .iter()
        .find(|participant| participant.participant_id() == "character_dying_patient")
        .unwrap();
    assert_eq!(stabilized.current_hp(), 1);
    assert_eq!(stabilized.condition(), CombatCondition::MajorWound);
    combat
        .validate_persistence_transition(Some(&initial))
        .expect("the stabilization transition must replay from canonical state");
}
