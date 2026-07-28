{

    repository
        .change_session_state(
            &metadata(
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                SESSION_ID,
                "session",
                2,
                "p08_session_end",
                "party_visible",
                "not_applicable",
                "human_keeper_statement",
            ),
            CAMPAIGN_ID,
            SESSION_ID,
            SessionState::Ended,
            NOW_MS + 8_000,
        )
        .await
        .expect("end the Tutorial Session");
    let gameplay_events_before_ended_session_write: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM public.event_store \
         WHERE event_type IN ('CombatStateRecorded', 'ChaseStateRecorded')",
    )
    .fetch_one(&primary)
    .await
    .unwrap();
    assert_eq!(
        repository
            .record_combat_state(&combat_end_metadata, &combat_end_request)
            .await
            .expect("return the combat receipt when exact retry happens after session end"),
        combat_end_receipt
    );
    assert_eq!(
        repository
            .record_chase_state(&chase_end_metadata, &chase_end_request)
            .await
            .expect("return the chase receipt when exact retry happens after session end"),
        chase_end_receipt
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM public.event_store \
             WHERE event_type IN ('CombatStateRecorded', 'ChaseStateRecorded')",
        )
        .fetch_one(&primary)
        .await
        .unwrap(),
        gameplay_events_before_ended_session_write,
        "exact retries after session end must return prior receipts without appending"
    );
    let blocked_combat = CombatState::start(
        "combat_p08_after_ending",
        vec![
            CombatantState::new(
                CHARACTER_ID,
                70,
                CombatHealth::new(10, 10, CombatCondition::Able).unwrap(),
                1,
                CombatSkillTargets::new(45, 35, 40, 30, 10).unwrap(),
                weapon_loadout(1, 5),
            )
            .unwrap(),
            CombatantState::new(
                "npc_marta",
                80,
                CombatHealth::new(8, 8, CombatCondition::Able).unwrap(),
                0,
                CombatSkillTargets::new(60, 80, 40, 30, 10).unwrap(),
                weapon_loadout(0, 5),
            )
            .unwrap(),
        ],
    )
    .unwrap();
    assert!(matches!(
        repository
            .record_combat_state(
                &metadata(
                    AUTHORITY_ID,
                    KEEPER_ID,
                    "human_keeper",
                    "combat_p08_after_ending",
                    "combat_state",
                    0,
                    "p08_combat_after_ending",
                    "party_visible",
                    "not_applicable",
                    "rules_engine_decision",
                ),
                &RecordCombatStateRequest {
                    campaign_id: CAMPAIGN_ID.to_owned(),
                    session_id: SESSION_ID.to_owned(),
                    state_json: blocked_combat.persistence_json().unwrap(),
                    attacker_roll: None,
                    defender_roll: None,
                    damage_roll: None,
                    medical_roll: None,
                },
            )
            .await,
        Err(CoreDomainRepositoryError::InvalidInput(
            "gameplay_session_state"
        ))
    ));
    let blocked_chase = ChaseState::start(
        "chase_p08_after_ending",
        vec![
            ChaseParticipant::new(CHARACTER_ID, ChaseRole::Quarry, 8).unwrap(),
            ChaseParticipant::new("npc_marta", ChaseRole::Pursuer, 8).unwrap(),
        ],
        2,
    )
    .unwrap();
    assert!(matches!(
        repository
            .record_chase_state(
                &metadata(
                    AUTHORITY_ID,
                    KEEPER_ID,
                    "human_keeper",
                    "chase_p08_after_ending",
                    "chase_state",
                    0,
                    "p08_chase_after_ending",
                    "party_visible",
                    "not_applicable",
                    "rules_engine_decision",
                ),
                &RecordChaseStateRequest {
                    campaign_id: CAMPAIGN_ID.to_owned(),
                    session_id: SESSION_ID.to_owned(),
                    state_json: blocked_chase.persistence_json().unwrap(),
                    participant_rolls: Vec::new(),
                },
            )
            .await,
        Err(CoreDomainRepositoryError::InvalidInput(
            "gameplay_session_state"
        ))
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM public.event_store \
             WHERE event_type IN ('CombatStateRecorded', 'ChaseStateRecorded')",
        )
        .fetch_one(&primary)
        .await
        .unwrap(),
        gameplay_events_before_ended_session_write,
        "an ended session must reject combat and chase before canonical append"
    );
    repository
        .start_session(
            &metadata(
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "session_p08_later",
                "session",
                0,
                "p08_later_session_start",
                "party_visible",
                "not_applicable",
                "human_keeper_statement",
            ),
            &StartSessionRequest {
                session_id: "session_p08_later".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                room_id: "room_p08_tutorial".to_owned(),
                scenario_id: "scenario_p08_tutorial".to_owned(),
                scene_id: "scene_p08_later".to_owned(),
                scene_key: "scene_basement".to_owned(),
                scene_name: "地下盐窖重访".to_owned(),
                started_at_unix_ms: NOW_MS + 10_000,
            },
        )
        .await
        .expect("start an interleaved later session before the source conclusion");
    let healed_later_combat = CombatState::start(
        "combat_p08_healed_later",
        vec![
            CombatantState::new(
                CHARACTER_ID,
                70,
                CombatHealth::new(10, 10, CombatCondition::Able).unwrap(),
                1,
                CombatSkillTargets::new(45, 35, 40, 30, 10).unwrap(),
                weapon_loadout(1, 5),
            )
            .unwrap(),
            CombatantState::new(
                "npc_marta",
                80,
                CombatHealth::new(8, 8, CombatCondition::Able).unwrap(),
                0,
                CombatSkillTargets::new(60, 80, 40, 30, 10).unwrap(),
                weapon_loadout(0, 5),
            )
            .unwrap(),
        ],
    )
    .unwrap();
    assert!(matches!(
        repository
            .record_combat_state(
                &metadata(
                    AUTHORITY_ID,
                    KEEPER_ID,
                    "human_keeper",
                    "combat_p08_healed_later",
                    "combat_state",
                    0,
                    "p08_healed_later_combat",
                    "party_visible",
                    "not_applicable",
                    "rules_engine_decision",
                ),
                &RecordCombatStateRequest {
                    campaign_id: CAMPAIGN_ID.to_owned(),
                    session_id: "session_p08_later".to_owned(),
                    state_json: healed_later_combat.persistence_json().unwrap(),
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
    let mut unfinished_later_combat = CombatState::start(
        "combat_p08_unfinished_later",
        vec![
            CombatantState::new(
                CHARACTER_ID,
                70,
                CombatHealth::new(5, 10, CombatCondition::MajorWound).unwrap(),
                1,
                CombatSkillTargets::new(45, 35, 40, 30, 10).unwrap(),
                weapon_loadout(1, 5),
            )
            .unwrap(),
            CombatantState::new(
                "npc_marta",
                80,
                CombatHealth::new(8, 8, CombatCondition::Able).unwrap(),
                0,
                CombatSkillTargets::new(60, 80, 40, 30, 10).unwrap(),
                weapon_loadout(0, 5),
            )
            .unwrap(),
        ],
    )
    .unwrap();
    repository
        .record_combat_state(
            &metadata(
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "combat_p08_unfinished_later",
                "combat_state",
                0,
                "p08_unfinished_later_combat",
                "party_visible",
                "not_applicable",
                "rules_engine_decision",
            ),
            &RecordCombatStateRequest {
                campaign_id: CAMPAIGN_ID.to_owned(),
                session_id: "session_p08_later".to_owned(),
                state_json: unfinished_later_combat.persistence_json().unwrap(),
                attacker_roll: None,
                defender_roll: None,
                damage_roll: None,
                medical_roll: None,
            },
        )
        .await
        .expect("persist an unfinished combat before ending the later session");
    let mut unfinished_later_chase = ChaseState::start(
        "chase_p08_unfinished_later",
        vec![
            ChaseParticipant::new(CHARACTER_ID, ChaseRole::Quarry, 8).unwrap(),
            ChaseParticipant::new("npc_marta", ChaseRole::Pursuer, 8).unwrap(),
        ],
        2,
    )
    .unwrap();
    repository
        .record_chase_state(
            &metadata(
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "chase_p08_unfinished_later",
                "chase_state",
                0,
                "p08_unfinished_later_chase",
                "party_visible",
                "not_applicable",
                "rules_engine_decision",
            ),
            &RecordChaseStateRequest {
                campaign_id: CAMPAIGN_ID.to_owned(),
                session_id: "session_p08_later".to_owned(),
                state_json: unfinished_later_chase.persistence_json().unwrap(),
                participant_rolls: Vec::new(),
            },
        )
        .await
        .expect("persist an unfinished chase before ending the later session");
    let later_session_end_metadata = metadata(
        AUTHORITY_ID,
        KEEPER_ID,
        "human_keeper",
        "session_p08_later",
        "session",
        1,
        "p08_later_session_end",
        "party_visible",
        "not_applicable",
        "human_keeper_statement",
    );
    include!("04_duplicate_growth_and_ending_guards.rs");
}
