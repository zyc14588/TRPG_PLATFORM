
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
