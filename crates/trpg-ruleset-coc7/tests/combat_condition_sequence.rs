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

#[test]
fn armor_and_multi_character_turns_are_persistent_aggregate_state() {
    let investigator = combatant("character_ada", 70, 12, 2, 45, 60, 35);
    let ally = combatant("character_bryn", 60, 10, 0, 50, 40, 30);
    let creature = combatant("npc_salt_wight", 50, 14, 1, 50, 50, 25);
    let mut combat =
        CombatState::start("combat_tutorial_cellar", vec![creature, ally, investigator]).unwrap();

    assert_eq!(combat.current_actor(), Some("character_ada"));
    assert_eq!(combat.advance_turn().unwrap(), "character_bryn");
    assert_eq!(combat.advance_turn().unwrap(), "npc_salt_wight");
    assert_eq!(combat.advance_turn().unwrap(), "character_ada");
    assert_eq!(combat.round(), 2);
    assert_eq!(combat.advance_turn().unwrap(), "character_bryn");
    assert_eq!(combat.advance_turn().unwrap(), "npc_salt_wight");

    let attack = percentile_with_result(50, true);
    let damage_roll = firearm_damage_with_value(8);
    let damage = combat
        .apply_damage(
            "character_ada",
            CombatActionKind::Firearm,
            CombatDefense::None,
            &attack,
            None,
            Some(&damage_roll),
        )
        .unwrap();
    assert_eq!(damage.armor_absorbed, 2);
    assert_eq!(damage.damage, 6);
    assert_eq!(damage.condition, CombatCondition::MajorWound);
    let follow_up_attack = percentile_with_result(50, true);
    let follow_up_roll = melee_damage_at_most(2);
    assert_eq!(
        combat
            .apply_damage(
                "character_ada",
                CombatActionKind::Melee,
                CombatDefense::None,
                &follow_up_attack,
                None,
                Some(&follow_up_roll),
            )
            .unwrap_err(),
        TrpgError::InvalidConfiguration("combat_turn_action_consumed")
    );
    assert_eq!(combat.advance_turn().unwrap(), "character_ada");
    assert_eq!(combat.advance_turn().unwrap(), "character_bryn");
    let follow_up = combat
        .apply_damage(
            "character_ada",
            CombatActionKind::Melee,
            CombatDefense::None,
            &follow_up_attack,
            None,
            Some(&follow_up_roll),
        )
        .unwrap();
    assert_eq!(follow_up.damage, 0);
    assert_eq!(follow_up.condition, CombatCondition::MajorWound);

    assert_eq!(combat.advance_turn().unwrap(), "npc_salt_wight");
    let forged_formula = server_damage_roll(1, 6, 0).unwrap();
    assert_eq!(
        combat
            .apply_damage(
                "character_ada",
                CombatActionKind::Firearm,
                CombatDefense::None,
                &percentile_with_result(50, true),
                None,
                Some(&forged_formula),
            )
            .unwrap_err(),
        TrpgError::InvalidConfiguration("combat_damage_evidence")
    );

    let direct = apply_damage_with_armor(12, 12, 4, 4, CombatCondition::Able).unwrap();
    assert_eq!(direct.after_hp, 12);
    assert_eq!(direct.damage, 0);
}

#[test]
fn damage_evidence_is_bound_to_the_actual_damage_dealers_selected_weapon() {
    let attacker = CombatantState::new(
        "fixture_attacker",
        90,
        CombatHealth::new(12, 12, CombatCondition::Able).unwrap(),
        0,
        CombatSkillTargets::new(60, 60, 40, 30, 10).unwrap(),
        weapon_loadout(1, 5),
    )
    .unwrap();
    let defender = CombatantState::new(
        "fixture_defender",
        70,
        CombatHealth::new(12, 12, CombatCondition::Able).unwrap(),
        0,
        CombatSkillTargets::new(80, 40, 55, 30, 10).unwrap(),
        weapon_loadout(2, 5),
    )
    .unwrap();
    let mut direct = CombatState::start(
        "combat_fixture_weapon_formula",
        vec![attacker.clone(), defender.clone()],
    )
    .unwrap();
    let direct_attack = percentile_with_result(60, true);
    let wrong_action_default = server_damage_roll(1, 6, 0).unwrap();
    let before_wrong_formula = direct.persistence_json().unwrap();
    assert_eq!(
        direct
            .apply_damage(
                "fixture_defender",
                CombatActionKind::Melee,
                CombatDefense::None,
                &direct_attack,
                None,
                Some(&wrong_action_default),
            )
            .unwrap_err(),
        TrpgError::InvalidConfiguration("combat_damage_evidence")
    );
    assert_eq!(direct.persistence_json().unwrap(), before_wrong_formula);
    direct
        .apply_damage(
            "fixture_defender",
            CombatActionKind::Melee,
            CombatDefense::None,
            &direct_attack,
            None,
            Some(&server_damage_roll(1, 6, 1).unwrap()),
        )
        .expect("the active fixture's 1d6+1 melee weapon must be accepted");

    let mut fight_back =
        CombatState::start("combat_fixture_fight_back_weapon", vec![attacker, defender]).unwrap();
    let fight_back_attack = percentile_with_level(60, SuccessLevel::Regular);
    let fight_back_defense = percentile_with_level(80, SuccessLevel::Hard);
    let before_wrong_counter = fight_back.persistence_json().unwrap();
    assert_eq!(
        fight_back
            .apply_damage(
                "fixture_defender",
                CombatActionKind::Melee,
                CombatDefense::FightBack,
                &fight_back_attack,
                Some(&fight_back_defense),
                Some(&server_damage_roll(1, 6, 1).unwrap()),
            )
            .unwrap_err(),
        TrpgError::InvalidConfiguration("combat_damage_evidence"),
        "a winning defender must use its own selected melee weapon, not the attacker's"
    );
    assert_eq!(fight_back.persistence_json().unwrap(), before_wrong_counter);
    fight_back
        .apply_damage(
            "fixture_defender",
            CombatActionKind::Melee,
            CombatDefense::FightBack,
            &fight_back_attack,
            Some(&fight_back_defense),
            Some(&server_damage_roll(1, 6, 2).unwrap()),
        )
        .expect("the defender's selected counterattack weapon must supply damage");
}

#[test]
fn a_rejected_turn_advance_does_not_mutate_the_aggregate() {
    let first = combatant("character_ada", 70, 5, 0, 50, 50, 35);
    let second = combatant("npc_salt_wight", 50, 5, 0, 50, 50, 25);
    let mut combat = CombatState::start("combat_no_actor", vec![first, second]).unwrap();
    combat.end().unwrap();
    let before = combat.persistence_json().unwrap();

    assert_eq!(
        combat.advance_turn().unwrap_err(),
        TrpgError::InvalidConfiguration("combat_terminal")
    );
    assert_eq!(combat.persistence_json().unwrap(), before);
}

#[test]
fn formal_combat_uses_combat_skills_and_models_fight_back() {
    let attacker = combatant("character_attacker", 90, 12, 0, 50, 20, 30);
    let defender = combatant("character_defender", 60, 12, 0, 80, 40, 55);
    let mut skill_bound = CombatState::start(
        "combat_skill_bound",
        vec![attacker.clone(), defender.clone()],
    )
    .unwrap();
    let dex_success_skill_failure = loop {
        let roll = server_percentile_roll().unwrap();
        if success_level(roll.value(), 90).unwrap() == SuccessLevel::Regular
            && matches!(
                success_level(roll.value(), 20).unwrap(),
                SuccessLevel::Failure | SuccessLevel::Fumble
            )
        {
            break roll;
        }
    };
    let before = skill_bound.persistence_json().unwrap();
    let missed = skill_bound
        .apply_damage(
            "character_defender",
            CombatActionKind::Firearm,
            CombatDefense::None,
            &dex_success_skill_failure,
            None,
            None,
        )
        .unwrap();
    assert_eq!(missed.before_hp, missed.after_hp);
    assert_eq!(missed.raw_damage, 0);
    assert_eq!(missed.damage, 0);
    assert_eq!(skill_bound.version(), 2);
    let after = skill_bound.persistence_json().unwrap();
    assert_ne!(after, before);
    assert!(after.contains("\"kind\":\"ATTACK_MISSED\""));
    CombatState::validate_serialized_persistence_transition(Some(&before), &after)
        .expect("a failed attack is a canonical no-damage transition with roll evidence");
    assert!(
        after.contains(dex_success_skill_failure.roll_id()),
        "the missed attack must retain its server-generated roll evidence"
    );
    assert_eq!(
        skill_bound
            .apply_damage(
                "character_defender",
                CombatActionKind::Firearm,
                CombatDefense::None,
                &percentile_with_result(20, true),
                None,
                Some(&firearm_damage_with_value(6)),
            )
            .unwrap_err(),
        TrpgError::InvalidConfiguration("combat_turn_action_consumed")
    );

    let mut tied_fight_back = CombatState::start(
        "combat_fight_back_tie",
        vec![attacker.clone(), defender.clone()],
    )
    .unwrap();
    tied_fight_back
        .apply_damage(
            "character_defender",
            CombatActionKind::Melee,
            CombatDefense::FightBack,
            &percentile_with_level(50, SuccessLevel::Regular),
            Some(&percentile_with_level(80, SuccessLevel::Regular)),
            Some(&server_damage_roll(1, 6, 0).unwrap()),
        )
        .unwrap();
    assert_eq!(tied_fight_back.participants()[0].current_hp(), 12);
    assert!(tied_fight_back.participants()[1].current_hp() < 12);

    let mut winning_fight_back =
        CombatState::start("combat_fight_back_win", vec![attacker, defender]).unwrap();
    winning_fight_back
        .apply_damage(
            "character_defender",
            CombatActionKind::Melee,
            CombatDefense::FightBack,
            &percentile_with_level(50, SuccessLevel::Regular),
            Some(&percentile_with_level(80, SuccessLevel::Hard)),
            Some(&server_damage_roll(1, 6, 0).unwrap()),
        )
        .unwrap();
    assert!(winning_fight_back.participants()[0].current_hp() < 12);
    assert_eq!(winning_fight_back.participants()[1].current_hp(), 12);

    let incapacitated_attacker = combatant("character_incapacitated", 90, 5, 0, 50, 20, 30);
    let counterattacker = combatant("character_counterattacker", 60, 12, 0, 80, 40, 55);
    let mut incapacitation = CombatState::start(
        "combat_fight_back_incapacitation",
        vec![incapacitated_attacker, counterattacker],
    )
    .unwrap();
    incapacitation
        .apply_damage(
            "character_counterattacker",
            CombatActionKind::Melee,
            CombatDefense::FightBack,
            &percentile_with_level(50, SuccessLevel::Regular),
            Some(&percentile_with_level(80, SuccessLevel::Hard)),
            Some(&melee_damage_with_value(5)),
        )
        .unwrap();
    assert!(!incapacitation.participants()[0].condition().can_act());
    let before_rejected_attack = incapacitation.persistence_json().unwrap();
    assert_eq!(
        incapacitation
            .apply_damage(
                "character_counterattacker",
                CombatActionKind::Melee,
                CombatDefense::None,
                &percentile_with_level(50, SuccessLevel::Regular),
                None,
                Some(&melee_damage_with_value(1)),
            )
            .unwrap_err(),
        TrpgError::InvalidConfiguration("combat_actor_incapacitated")
    );
    assert_eq!(
        incapacitation.persistence_json().unwrap(),
        before_rejected_attack,
        "an incapacitated current actor must advance turn before another attack"
    );
}

#[test]
fn defenses_medical_targets_and_roll_ids_are_derived_and_single_use() {
    let attacker = combatant("attacker", 90, 10, 0, 60, 60, 40);
    let defender = combatant("defender", 70, 5, 0, 60, 60, 40);
    let third = combatant("third", 50, 10, 0, 60, 60, 40);
    let mut incapacitated_defense = CombatState::start(
        "combat_incapacitated_defense",
        vec![attacker, defender, third],
    )
    .unwrap();
    incapacitated_defense
        .apply_damage(
            "defender",
            CombatActionKind::Melee,
            CombatDefense::None,
            &percentile_with_result(60, true),
            None,
            Some(&melee_damage_with_value(5)),
        )
        .unwrap();
    assert!(!incapacitated_defense.participants()[1]
        .condition()
        .can_act());
    assert_eq!(incapacitated_defense.advance_turn().unwrap(), "third");
    assert_eq!(
        incapacitated_defense
            .apply_damage(
                "defender",
                CombatActionKind::Melee,
                CombatDefense::Dodge,
                &percentile_with_result(60, true),
                Some(&percentile_with_result(40, true)),
                Some(&melee_damage_with_value(1)),
            )
            .unwrap_err(),
        TrpgError::InvalidConfiguration("combat_defender_incapacitated")
    );

    let equal_a = combatant("equal_a", 90, 10, 0, 60, 60, 40);
    let equal_b = combatant("equal_b", 70, 10, 0, 60, 60, 40);
    let mut roll_ledger = CombatState::start("combat_roll_ledger", vec![equal_a, equal_b]).unwrap();
    let consumed_attack = percentile_with_result(60, true);
    roll_ledger
        .apply_damage(
            "equal_b",
            CombatActionKind::Melee,
            CombatDefense::None,
            &consumed_attack,
            None,
            Some(&melee_damage_with_value(1)),
        )
        .unwrap();
    assert_eq!(roll_ledger.advance_turn().unwrap(), "equal_b");
    assert_eq!(
        roll_ledger
            .apply_damage(
                "equal_a",
                CombatActionKind::Melee,
                CombatDefense::None,
                &consumed_attack,
                None,
                Some(&melee_damage_with_value(1)),
            )
            .unwrap_err(),
        TrpgError::InvalidConfiguration("combat_roll_reuse")
    );

    let wounder = CombatantState::new(
        "wounder",
        90,
        CombatHealth::new(10, 10, CombatCondition::Able).unwrap(),
        0,
        CombatSkillTargets::new(60, 60, 40, 30, 10).unwrap(),
        standard_weapon_loadout(),
    )
    .unwrap();
    let patient = CombatantState::new(
        "patient",
        70,
        CombatHealth::new(10, 10, CombatCondition::Able).unwrap(),
        0,
        CombatSkillTargets::new(60, 60, 40, 30, 10).unwrap(),
        standard_weapon_loadout(),
    )
    .unwrap();
    let healer = CombatantState::new(
        "healer",
        50,
        CombatHealth::new(10, 10, CombatCondition::Able).unwrap(),
        0,
        CombatSkillTargets::new(60, 60, 40, 20, 5).unwrap(),
        standard_weapon_loadout(),
    )
    .unwrap();
    let mut medical = CombatState::start("combat_medical", vec![wounder, patient, healer]).unwrap();
    medical
        .apply_damage(
            "patient",
            CombatActionKind::Melee,
            CombatDefense::None,
            &percentile_with_result(60, true),
            None,
            Some(&melee_damage_with_value(5)),
        )
        .unwrap();
    assert_eq!(medical.advance_turn().unwrap(), "patient");
    assert_eq!(medical.advance_turn().unwrap(), "healer");
    let inflated_only_roll = loop {
        let roll = server_percentile_roll().unwrap();
        if success_level(roll.value(), 100).unwrap() == SuccessLevel::Regular
            && matches!(
                success_level(roll.value(), 20).unwrap(),
                SuccessLevel::Failure | SuccessLevel::Fumble
            )
        {
            break roll;
        }
    };
    assert_eq!(
        medical
            .recover_major_wound(
                "healer",
                "patient",
                CombatMedicalSkill::FirstAid,
                &inflated_only_roll,
            )
            .unwrap(),
        CombatCondition::MajorWound,
        "a caller cannot inflate the persisted healer's First Aid target"
    );
    let failed_medical_state = medical.persistence_json().unwrap();
    assert!(failed_medical_state.contains("\"target\":20"));
    assert!(failed_medical_state.contains("\"recovered\":false"));
    assert_eq!(medical.advance_turn().unwrap(), "wounder");
    assert_eq!(medical.advance_turn().unwrap(), "patient");
    assert_eq!(medical.advance_turn().unwrap(), "healer");
    assert_eq!(
        medical
            .recover_major_wound(
                "healer",
                "patient",
                CombatMedicalSkill::FirstAid,
                &inflated_only_roll,
            )
            .unwrap_err(),
        TrpgError::InvalidConfiguration("combat_roll_reuse")
    );
}
