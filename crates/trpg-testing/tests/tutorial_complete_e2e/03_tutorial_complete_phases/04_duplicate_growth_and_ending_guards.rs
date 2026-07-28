{
    assert!(matches!(
        repository
            .change_session_state(
                &later_session_end_metadata,
                CAMPAIGN_ID,
                "session_p08_later",
                SessionState::Ended,
                NOW_MS + 11_000,
            )
            .await,
        Err(CoreDomainRepositoryError::InvalidInput(
            "session_gameplay_not_terminal"
        ))
    ));
    unfinished_later_combat.end().unwrap();
    repository
        .record_combat_state(
            &metadata(
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "combat_p08_unfinished_later",
                "combat_state",
                1,
                "p08_finish_later_combat",
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
        .expect("finish the later Combat before ending its Session");
    let later_chase_rolls = vec![
        percentile_with_result(40, false),
        percentile_with_result(40, true),
    ];
    let later_chase_obstacle = ChaseObstacle::new("obstacle_later_collapse", 1).unwrap();
    unfinished_later_chase
        .advance(&later_chase_rolls, Some(&later_chase_obstacle))
        .unwrap();
    assert_eq!(unfinished_later_chase.status(), ChaseStatus::Caught);
    repository
        .record_chase_state(
            &metadata(
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "chase_p08_unfinished_later",
                "chase_state",
                1,
                "p08_finish_later_chase",
                "party_visible",
                "not_applicable",
                "rules_engine_decision",
            ),
            &RecordChaseStateRequest {
                campaign_id: CAMPAIGN_ID.to_owned(),
                session_id: "session_p08_later".to_owned(),
                state_json: unfinished_later_chase.persistence_json().unwrap(),
                participant_rolls: later_chase_rolls,
            },
        )
        .await
        .expect("finish the later Chase before ending its Session");
    repository
        .change_session_state(
            &later_session_end_metadata,
            CAMPAIGN_ID,
            "session_p08_later",
            SessionState::Ended,
            NOW_MS + 11_000,
        )
        .await
        .expect("end the interleaved later Session after gameplay is terminal");
    let invalid_ending = repository
        .record_ending(
            &metadata(
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "ending_event_p08_invalid",
                "ending",
                0,
                "p08_ending_invalid",
                "party_visible",
                "not_applicable",
                "human_keeper_statement",
            ),
            &RecordEndingRequest {
                ending_event_id: "ending_event_p08_invalid".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                session_id: SESSION_ID.to_owned(),
                ending_id: "ending_not_in_scenario".to_owned(),
                summary: "This ending is not defined by the scenario.".to_owned(),
                ended_at_unix_ms: NOW_MS + 9_000,
            },
        )
        .await;
    assert!(matches!(
        invalid_ending,
        Err(CoreDomainRepositoryError::InvalidInput(
            "ending_id_not_defined"
        ))
    ));
    let ending_metadata = metadata(
        AUTHORITY_ID,
        KEEPER_ID,
        "human_keeper",
        "ending_event_p08_tutorial",
        "ending",
        0,
        "p08_ending",
        "party_visible",
        "not_applicable",
        "human_keeper_statement",
    );
    let ending_request = RecordEndingRequest {
        ending_event_id: "ending_event_p08_tutorial".to_owned(),
        campaign_id: CAMPAIGN_ID.to_owned(),
        session_id: SESSION_ID.to_owned(),
        ending_id: "ending_expose_marta".to_owned(),
        summary: "The investigators expose Marta and preserve the archive.".to_owned(),
        ended_at_unix_ms: NOW_MS + 9_000,
    };
    let ending_receipt = repository
        .record_ending(&ending_metadata, &ending_request)
        .await
        .expect("record an allowed Tutorial ending");
    assert_eq!(
        repository
            .record_ending(&ending_metadata, &ending_request)
            .await
            .expect("return the persisted ending receipt on exact retry"),
        ending_receipt
    );
    let ending_events_before_duplicate: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM public.event_store WHERE event_type = 'EndingRecorded'",
    )
    .fetch_one(&primary)
    .await
    .unwrap();
    let duplicate_session_ending = repository
        .record_ending(
            &metadata(
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "ending_event_p08_duplicate",
                "ending",
                0,
                "p08_ending_duplicate",
                "party_visible",
                "not_applicable",
                "human_keeper_statement",
            ),
            &RecordEndingRequest {
                ending_event_id: "ending_event_p08_duplicate".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                session_id: SESSION_ID.to_owned(),
                ending_id: "ending_expose_marta".to_owned(),
                summary: "A conflicting second canonical ending.".to_owned(),
                ended_at_unix_ms: NOW_MS + 9_001,
            },
        )
        .await;
    assert!(matches!(
        duplicate_session_ending,
        Err(CoreDomainRepositoryError::Integrity(
            "ending_session_already_recorded"
        ))
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM public.event_store WHERE event_type = 'EndingRecorded'",
        )
        .fetch_one(&primary)
        .await
        .unwrap(),
        ending_events_before_duplicate,
        "a semantic duplicate ending must be rejected before canonical append"
    );
    let growth_events_before_unawarded: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM public.event_store WHERE event_type = 'CharacterGrowthApplied'",
    )
    .fetch_one(&primary)
    .await
    .unwrap();
    let combat_health_sheet_id: String = sqlx::query_scalar(
        r#"
        SELECT sheet.sheet_version_id
          FROM public.characters AS character
          JOIN public.character_sheet_versions AS sheet
            ON sheet.character_id = character.character_id
           AND sheet.version = character.current_sheet_version
         WHERE character.character_id = $1
           AND sheet.sheet_json #>> '{combat_profile,current_hp}' = '5'
           AND sheet.sheet_json #>> '{combat_profile,condition}' =
               'MAJOR_WOUND'
           AND character.visibility_label::TEXT = 'private_to_player'
           AND sheet.visibility_label::TEXT = 'private_to_player'
        "#,
    )
    .bind(CHARACTER_ID)
    .fetch_one(&primary)
    .await
    .expect("Combat damage must advance the private Character sheet");
    let unawarded_roll =
        server_roll_skill_growth(50).expect("server-owned unawarded growth evidence");
    let unawarded_growth = repository
        .record_growth(
            &metadata(
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "growth_event_p08_unawarded",
                "growth",
                0,
                "p08_growth_unawarded",
                "private_to_player",
                PLAYER_ID,
                "rules_engine_decision",
            ),
            &RecordGrowthRequest {
                growth_event_id: "growth_event_p08_unawarded".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                session_id: SESSION_ID.to_owned(),
                ending_event_id: "ending_event_p08_tutorial".to_owned(),
                character_id: CHARACTER_ID.to_owned(),
                source_sheet_version_id: combat_health_sheet_id.clone(),
                new_sheet_version_id: "sheet_p08_evelyn_v3_unawarded".to_owned(),
                skill_name: "Dodge".to_owned(),
                growth_rolls: unawarded_roll.evidence().clone(),
            },
        )
        .await;
    assert!(matches!(
        unawarded_growth,
        Err(CoreDomainRepositoryError::InvalidInput(
            "growth_skill_not_awarded"
        ))
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM public.event_store \
             WHERE event_type = 'CharacterGrowthApplied'",
        )
        .fetch_one(&primary)
        .await
        .unwrap(),
        growth_events_before_unawarded,
        "an unawarded skill must be rejected before canonical append"
    );
    let growth_roll = server_roll_skill_growth(70).expect("server-owned COC7 growth rolls");
    let growth_after = growth_roll.outcome().skill_after;
    let growth_metadata = metadata(
        AUTHORITY_ID,
        KEEPER_ID,
        "human_keeper",
        "growth_event_p08_tutorial",
        "growth",
        0,
        "p08_growth",
        "private_to_player",
        PLAYER_ID,
        "rules_engine_decision",
    );
    let growth_request = RecordGrowthRequest {
        growth_event_id: "growth_event_p08_tutorial".to_owned(),
        campaign_id: CAMPAIGN_ID.to_owned(),
        session_id: SESSION_ID.to_owned(),
        ending_event_id: "ending_event_p08_tutorial".to_owned(),
        character_id: CHARACTER_ID.to_owned(),
        source_sheet_version_id: combat_health_sheet_id,
        new_sheet_version_id: "sheet_p08_evelyn_v3".to_owned(),
        skill_name: "Library Use".to_owned(),
        growth_rolls: growth_roll.evidence().clone(),
    };
    let growth_receipt = repository
        .record_growth(&growth_metadata, &growth_request)
        .await
        .expect("apply server-generated growth to a new locked Sheet version");
    assert_eq!(
        repository
            .record_growth(&growth_metadata, &growth_request)
            .await
            .expect("return the persisted growth receipt on exact retry"),
        growth_receipt
    );
    let growth_events_before_duplicate: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM public.event_store WHERE event_type = 'CharacterGrowthApplied'",
    )
    .fetch_one(&primary)
    .await
    .unwrap();
    let duplicate_growth_roll =
        server_roll_skill_growth(growth_after).expect("server-owned duplicate growth evidence");
    let duplicate_skill_growth = repository
        .record_growth(
            &metadata(
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "growth_event_p08_duplicate",
                "growth",
                0,
                "p08_growth_duplicate",
                "private_to_player",
                PLAYER_ID,
                "rules_engine_decision",
            ),
            &RecordGrowthRequest {
                growth_event_id: "growth_event_p08_duplicate".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                session_id: SESSION_ID.to_owned(),
                ending_event_id: "ending_event_p08_tutorial".to_owned(),
                character_id: CHARACTER_ID.to_owned(),
                source_sheet_version_id: "sheet_p08_evelyn_v3".to_owned(),
                new_sheet_version_id: "sheet_p08_evelyn_v4_duplicate".to_owned(),
                skill_name: "Library Use".to_owned(),
                growth_rolls: duplicate_growth_roll.evidence().clone(),
            },
        )
        .await;
    assert!(matches!(
        duplicate_skill_growth,
        Err(CoreDomainRepositoryError::Integrity(
            "growth_skill_already_recorded"
        ))
    ));
    include!("05_concurrent_ending_and_fork.rs");
}
