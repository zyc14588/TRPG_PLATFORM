{

    Box::pin(async {
        create_campaign(
            &repository,
            EMPTY_P08_CAMPAIGN_ID,
            EMPTY_P08_AUTHORITY_ID,
            "room_p08_empty_rebuild",
            "p08_empty_rebuild",
        )
        .await;
        let empty_tutorial = parse_scenario_yaml(include_str!(
            "../../../../../fixtures/scenarios/tutorial_mist_archive.scenario.yaml"
        ))
        .unwrap();
        repository
            .import_scenario(
                &metadata(
                    EMPTY_P08_CAMPAIGN_ID,
                    EMPTY_P08_AUTHORITY_ID,
                    KEEPER_ID,
                    "human_keeper",
                    "scenario_p08_empty_rebuild",
                    "scenario",
                    "scenario.import",
                    0,
                    "scenario_p08_empty_rebuild",
                    "keeper_only",
                    "not_applicable",
                    "imported_source",
                ),
                &ImportScenarioRequest {
                    scenario_id: "scenario_p08_empty_rebuild".to_owned(),
                    campaign_id: EMPTY_P08_CAMPAIGN_ID.to_owned(),
                    ruleset_id: empty_tutorial.ruleset_id,
                    format_version: empty_tutorial.format_version,
                    content_hash: empty_tutorial.content_hash,
                    document_json: empty_tutorial.canonical_json,
                },
            )
            .await
            .expect("import a scenario without creating any P08 canonical event");
        repository
            .start_session(
                &metadata(
                    EMPTY_P08_CAMPAIGN_ID,
                    EMPTY_P08_AUTHORITY_ID,
                    KEEPER_ID,
                    "human_keeper",
                    "session_p08_empty_rebuild",
                    "session",
                    "session.start",
                    0,
                    "session_p08_empty_rebuild",
                    "party_visible",
                    "not_applicable",
                    "human_keeper_statement",
                ),
                &StartSessionRequest {
                    session_id: "session_p08_empty_rebuild".to_owned(),
                    campaign_id: EMPTY_P08_CAMPAIGN_ID.to_owned(),
                    room_id: "room_p08_empty_rebuild".to_owned(),
                    scenario_id: "scenario_p08_empty_rebuild".to_owned(),
                    scene_id: "scene_p08_empty_rebuild".to_owned(),
                    scene_key: "scene_archive_front".to_owned(),
                    scene_name: "Empty rebuild fixture".to_owned(),
                    started_at_unix_ms: NOW_MS + 20_000,
                },
            )
            .await
            .expect("start a non-P08 Session for the empty-history rebuild");
        let empty_authorizing_sequence: i64 = sqlx::query_scalar(
            "SELECT last_event_sequence FROM core_domain.sessions \
         WHERE session_id = 'session_p08_empty_rebuild'",
        )
        .fetch_one(&primary)
        .await
        .unwrap();
        let ghost_empty_combat = CombatState::start(
            "combat_p08_empty_ghost",
            vec![
                CombatantState::new(
                    "ghost_investigator",
                    70,
                    CombatHealth::new(10, 10, CombatCondition::Able).unwrap(),
                    0,
                    CombatSkillTargets::new(45, 35, 40, 30, 10).unwrap(),
                    weapon_loadout(1, 5),
                )
                .unwrap(),
                CombatantState::new(
                    "ghost_npc",
                    60,
                    CombatHealth::new(8, 8, CombatCondition::Able).unwrap(),
                    0,
                    CombatSkillTargets::new(40, 40, 30, 20, 10).unwrap(),
                    weapon_loadout(0, 5),
                )
                .unwrap(),
            ],
        )
        .unwrap();
        let mut inject_empty_ghost = primary.begin().await.unwrap();
        sqlx::query(
            "ALTER TABLE public.combat_states \
         DISABLE TRIGGER combat_states_event_guard",
        )
        .execute(&mut *inject_empty_ghost)
        .await
        .unwrap();
        sqlx::query(
            r#"
        INSERT INTO public.combat_states (
            combat_id, campaign_id, session_id, status, round,
            current_turn_index, state_json, version,
            visibility_label, visibility_subject,
            provenance_kind, provenance_reference, provenance_recorded_by,
            last_event_sequence
        ) VALUES (
            'combat_p08_empty_ghost', $1, 'session_p08_empty_rebuild',
            'ONGOING', 1, 0, $2::JSONB, 1,
            'party_visible', 'not_applicable',
            'system_fixture', 'empty_rebuild_ghost', 'test_workflow', $3
        )
        "#,
        )
        .bind(EMPTY_P08_CAMPAIGN_ID)
        .bind(ghost_empty_combat.persistence_json().unwrap())
        .bind(empty_authorizing_sequence)
        .execute(&mut *inject_empty_ghost)
        .await
        .unwrap();
        sqlx::query("SET CONSTRAINTS ALL IMMEDIATE")
            .execute(&mut *inject_empty_ghost)
            .await
            .unwrap();
        sqlx::query(
            "ALTER TABLE public.combat_states \
         ENABLE TRIGGER combat_states_event_guard",
        )
        .execute(&mut *inject_empty_ghost)
        .await
        .unwrap();
        inject_empty_ghost.commit().await.unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM public.event_store \
             WHERE campaign_id = $1 \
               AND event_type = 'CombatStateRecorded'",
            )
            .bind(EMPTY_P08_CAMPAIGN_ID)
            .fetch_one(&primary)
            .await
            .unwrap(),
            0
        );
        let empty_rebuild = api_repository
            .rebuild_p08_projections(EMPTY_P08_CAMPAIGN_ID)
            .await
            .expect("an empty canonical P08 history must clear ghost projections");
        assert_eq!(empty_rebuild.replayed_events, 0);
        assert_eq!(empty_rebuild.combat_states, 0);
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM public.combat_states \
             WHERE campaign_id = $1",
            )
            .bind(EMPTY_P08_CAMPAIGN_ID)
            .fetch_one(&primary)
            .await
            .unwrap(),
            0,
            "empty-history rebuild must remove a non-canonical combat projection"
        );
    })
    .await;

    assert!(
        sqlx::query(
            "UPDATE public.characters SET display_name = 'tampered' \
             WHERE character_id = 'character_p06_player'"
        )
        .execute(&primary)
        .await
        .is_err(),
        "projection mutation without a newer matching canonical event must fail"
    );
}
