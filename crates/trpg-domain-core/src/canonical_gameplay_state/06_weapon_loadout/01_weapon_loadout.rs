    use super::*;

    fn weapon_loadout(melee_bonus: i8, firearm_bonus: i8) -> CombatWeaponLoadout {
        CombatWeaponLoadout {
            melee: CombatWeapon {
                weapon_id: "selected_melee_weapon".to_owned(),
                damage_formula: CombatDamageFormula {
                    dice_count: 1,
                    die_sides: 6,
                    flat_bonus: melee_bonus,
                },
            },
            firearm: CombatWeapon {
                weapon_id: "selected_firearm".to_owned(),
                damage_formula: CombatDamageFormula {
                    dice_count: 1,
                    die_sides: 6,
                    flat_bonus: firearm_bonus,
                },
            },
        }
    }

    #[test]
    fn malformed_percentile_digits_fail_closed_without_overflow() {
        let combat_evidence = PercentileRollEvidence {
            roll_id: "malformed_combat_roll".to_owned(),
            target: 60,
            roll: 60,
            selected_tens_digit: 26,
            ones_digit: 0,
            success_level: SuccessLevel::Regular,
        };
        assert_eq!(
            validate_percentile_evidence(&combat_evidence, 60),
            Err(CanonicalGameplayStateError::InvalidTransition)
        );

        let chase_participant = ChaseParticipant {
            participant_id: "malformed_chase_participant".to_owned(),
            role: ChaseRole::Quarry,
            movement_rate: 8,
        };
        let chase_evidence = ChaseParticipantRollEvidence {
            participant_id: chase_participant.participant_id.clone(),
            roll_id: "malformed_chase_roll".to_owned(),
            target: 40,
            roll: 40,
            selected_tens_digit: 26,
            ones_digit: 0,
            success_level: SuccessLevel::Regular,
        };
        assert_eq!(
            validate_chase_roll_evidence(&chase_evidence, &chase_participant, 40),
            Err(CanonicalGameplayStateError::InvalidTransition)
        );
    }

    #[test]
    fn rejects_same_id_combat_from_an_unrelated_lineage() {
        let initial = r#"{
            "combat_id":"combat_a",
            "participants":[
                {"participant_id":"one","dexterity":70,"current_hp":10,
                 "skill_targets":{"melee":60,"firearm":55,"dodge":40,"first_aid":30,"medicine":10},
                 "weapon_loadout":{"melee":{"weapon_id":"knife","damage_formula":{"dice_count":1,"die_sides":6,"flat_bonus":1}},"firearm":{"weapon_id":"revolver","damage_formula":{"dice_count":1,"die_sides":6,"flat_bonus":5}}},
                 "max_hp":10,"armor":0,"condition":"ABLE"},
                {"participant_id":"two","dexterity":50,"current_hp":8,
                 "skill_targets":{"melee":45,"firearm":35,"dodge":25,"first_aid":30,"medicine":10},
                 "weapon_loadout":{"melee":{"weapon_id":"claw","damage_formula":{"dice_count":1,"die_sides":6,"flat_bonus":0}},"firearm":{"weapon_id":"revolver","damage_formula":{"dice_count":1,"die_sides":6,"flat_bonus":5}}},
                 "max_hp":8,"armor":0,"condition":"ABLE"}
            ],
            "initiative_order":["one","two"],"round":1,
            "current_turn_index":0,"turn_action_consumed":false,"consumed_roll_ids":[],
            "status":"ONGOING","version":1,
            "last_transition":{"kind":"STARTED"}
        }"#;
        let unrelated = initial
            .replace("\"current_hp\":10", "\"current_hp\":4")
            .replace("\"version\":1", "\"version\":2")
            .replace(
                "{\"kind\":\"STARTED\"}",
                "{\"kind\":\"DAMAGE_APPLIED\",\"target_id\":\"one\",\"raw_damage\":1}",
            );
        validate_combat_state_transition(None, initial).unwrap();
        let persisted_injury = initial
            .replace("\"current_hp\":10", "\"current_hp\":5")
            .replacen("\"condition\":\"ABLE\"", "\"condition\":\"MAJOR_WOUND\"", 1);
        validate_combat_state_transition(None, &persisted_injury)
            .expect("an initial encounter state must preserve durable injuries");
        let inconsistent_health = initial.replace("\"current_hp\":10", "\"current_hp\":0");
        assert!(validate_combat_state_transition(None, &inconsistent_health).is_err());
        assert!(validate_combat_state_transition(Some(initial), &unrelated).is_err());
    }

    #[test]
    fn independent_replay_binds_damage_to_the_persisted_weapon_formula() {
        let initial = CombatSnapshot {
            combat_id: "combat_weapon_binding".to_owned(),
            participants: vec![
                Combatant {
                    participant_id: "attacker".to_owned(),
                    dexterity: 80,
                    skill_targets: CombatSkillTargets {
                        melee: 60,
                        firearm: 50,
                        dodge: 40,
                        first_aid: 30,
                        medicine: 10,
                    },
                    weapon_loadout: weapon_loadout(1, 5),
                    current_hp: 10,
                    max_hp: 10,
                    armor: 0,
                    condition: CombatCondition::Able,
                },
                Combatant {
                    participant_id: "defender".to_owned(),
                    dexterity: 50,
                    skill_targets: CombatSkillTargets {
                        melee: 45,
                        firearm: 35,
                        dodge: 25,
                        first_aid: 30,
                        medicine: 10,
                    },
                    weapon_loadout: weapon_loadout(0, 5),
                    current_hp: 8,
                    max_hp: 8,
                    armor: 0,
                    condition: CombatCondition::Able,
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
        let attack_roll = PercentileRollEvidence {
            roll_id: "weapon_attack".to_owned(),
            target: 60,
            roll: 40,
            selected_tens_digit: 4,
            ones_digit: 0,
            success_level: SuccessLevel::Regular,
        };
        let valid_mutation = CombatMutation::DamageApplied {
            attacker_id: "attacker".to_owned(),
            target_id: "defender".to_owned(),
            action: CombatActionKind::Melee,
            defense: CombatDefense::None,
            outcome: CombatExchangeOutcome::AttackerHit,
            attacker_roll: attack_roll.clone(),
            defender_roll: None,
            damage_roll: DamageRollEvidence {
                roll_id: "weapon_damage".to_owned(),
                dice_count: 1,
                die_sides: 6,
                flat_bonus: 1,
                dice_values: vec![1],
                raw_damage: 2,
            },
            raw_damage: 2,
        };
        let mut valid = initial.clone();
        apply_combat_mutation(&mut valid, &valid_mutation)
            .expect("the active fixture's persisted 1d6+1 weapon formula must replay");

        let mut forged_mutation = valid_mutation;
        let CombatMutation::DamageApplied {
            damage_roll,
            raw_damage,
            ..
        } = &mut forged_mutation
        else {
            unreachable!("the test constructed a damage mutation");
        };
        damage_roll.flat_bonus = 0;
        damage_roll.raw_damage = 1;
        *raw_damage = 1;
        let mut forged = initial;
        assert_eq!(
            apply_combat_mutation(&mut forged, &forged_mutation),
            Err(CanonicalGameplayStateError::InvalidTransition)
        );
    }

    #[test]
    fn serialized_replay_rejects_an_attack_from_an_incapacitated_actor() {
        let mut previous = CombatSnapshot {
            combat_id: "combat_incapacitated".to_owned(),
            participants: vec![
                Combatant {
                    participant_id: "attacker".to_owned(),
                    dexterity: 80,
                    skill_targets: CombatSkillTargets {
                        melee: 60,
                        firearm: 50,
                        dodge: 40,
                        first_aid: 30,
                        medicine: 10,
                    },
                    weapon_loadout: weapon_loadout(0, 5),
                    current_hp: 0,
                    max_hp: 5,
                    armor: 0,
                    condition: CombatCondition::Dead,
                },
                Combatant {
                    participant_id: "defender".to_owned(),
                    dexterity: 50,
                    skill_targets: CombatSkillTargets {
                        melee: 45,
                        firearm: 35,
                        dodge: 25,
                        first_aid: 30,
                        medicine: 10,
                    },
                    weapon_loadout: weapon_loadout(0, 5),
                    current_hp: 8,
                    max_hp: 8,
                    armor: 0,
                    condition: CombatCondition::Able,
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
        let previous_json = serde_json::to_string(&previous).unwrap();
        previous.participants[1].current_hp = 7;
        previous.version = 2;
        previous.last_transition = CombatMutation::DamageApplied {
            attacker_id: "attacker".to_owned(),
            target_id: "defender".to_owned(),
            action: CombatActionKind::Melee,
            defense: CombatDefense::None,
            outcome: CombatExchangeOutcome::AttackerHit,
            attacker_roll: PercentileRollEvidence {
                roll_id: "attack_roll".to_owned(),
                target: 60,
                roll: 40,
                selected_tens_digit: 4,
                ones_digit: 0,
                success_level: SuccessLevel::Regular,
            },
            defender_roll: None,
            damage_roll: DamageRollEvidence {
                roll_id: "damage_roll".to_owned(),
                dice_count: 1,
                die_sides: 6,
                flat_bonus: 0,
                dice_values: vec![1],
                raw_damage: 1,
            },
            raw_damage: 1,
        };
        let forged_successor = serde_json::to_string(&previous).unwrap();

        assert_eq!(
            validate_combat_state_transition(Some(&previous_json), &forged_successor),
            Err(CanonicalGameplayStateError::InvalidTransition)
        );
    }

    #[test]
    fn serialized_replay_accepts_only_a_genuine_no_damage_miss() {
        let mut next = CombatSnapshot {
            combat_id: "combat_miss".to_owned(),
            participants: vec![
                Combatant {
                    participant_id: "attacker".to_owned(),
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
                    participant_id: "defender".to_owned(),
                    dexterity: 50,
                    skill_targets: CombatSkillTargets {
                        melee: 45,
                        firearm: 35,
                        dodge: 25,
                        first_aid: 30,
                        medicine: 10,
                    },
                    weapon_loadout: weapon_loadout(0, 5),
                    current_hp: 8,
                    max_hp: 8,
                    armor: 0,
                    condition: CombatCondition::Able,
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
        let previous_json = serde_json::to_string(&next).unwrap();
        next.version = 2;
        next.turn_action_consumed = true;
        next.consumed_roll_ids.push("miss_roll".to_owned());
        next.last_transition = CombatMutation::AttackMissed {
            attacker_id: "attacker".to_owned(),
            target_id: "defender".to_owned(),
            action: CombatActionKind::Firearm,
            defense: CombatDefense::None,
            attacker_roll: PercentileRollEvidence {
                roll_id: "miss_roll".to_owned(),
                target: 50,
                roll: 80,
                selected_tens_digit: 8,
                ones_digit: 0,
                success_level: SuccessLevel::Failure,
            },
            defender_roll: None,
        };
        let missed_json = serde_json::to_string(&next).unwrap();
        validate_combat_state_transition(Some(&previous_json), &missed_json)
            .expect("a failed roll must replay as an explicit no-damage mutation");

        let CombatMutation::AttackMissed { attacker_roll, .. } = &mut next.last_transition else {
            unreachable!("the test just constructed an attack miss");
        };
        attacker_roll.roll = 40;
        attacker_roll.selected_tens_digit = 4;
        attacker_roll.success_level = SuccessLevel::Regular;
        let forged_miss = serde_json::to_string(&next).unwrap();
        assert_eq!(
            validate_combat_state_transition(Some(&previous_json), &forged_miss),
            Err(CanonicalGameplayStateError::InvalidTransition)
        );
    }
