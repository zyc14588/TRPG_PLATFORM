use trpg_ruleset_coc7::combat_state_machine::{
    apply_damage, apply_damage_with_armor, recover_major_wound, CombatActionKind, CombatCondition,
    CombatDefense, CombatSkillTargets, CombatState, CombatantState,
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
        max_hp,
        armor,
        CombatSkillTargets::new(melee, firearm, dodge).unwrap(),
    )
    .unwrap()
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
            &damage_roll,
        )
        .unwrap();
    assert_eq!(damage.armor_absorbed, 2);
    assert_eq!(damage.damage, 6);
    assert_eq!(damage.condition, CombatCondition::MajorWound);
    let follow_up_attack = percentile_with_result(50, true);
    let follow_up_roll = melee_damage_at_most(2);
    let follow_up = combat
        .apply_damage(
            "character_ada",
            CombatActionKind::Melee,
            CombatDefense::None,
            &follow_up_attack,
            None,
            &follow_up_roll,
        )
        .unwrap();
    assert_eq!(follow_up.damage, 0);
    assert_eq!(follow_up.condition, CombatCondition::MajorWound);

    let forged_formula = server_damage_roll(1, 6, 0).unwrap();
    assert_eq!(
        combat
            .apply_damage(
                "character_ada",
                CombatActionKind::Firearm,
                CombatDefense::None,
                &percentile_with_result(50, true),
                None,
                &forged_formula,
            )
            .unwrap_err(),
        TrpgError::InvalidConfiguration("combat_damage_evidence")
    );

    let direct = apply_damage_with_armor(12, 12, 4, 4, CombatCondition::Able).unwrap();
    assert_eq!(direct.after_hp, 12);
    assert_eq!(direct.damage, 0);
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
    assert_eq!(
        skill_bound
            .apply_damage(
                "character_defender",
                CombatActionKind::Firearm,
                CombatDefense::None,
                &dex_success_skill_failure,
                None,
                &firearm_damage_with_value(6),
            )
            .unwrap_err(),
        TrpgError::InvalidConfiguration("combat_attack_missed")
    );
    assert_eq!(skill_bound.persistence_json().unwrap(), before);

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
            &server_damage_roll(1, 6, 0).unwrap(),
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
            &server_damage_roll(1, 6, 0).unwrap(),
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
            &melee_damage_with_value(5),
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
                &melee_damage_with_value(1),
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
