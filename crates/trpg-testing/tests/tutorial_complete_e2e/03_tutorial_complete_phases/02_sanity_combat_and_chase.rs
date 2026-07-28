{
    repository
        .commit_sanity_execution(
            &metadata(
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "action_p08_sanity",
                "player_action",
                1,
                "p08_sanity_confirm",
                "private_to_player",
                PLAYER_ID,
                "human_keeper_statement",
            ),
            &SanityExecutionRecord {
                action_id: "action_p08_sanity".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                character_id: CHARACTER_ID.to_owned(),
                decision_id: "decision_p08_sanity".to_owned(),
                tool_execution_id: "tool_execution_p08_sanity".to_owned(),
                confirmed_by: KEEPER_ID.to_owned(),
                resolved_at_unix_ms: NOW_MS + 6_000,
                dice: server_dice_record(&sanity_roll),
                sanity_event_id: "sanity_event_p08".to_owned(),
                sheet_version_id: "sheet_p08_evelyn_v2".to_owned(),
                day_key: "tutorial_day_1".to_owned(),
                day_start_sanity: 65,
                sanity_before: 65,
                sanity_after: 65 - sanity_loss,
                sanity_loss,
                day_loss: sanity_loss,
                indefinite_threshold: 13,
                madness_state: "STABLE".to_owned(),
            },
        )
        .await
        .expect("commit SAN Decision, Dice, event and new Sheet atomically");

    let wrong_scene_combat = tutorial_combat_state(
        "combat_p08_wrong_scene",
        CombatHealth::new(10, 10, CombatCondition::Able).unwrap(),
    );
    assert!(matches!(
        repository
            .record_combat_state(
                &metadata(
                    AUTHORITY_ID,
                    KEEPER_ID,
                    "human_keeper",
                    "combat_p08_wrong_scene",
                    "combat_state",
                    0,
                    "p08_combat_wrong_scene",
                    "party_visible",
                    "not_applicable",
                    "rules_engine_decision",
                ),
                &RecordCombatStateRequest {
                    campaign_id: CAMPAIGN_ID.to_owned(),
                    session_id: SESSION_ID.to_owned(),
                    state_json: wrong_scene_combat.persistence_json().unwrap(),
                    attacker_roll: None,
                    defender_roll: None,
                    damage_roll: None,
                    medical_roll: None,
                },
            )
            .await,
        Err(CoreDomainRepositoryError::InvalidInput(
            "combat_participant_authority"
        ))
    ));

    repository
        .switch_scene(
            &metadata(
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                SESSION_ID,
                "session",
                1,
                "p08_scene_switch",
                "party_visible",
                "not_applicable",
                "human_keeper_statement",
            ),
            &SwitchSceneRequest {
                session_id: SESSION_ID.to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                next_scene_id: "scene_p08_basement".to_owned(),
                next_scene_key: "scene_basement".to_owned(),
                next_scene_name: "地下盐窖".to_owned(),
                switched_at_unix_ms: NOW_MS + 7_000,
            },
        )
        .await
        .expect("switch into the confrontation scene");

    let mut combat = tutorial_combat_state(
        "combat_p08_tutorial",
        CombatHealth::new(10, 10, CombatCondition::Able).unwrap(),
    );
    repository
        .record_combat_state(
            &metadata(
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "combat_p08_tutorial",
                "combat_state",
                0,
                "p08_combat_start",
                "party_visible",
                "not_applicable",
                "rules_engine_decision",
            ),
            &RecordCombatStateRequest {
                campaign_id: CAMPAIGN_ID.to_owned(),
                session_id: SESSION_ID.to_owned(),
                state_json: combat.persistence_json().unwrap(),
                attacker_roll: None,
                defender_roll: None,
                damage_roll: None,
                medical_roll: None,
            },
        )
        .await
        .expect("persist combat start");
    let combat_attack = percentile_with_result(80, true);
    let combat_damage = damage_with_value(1, 6, 5, 6);
    let damage = combat
        .apply_damage(
            CHARACTER_ID,
            CombatActionKind::Firearm,
            CombatDefense::None,
            &combat_attack,
            None,
            Some(&combat_damage),
        )
        .unwrap();
    assert_eq!(damage.condition, CombatCondition::MajorWound);
    repository
        .record_combat_state(
            &metadata(
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "combat_p08_tutorial",
                "combat_state",
                1,
                "p08_combat_damage",
                "party_visible",
                "not_applicable",
                "rules_engine_decision",
            ),
            &RecordCombatStateRequest {
                campaign_id: CAMPAIGN_ID.to_owned(),
                session_id: SESSION_ID.to_owned(),
                state_json: combat.persistence_json().unwrap(),
                attacker_roll: Some(combat_attack),
                defender_roll: None,
                damage_roll: Some(combat_damage),
                medical_roll: None,
            },
        )
        .await
        .expect("persist combat damage transition");
    combat.end().unwrap();
    assert_eq!(combat.status(), CombatStatus::Ended);
    let combat_end_metadata = metadata(
        AUTHORITY_ID,
        KEEPER_ID,
        "human_keeper",
        "combat_p08_tutorial",
        "combat_state",
        2,
        "p08_combat_end",
        "party_visible",
        "not_applicable",
        "rules_engine_decision",
    );
    let combat_end_request = RecordCombatStateRequest {
        campaign_id: CAMPAIGN_ID.to_owned(),
        session_id: SESSION_ID.to_owned(),
        state_json: combat.persistence_json().unwrap(),
        attacker_roll: None,
        defender_roll: None,
        damage_roll: None,
        medical_roll: None,
    };
    let combat_end_receipt = repository
        .record_combat_state(&combat_end_metadata, &combat_end_request)
        .await
        .expect("persist terminal combat state");
    assert_eq!(
        repository
            .record_combat_state(&combat_end_metadata, &combat_end_request)
            .await
            .expect("return the persisted combat receipt on exact retry"),
        combat_end_receipt
    );

    let forged_chase = ChaseState::start(
        "chase_p08_forged_start",
        vec![
            ChaseParticipant::new(CHARACTER_ID, ChaseRole::Pursuer, 20).unwrap(),
            ChaseParticipant::new("npc_marta", ChaseRole::Quarry, 1).unwrap(),
        ],
        4,
    )
    .expect("construct an internally valid but unauthorized chase");
    assert!(matches!(
        repository
            .record_chase_state(
                &metadata(
                    AUTHORITY_ID,
                    KEEPER_ID,
                    "human_keeper",
                    "chase_p08_forged_start",
                    "chase_state",
                    0,
                    "p08_chase_forged_start",
                    "party_visible",
                    "not_applicable",
                    "rules_engine_decision",
                ),
                &RecordChaseStateRequest {
                    campaign_id: CAMPAIGN_ID.to_owned(),
                    session_id: SESSION_ID.to_owned(),
                    state_json: forged_chase.persistence_json().unwrap(),
                    participant_rolls: Vec::new(),
                },
            )
            .await,
        Err(CoreDomainRepositoryError::InvalidInput(
            "chase_participant_authority"
        ))
    ));

    let mut chase = ChaseState::start(
        "chase_p08_tutorial",
        vec![
            ChaseParticipant::new(CHARACTER_ID, ChaseRole::Quarry, 8).unwrap(),
            ChaseParticipant::new("npc_marta", ChaseRole::Pursuer, 8).unwrap(),
        ],
        2,
    )
    .expect("start rules-engine chase aggregate");
    repository
        .record_chase_state(
            &metadata(
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "chase_p08_tutorial",
                "chase_state",
                0,
                "p08_chase_start",
                "party_visible",
                "not_applicable",
                "rules_engine_decision",
            ),
            &RecordChaseStateRequest {
                campaign_id: CAMPAIGN_ID.to_owned(),
                session_id: SESSION_ID.to_owned(),
                state_json: chase.persistence_json().unwrap(),
                participant_rolls: Vec::new(),
            },
        )
        .await
        .expect("persist chase start");
    let chase_rolls = vec![
        percentile_with_result(40, false),
        percentile_with_result(40, true),
    ];
    let chase_obstacle = ChaseObstacle::new("obstacle_collapsing_salt", 1).unwrap();
    chase.advance(&chase_rolls, Some(&chase_obstacle)).unwrap();
    assert_eq!(chase.status(), ChaseStatus::Caught);
    let chase_end_metadata = metadata(
        AUTHORITY_ID,
        KEEPER_ID,
        "human_keeper",
        "chase_p08_tutorial",
        "chase_state",
        1,
        "p08_chase_caught",
        "party_visible",
        "not_applicable",
        "rules_engine_decision",
    );
    let chase_end_request = RecordChaseStateRequest {
        campaign_id: CAMPAIGN_ID.to_owned(),
        session_id: SESSION_ID.to_owned(),
        state_json: chase.persistence_json().unwrap(),
        participant_rolls: chase_rolls,
    };
    let chase_end_receipt = repository
        .record_chase_state(&chase_end_metadata, &chase_end_request)
        .await
        .expect("persist terminal chase state");
    assert_eq!(
        repository
            .record_chase_state(&chase_end_metadata, &chase_end_request)
            .await
            .expect("return the persisted chase receipt on exact retry"),
        chase_end_receipt
    );
    assert!(
        chase
            .advance(
                &[
                    percentile_with_result(40, true),
                    percentile_with_result(40, false),
                ],
                None,
            )
            .is_err(),
        "a terminal chase cannot resume under the same ID"
    );
    include!("03_session_end_and_growth.rs");
}
