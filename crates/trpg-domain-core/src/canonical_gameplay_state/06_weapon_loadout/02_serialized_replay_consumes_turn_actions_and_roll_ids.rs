
    #[test]
    fn serialized_replay_consumes_turn_actions_and_roll_ids() {
        let mut first = CombatSnapshot {
            combat_id: "combat_consumption".to_owned(),
            participants: vec![
                Combatant {
                    participant_id: "first".to_owned(),
                    dexterity: 80,
                    skill_targets: CombatSkillTargets {
                        melee: 60,
                        firearm: 50,
                        dodge: 40,
                        first_aid: 30,
                        medicine: 10,
                    },
                    weapon_loadout: weapon_loadout(0, 5),
                    current_hp: 10,
                    max_hp: 10,
                    armor: 0,
                    condition: CombatCondition::Able,
                },
                Combatant {
                    participant_id: "second".to_owned(),
                    dexterity: 50,
                    skill_targets: CombatSkillTargets {
                        melee: 60,
                        firearm: 50,
                        dodge: 40,
                        first_aid: 30,
                        medicine: 10,
                    },
                    weapon_loadout: weapon_loadout(0, 5),
                    current_hp: 10,
                    max_hp: 10,
                    armor: 0,
                    condition: CombatCondition::Able,
                },
            ],
            initiative_order: vec!["first".to_owned(), "second".to_owned()],
            round: 1,
            current_turn_index: 0,
            turn_action_consumed: false,
            consumed_roll_ids: Vec::new(),
            status: CombatStatus::Ongoing,
            version: 1,
            last_transition: CombatMutation::Started,
        };
        let initial_json = serde_json::to_string(&first).unwrap();
        let missed_roll = PercentileRollEvidence {
            roll_id: "consumed_attack".to_owned(),
            target: 50,
            roll: 80,
            selected_tens_digit: 8,
            ones_digit: 0,
            success_level: SuccessLevel::Failure,
        };
        apply_combat_mutation(
            &mut first,
            &CombatMutation::AttackMissed {
                attacker_id: "first".to_owned(),
                target_id: "second".to_owned(),
                action: CombatActionKind::Firearm,
                defense: CombatDefense::None,
                attacker_roll: missed_roll.clone(),
                defender_roll: None,
            },
        )
        .unwrap();
        let first_json = serde_json::to_string(&first).unwrap();
        validate_combat_state_transition(Some(&initial_json), &first_json).unwrap();

        let mut forged_second_action = first.clone();
        forged_second_action.version = 3;
        forged_second_action
            .consumed_roll_ids
            .push("fresh_attack".to_owned());
        forged_second_action.last_transition = CombatMutation::AttackMissed {
            attacker_id: "first".to_owned(),
            target_id: "second".to_owned(),
            action: CombatActionKind::Firearm,
            defense: CombatDefense::None,
            attacker_roll: PercentileRollEvidence {
                roll_id: "fresh_attack".to_owned(),
                ..missed_roll.clone()
            },
            defender_roll: None,
        };
        assert_eq!(
            validate_combat_state_transition(
                Some(&first_json),
                &serde_json::to_string(&forged_second_action).unwrap(),
            ),
            Err(CanonicalGameplayStateError::InvalidTransition)
        );

        let mut advanced = first;
        apply_combat_mutation(&mut advanced, &CombatMutation::TurnAdvanced).unwrap();
        let advanced_json = serde_json::to_string(&advanced).unwrap();
        validate_combat_state_transition(Some(&first_json), &advanced_json).unwrap();
        let mut reused_roll = advanced;
        reused_roll.version = 4;
        reused_roll.turn_action_consumed = true;
        reused_roll.last_transition = CombatMutation::AttackMissed {
            attacker_id: "second".to_owned(),
            target_id: "first".to_owned(),
            action: CombatActionKind::Firearm,
            defense: CombatDefense::None,
            attacker_roll: missed_roll,
            defender_roll: None,
        };
        assert_eq!(
            validate_combat_state_transition(
                Some(&advanced_json),
                &serde_json::to_string(&reused_roll).unwrap(),
            ),
            Err(CanonicalGameplayStateError::InvalidTransition)
        );
    }

    #[test]
    fn serialized_replay_binds_active_defense_and_medical_skill_targets() {
        let skills = CombatSkillTargets {
            melee: 60,
            firearm: 50,
            dodge: 40,
            first_aid: 20,
            medicine: 5,
        };
        let incapacitated_defender = CombatSnapshot {
            combat_id: "combat_defense_guard".to_owned(),
            participants: vec![
                Combatant {
                    participant_id: "attacker".to_owned(),
                    dexterity: 90,
                    skill_targets: skills,
                    weapon_loadout: weapon_loadout(0, 5),
                    current_hp: 10,
                    max_hp: 10,
                    armor: 0,
                    condition: CombatCondition::Able,
                },
                Combatant {
                    participant_id: "defender".to_owned(),
                    dexterity: 70,
                    skill_targets: skills,
                    weapon_loadout: weapon_loadout(0, 5),
                    current_hp: 0,
                    max_hp: 5,
                    armor: 0,
                    condition: CombatCondition::Dead,
                },
            ],
            initiative_order: vec!["attacker".to_owned(), "defender".to_owned()],
            round: 1,
            current_turn_index: 0,
            turn_action_consumed: false,
            consumed_roll_ids: Vec::new(),
            status: CombatStatus::Ongoing,
            version: 1,
            last_transition: CombatMutation::Started,
        };
        let previous_json = serde_json::to_string(&incapacitated_defender).unwrap();
        let mut forged_defense = incapacitated_defender.clone();
        forged_defense.version = 2;
        forged_defense.turn_action_consumed = true;
        forged_defense.consumed_roll_ids =
            vec!["defense_attack".to_owned(), "dead_dodge".to_owned()];
        forged_defense.last_transition = CombatMutation::AttackMissed {
            attacker_id: "attacker".to_owned(),
            target_id: "defender".to_owned(),
            action: CombatActionKind::Melee,
            defense: CombatDefense::Dodge,
            attacker_roll: PercentileRollEvidence {
                roll_id: "defense_attack".to_owned(),
                target: 60,
                roll: 40,
                selected_tens_digit: 4,
                ones_digit: 0,
                success_level: SuccessLevel::Regular,
            },
            defender_roll: Some(PercentileRollEvidence {
                roll_id: "dead_dodge".to_owned(),
                target: 40,
                roll: 20,
                selected_tens_digit: 2,
                ones_digit: 0,
                success_level: SuccessLevel::Hard,
            }),
        };
        assert_eq!(
            validate_combat_state_transition(
                Some(&previous_json),
                &serde_json::to_string(&forged_defense).unwrap(),
            ),
            Err(CanonicalGameplayStateError::InvalidTransition)
        );

        let mut forged_dead_miss = incapacitated_defender;
        forged_dead_miss.version = 2;
        forged_dead_miss.turn_action_consumed = true;
        forged_dead_miss.consumed_roll_ids = vec!["dead_target_miss".to_owned()];
        forged_dead_miss.last_transition = CombatMutation::AttackMissed {
            attacker_id: "attacker".to_owned(),
            target_id: "defender".to_owned(),
            action: CombatActionKind::Melee,
            defense: CombatDefense::None,
            attacker_roll: PercentileRollEvidence {
                roll_id: "dead_target_miss".to_owned(),
                target: 60,
                roll: 80,
                selected_tens_digit: 8,
                ones_digit: 0,
                success_level: SuccessLevel::Failure,
            },
            defender_roll: None,
        };
        assert_eq!(
            validate_combat_state_transition(
                Some(&previous_json),
                &serde_json::to_string(&forged_dead_miss).unwrap(),
            ),
            Err(CanonicalGameplayStateError::InvalidTransition),
            "a failed roll cannot turn an attack against a dead target into a valid mutation"
        );

        let medical = CombatSnapshot {
            combat_id: "combat_medical_guard".to_owned(),
            participants: vec![
                Combatant {
                    participant_id: "healer".to_owned(),
                    dexterity: 90,
                    skill_targets: skills,
                    weapon_loadout: weapon_loadout(0, 5),
                    current_hp: 10,
                    max_hp: 10,
                    armor: 0,
                    condition: CombatCondition::Able,
                },
                Combatant {
                    participant_id: "patient".to_owned(),
                    dexterity: 70,
                    skill_targets: skills,
                    weapon_loadout: weapon_loadout(0, 5),
                    current_hp: 5,
                    max_hp: 10,
                    armor: 0,
                    condition: CombatCondition::MajorWound,
                },
            ],
            initiative_order: vec!["healer".to_owned(), "patient".to_owned()],
            round: 1,
            current_turn_index: 0,
            turn_action_consumed: false,
            consumed_roll_ids: Vec::new(),
            status: CombatStatus::Ongoing,
            version: 1,
            last_transition: CombatMutation::Started,
        };
        let medical_json = serde_json::to_string(&medical).unwrap();
        let mut failed_attempt = medical.clone();
        failed_attempt.version = 2;
        failed_attempt.turn_action_consumed = true;
        failed_attempt
            .consumed_roll_ids
            .push("medical_roll".to_owned());
        failed_attempt.last_transition = CombatMutation::MajorWoundRecoveryAttempted {
            healer_id: "healer".to_owned(),
            target_id: "patient".to_owned(),
            medical_skill: CombatMedicalSkill::FirstAid,
            medical_roll: PercentileRollEvidence {
                roll_id: "medical_roll".to_owned(),
                target: 20,
                roll: 50,
                selected_tens_digit: 5,
                ones_digit: 0,
                success_level: SuccessLevel::Failure,
            },
            recovered: false,
        };
        validate_combat_state_transition(
            Some(&medical_json),
            &serde_json::to_string(&failed_attempt).unwrap(),
        )
        .expect("a failed canonical medical attempt still consumes its turn and roll");

        let mut inflated_target = failed_attempt;
        inflated_target.version = 2;
        inflated_target.last_transition = CombatMutation::MajorWoundRecoveryAttempted {
            healer_id: "healer".to_owned(),
            target_id: "patient".to_owned(),
            medical_skill: CombatMedicalSkill::FirstAid,
            medical_roll: PercentileRollEvidence {
                roll_id: "medical_roll".to_owned(),
                target: 100,
                roll: 50,
                selected_tens_digit: 5,
                ones_digit: 0,
                success_level: SuccessLevel::Hard,
            },
            recovered: true,
        };
        inflated_target.participants[1].condition = CombatCondition::Able;
        assert_eq!(
            validate_combat_state_transition(
                Some(&medical_json),
                &serde_json::to_string(&inflated_target).unwrap(),
            ),
            Err(CanonicalGameplayStateError::InvalidTransition)
        );

        let mut dying = medical;
        dying.participants[1].current_hp = 0;
        dying.participants[1].condition = CombatCondition::Dying;
        let dying_json = serde_json::to_string(&dying).unwrap();
        validate_combat_state_transition(None, &dying_json)
            .expect("a canonical encounter may begin with a persisted dying investigator");
        let mut stabilized = dying;
        stabilized.version = 2;
        stabilized.turn_action_consumed = true;
        stabilized
            .consumed_roll_ids
            .push("stabilization_roll".to_owned());
        stabilized.participants[1].current_hp = 1;
        stabilized.participants[1].condition = CombatCondition::MajorWound;
        stabilized.last_transition = CombatMutation::MajorWoundRecoveryAttempted {
            healer_id: "healer".to_owned(),
            target_id: "patient".to_owned(),
            medical_skill: CombatMedicalSkill::FirstAid,
            medical_roll: PercentileRollEvidence {
                roll_id: "stabilization_roll".to_owned(),
                target: 20,
                roll: 10,
                selected_tens_digit: 1,
                ones_digit: 0,
                success_level: SuccessLevel::Hard,
            },
            recovered: true,
        };
        validate_combat_state_transition(
            Some(&dying_json),
            &serde_json::to_string(&stabilized).unwrap(),
        )
        .expect("successful First Aid must independently replay to one HP and MajorWound");

        let mut forged_stabilization = stabilized;
        forged_stabilization.participants[1].condition = CombatCondition::Able;
        assert_eq!(
            validate_combat_state_transition(
                Some(&dying_json),
                &serde_json::to_string(&forged_stabilization).unwrap(),
            ),
            Err(CanonicalGameplayStateError::InvalidTransition),
            "First Aid cannot erase the surviving investigator's MajorWound"
        );
    }
