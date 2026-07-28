{
    repository
        .record_combat_state(
            &metadata(
                CAMPAIGN_ID,
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "combat_p08_schema",
                "combat_state",
                "combat.state.damage",
                10,
                "combat_p08_fight_back",
                "party_visible",
                "not_applicable",
                "rules_engine_decision",
            ),
            &RecordCombatStateRequest {
                campaign_id: CAMPAIGN_ID.to_owned(),
                session_id: "session_p06_schema".to_owned(),
                state_json: fight_back_state,
                attacker_roll: Some(fight_back_attack),
                defender_roll: Some(fight_back_defense),
                damage_roll: Some(fight_back_damage),
                medical_roll: None,
            },
        )
        .await
        .expect("persist a verified fight-back counterattack");
    assert_eq!(
        persist_combat_turn_advance(
            &repository,
            &mut combat,
            11,
            "combat_p08_after_fight_back_to_player",
        )
        .await,
        "character_p06_player"
    );
    let growth_roll_reused_by_combat = loop {
        let roll = server_roll_skill_growth(70).unwrap();
        if matches!(
            success_level(roll.evidence().improvement_check().value(), 30).unwrap(),
            SuccessLevel::Critical
                | SuccessLevel::Extreme
                | SuccessLevel::Hard
                | SuccessLevel::Regular
        ) {
            break roll;
        }
    };
    let medical_roll = growth_roll_reused_by_combat
        .evidence()
        .improvement_check()
        .clone();
    assert_eq!(
        combat
            .recover_major_wound(
                "character_p06_player",
                "character_p06_player",
                CombatMedicalSkill::FirstAid,
                &medical_roll,
            )
            .unwrap(),
        CombatCondition::Able
    );
    repository
        .record_combat_state(
            &metadata(
                CAMPAIGN_ID,
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "combat_p08_schema",
                "combat_state",
                "combat.state.medical",
                12,
                "combat_p08_major_wound_recovery",
                "party_visible",
                "not_applicable",
                "rules_engine_decision",
            ),
            &RecordCombatStateRequest {
                campaign_id: CAMPAIGN_ID.to_owned(),
                session_id: "session_p06_schema".to_owned(),
                state_json: combat.persistence_json().unwrap(),
                attacker_roll: None,
                defender_roll: None,
                damage_roll: None,
                medical_roll: Some(medical_roll.clone()),
            },
        )
        .await
        .expect("persist medical recovery using the current healer's First Aid target");
    combat.end().unwrap();
    assert_eq!(combat.status(), CombatStatus::Ended);
    let terminal_combat_metadata = metadata(
        CAMPAIGN_ID,
        AUTHORITY_ID,
        KEEPER_ID,
        "human_keeper",
        "combat_p08_schema",
        "combat_state",
        "combat.state.end",
        13,
        "combat_p08_end",
        "party_visible",
        "not_applicable",
        "rules_engine_decision",
    );
    let terminal_combat_request = RecordCombatStateRequest {
        campaign_id: CAMPAIGN_ID.to_owned(),
        session_id: "session_p06_schema".to_owned(),
        state_json: combat.persistence_json().unwrap(),
        attacker_roll: None,
        defender_roll: None,
        damage_roll: None,
        medical_roll: None,
    };
    sqlx::raw_sql(
        r#"
        CREATE FUNCTION public.reject_terminal_combat_projection_for_test()
        RETURNS trigger
        LANGUAGE plpgsql
        AS $$
        BEGIN
            IF NEW.combat_id = 'combat_p08_schema'
               AND NEW.version = 14 THEN
                RAISE EXCEPTION
                    'injected terminal Combat projection failure';
            END IF;
            RETURN NEW;
        END;
        $$;
        CREATE TRIGGER zz_reject_terminal_combat_projection_for_test
        BEFORE INSERT OR UPDATE ON public.combat_states
        FOR EACH ROW EXECUTE FUNCTION
            public.reject_terminal_combat_projection_for_test();
        "#,
    )
    .execute(&primary)
    .await
    .expect("install terminal Combat projection failure injection");
    assert!(matches!(
        repository
            .record_combat_state(&terminal_combat_metadata, &terminal_combat_request,)
            .await,
        Err(CoreDomainRepositoryError::Database("project_combat_state"))
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT version FROM public.combat_states \
             WHERE combat_id = 'combat_p08_schema'",
        )
        .fetch_one(&primary)
        .await
        .unwrap(),
        13,
        "the terminal Combat event must be canonical while its projection remains ongoing"
    );

    let mut chase = ChaseState::start(
        "chase_p08_schema",
        vec![
            ChaseParticipant::new("character_p06_player", ChaseRole::Quarry, 8).unwrap(),
            ChaseParticipant::new("npc_marta", ChaseRole::Pursuer, 8).unwrap(),
        ],
        2,
    )
    .unwrap();
    repository
        .record_chase_state(
            &metadata(
                CAMPAIGN_ID,
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "chase_p08_schema",
                "chase_state",
                "chase.state.start",
                0,
                "chase_p08_start",
                "party_visible",
                "not_applicable",
                "rules_engine_decision",
            ),
            &RecordChaseStateRequest {
                campaign_id: CAMPAIGN_ID.to_owned(),
                session_id: "session_p06_schema".to_owned(),
                state_json: chase.persistence_json().unwrap(),
                participant_rolls: Vec::new(),
            },
        )
        .await
        .expect("persist started chase aggregate");
    let chase_events_before_forgery: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM public.event_store \
         WHERE campaign_id = $1 AND stream_id = 'chase_p08_schema'",
    )
    .bind(CAMPAIGN_ID)
    .fetch_one(&primary)
    .await
    .unwrap();
    let mut reused_roll_chase = chase.clone();
    let reused_roll_chase_evidence = vec![missed_attack.clone(), percentile_with_result(40, true)];
    reused_roll_chase
        .advance(&reused_roll_chase_evidence, None)
        .unwrap();
    assert!(matches!(
        repository
            .record_chase_state(
                &metadata(
                    CAMPAIGN_ID,
                    AUTHORITY_ID,
                    KEEPER_ID,
                    "human_keeper",
                    "chase_p08_schema",
                    "chase_state",
                    "chase.state.advance",
                    1,
                    "chase_p08_cross_aggregate_roll_reuse",
                    "party_visible",
                    "not_applicable",
                    "rules_engine_decision",
                ),
                &RecordChaseStateRequest {
                    campaign_id: CAMPAIGN_ID.to_owned(),
                    session_id: "session_p06_schema".to_owned(),
                    state_json: reused_roll_chase.persistence_json().unwrap(),
                    participant_rolls: reused_roll_chase_evidence,
                },
            )
            .await,
        Err(CoreDomainRepositoryError::InvalidInput(
            "gameplay_roll_reuse"
        ))
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM public.event_store \
             WHERE campaign_id = $1 AND stream_id = 'chase_p08_schema'",
        )
        .bind(CAMPAIGN_ID)
        .fetch_one(&primary)
        .await
        .unwrap(),
        chase_events_before_forgery,
        "a Combat roll reused by Chase must fail before canonical append"
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT aggregate_kind FROM public.gameplay_roll_consumptions \
             WHERE roll_id = $1",
        )
        .bind(missed_attack.roll_id())
        .fetch_one(&primary)
        .await
        .unwrap(),
        "COMBAT",
        "a roll reserved across a failed projection must remain bound to its first aggregate"
    );
    let mut mismatched_chase = chase.clone();
    let recorded_chase_rolls = vec![
        percentile_with_result(40, false),
        percentile_with_result(40, true),
    ];
    mismatched_chase
        .advance(&recorded_chase_rolls, None)
        .unwrap();
    assert!(matches!(
        repository
            .record_chase_state(
                &metadata(
                    CAMPAIGN_ID,
                    AUTHORITY_ID,
                    KEEPER_ID,
                    "human_keeper",
                    "chase_p08_schema",
                    "chase_state",
                    "chase.state.advance",
                    1,
                    "chase_p08_mismatched_roll",
                    "party_visible",
                    "not_applicable",
                    "rules_engine_decision",
                ),
                &RecordChaseStateRequest {
                    campaign_id: CAMPAIGN_ID.to_owned(),
                    session_id: "session_p06_schema".to_owned(),
                    state_json: mismatched_chase.persistence_json().unwrap(),
                    participant_rolls: vec![
                        percentile_with_result(40, false),
                        percentile_with_result(40, true),
                    ],
                },
            )
            .await,
        Err(CoreDomainRepositoryError::InvalidInput(
            "chase_roll_evidence"
        ))
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM public.event_store \
             WHERE campaign_id = $1 AND stream_id = 'chase_p08_schema'",
        )
        .bind(CAMPAIGN_ID)
        .fetch_one(&primary)
        .await
        .unwrap(),
        chase_events_before_forgery,
        "mismatched chase roll evidence must fail before canonical append"
    );
    let chase_rolls = vec![
        percentile_with_result(40, false),
        percentile_with_result(40, true),
    ];
    let chase_obstacle = ChaseObstacle::new("obstacle_collapsing_salt", 1).unwrap();
    chase.advance(&chase_rolls, Some(&chase_obstacle)).unwrap();
    assert_eq!(chase.status(), ChaseStatus::Caught);
    let terminal_chase_metadata = metadata(
        CAMPAIGN_ID,
        AUTHORITY_ID,
        KEEPER_ID,
        "human_keeper",
        "chase_p08_schema",
        "chase_state",
        "chase.state.advance",
        1,
        "chase_p08_caught",
        "party_visible",
        "not_applicable",
        "rules_engine_decision",
    );
    let terminal_chase_request = RecordChaseStateRequest {
        campaign_id: CAMPAIGN_ID.to_owned(),
        session_id: "session_p06_schema".to_owned(),
        state_json: chase.persistence_json().unwrap(),
        participant_rolls: chase_rolls.clone(),
    };
    include!("08_chase_projection_failure.rs");
}
