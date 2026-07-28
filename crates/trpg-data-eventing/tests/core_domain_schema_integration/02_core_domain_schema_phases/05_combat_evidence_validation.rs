{

    Box::pin(async {
        let forged_initial_combat = CombatState::start(
            "combat_p08_forged_initial",
            vec![
                CombatantState::new(
                    "character_p06_player",
                    99,
                    CombatHealth::new(30, 30, CombatCondition::Able).unwrap(),
                    20,
                    CombatSkillTargets::new(99, 99, 99, 99, 99).unwrap(),
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
                        CAMPAIGN_ID,
                        AUTHORITY_ID,
                        KEEPER_ID,
                        "human_keeper",
                        "combat_p08_forged_initial",
                        "combat_state",
                        "combat.state.start",
                        0,
                        "combat_p08_forged_initial",
                        "party_visible",
                        "not_applicable",
                        "rules_engine_decision",
                    ),
                    &RecordCombatStateRequest {
                        campaign_id: CAMPAIGN_ID.to_owned(),
                        session_id: "session_p06_schema".to_owned(),
                        state_json: forged_initial_combat.persistence_json().unwrap(),
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
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM public.event_store \
             WHERE campaign_id = $1 AND stream_id = 'combat_p08_forged_initial'",
            )
            .bind(CAMPAIGN_ID)
            .fetch_one(&primary)
            .await
            .unwrap(),
            0,
            "invented HP, skills, and armor must fail before canonical append"
        );
    })
    .await;

    let mut combat = CombatState::start(
        "combat_p08_schema",
        vec![
            CombatantState::new(
                "character_p06_player",
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
    repository
        .record_combat_state(
            &metadata(
                CAMPAIGN_ID,
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "combat_p08_schema",
                "combat_state",
                "combat.state.start",
                0,
                "combat_p08_start",
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
                medical_roll: None,
            },
        )
        .await
        .expect("persist started combat aggregate");
    let combat_events_before_forgery: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM public.event_store \
         WHERE campaign_id = $1 AND stream_id = 'combat_p08_schema'",
    )
    .bind(CAMPAIGN_ID)
    .fetch_one(&primary)
    .await
    .unwrap();
    let mut mismatched_evidence_state = combat.clone();
    let recorded_attack = percentile_with_result(80, true);
    let recorded_damage = damage_with_value(1, 6, 5, 6);
    mismatched_evidence_state
        .apply_damage(
            "character_p06_player",
            CombatActionKind::Firearm,
            CombatDefense::None,
            &recorded_attack,
            None,
            Some(&recorded_damage),
        )
        .unwrap();
    assert!(matches!(
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
                    1,
                    "combat_p08_mismatched_roll",
                    "party_visible",
                    "not_applicable",
                    "rules_engine_decision",
                ),
                &RecordCombatStateRequest {
                    campaign_id: CAMPAIGN_ID.to_owned(),
                    session_id: "session_p06_schema".to_owned(),
                    state_json: mismatched_evidence_state.persistence_json().unwrap(),
                    attacker_roll: Some(percentile_with_result(80, true)),
                    defender_roll: None,
                    damage_roll: Some(recorded_damage),
                    medical_roll: None,
                },
            )
            .await,
        Err(CoreDomainRepositoryError::InvalidInput(
            "combat_roll_evidence"
        ))
    ));
    let mut foreign_lineage = CombatState::start(
        "combat_p08_schema",
        vec![
            CombatantState::new(
                "character_p06_player",
                99,
                CombatHealth::new(30, 30, CombatCondition::Able).unwrap(),
                20,
                CombatSkillTargets::new(99, 99, 99, 99, 99).unwrap(),
                weapon_loadout(1, 5),
            )
            .unwrap(),
            CombatantState::new(
                "npc_marta",
                100,
                CombatHealth::new(30, 30, CombatCondition::Able).unwrap(),
                20,
                CombatSkillTargets::new(100, 100, 100, 100, 100).unwrap(),
                weapon_loadout(0, 5),
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let foreign_attack = percentile_with_result(100, true);
    let foreign_damage = damage_with_value(1, 6, 5, 11);
    foreign_lineage
        .apply_damage(
            "character_p06_player",
            CombatActionKind::Firearm,
            CombatDefense::None,
            &foreign_attack,
            None,
            Some(&foreign_damage),
        )
        .unwrap();
    let foreign_state_json = foreign_lineage.persistence_json().unwrap();
    let foreign_evidence_validation = validate_combat_server_roll_evidence(
        &foreign_state_json,
        Some(&foreign_attack),
        None,
        Some(&foreign_damage),
        None,
    );
    assert!(
        foreign_evidence_validation.is_ok(),
        "foreign evidence should be internally consistent before lineage validation: \
         {foreign_evidence_validation:?}; state={foreign_state_json}"
    );
    let foreign_lineage_result = repository
        .record_combat_state(
            &metadata(
                CAMPAIGN_ID,
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "combat_p08_schema",
                "combat_state",
                "combat.state.damage",
                1,
                "combat_p08_foreign_lineage",
                "party_visible",
                "not_applicable",
                "rules_engine_decision",
            ),
            &RecordCombatStateRequest {
                campaign_id: CAMPAIGN_ID.to_owned(),
                session_id: "session_p06_schema".to_owned(),
                state_json: foreign_state_json,
                attacker_roll: Some(foreign_attack),
                defender_roll: None,
                damage_roll: Some(foreign_damage),
                medical_roll: None,
            },
        )
        .await;
    assert!(
        matches!(
            foreign_lineage_result,
            Err(CoreDomainRepositoryError::InvalidInput("combat_transition"))
        ),
        "unexpected foreign-lineage error: {foreign_lineage_result:?}"
    );
    let combat_events_after_forgery: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM public.event_store \
         WHERE campaign_id = $1 AND stream_id = 'combat_p08_schema'",
    )
    .bind(CAMPAIGN_ID)
    .fetch_one(&primary)
    .await
    .unwrap();
    assert_eq!(
        combat_events_after_forgery, combat_events_before_forgery,
        "a same-ID aggregate from another lineage must be rejected before Event Store append"
    );
    let missed_attack = percentile_with_result(80, false);
    let missed_transition = combat
        .apply_damage(
            "character_p06_player",
            CombatActionKind::Firearm,
            CombatDefense::None,
            &missed_attack,
            None,
            None,
        )
        .unwrap();
    assert_eq!(missed_transition.before_hp, missed_transition.after_hp);
    assert_eq!(missed_transition.damage, 0);
    let missed_state = combat.persistence_json().unwrap();
    assert!(missed_state.contains("\"kind\":\"ATTACK_MISSED\""));
    let missed_attack_metadata = metadata(
        CAMPAIGN_ID,
        AUTHORITY_ID,
        KEEPER_ID,
        "human_keeper",
        "combat_p08_schema",
        "combat_state",
        "combat.state.attack",
        1,
        "combat_p08_missed_attack",
        "party_visible",
        "not_applicable",
        "rules_engine_decision",
    );
    let missed_attack_request = RecordCombatStateRequest {
        campaign_id: CAMPAIGN_ID.to_owned(),
        session_id: "session_p06_schema".to_owned(),
        state_json: missed_state,
        attacker_roll: Some(missed_attack.clone()),
        defender_roll: None,
        damage_roll: None,
        medical_roll: None,
    };
    sqlx::raw_sql(
        r#"
        CREATE FUNCTION public.reject_p08_combat_projection_for_test()
        RETURNS trigger
        LANGUAGE plpgsql
        AS $$
        BEGIN
            IF NEW.combat_id = 'combat_p08_schema' AND NEW.version = 2 THEN
                RAISE EXCEPTION 'injected P08 combat projection failure';
            END IF;
            RETURN NEW;
        END;
        $$;
        CREATE TRIGGER zz_reject_p08_combat_projection_for_test
        BEFORE INSERT OR UPDATE ON public.combat_states
        FOR EACH ROW EXECUTE FUNCTION
            public.reject_p08_combat_projection_for_test();
        "#,
    )
    .execute(&primary)
    .await
    .expect("install P08 projection failure injection");
    assert!(matches!(
        repository
            .record_combat_state(&missed_attack_metadata, &missed_attack_request)
            .await,
        Err(CoreDomainRepositoryError::Database("project_combat_state"))
    ));
    include!("06_combat_atomicity_and_idempotency.rs");
}
