{
    repository
        .import_scenario(
            &metadata(
                CAMPAIGN_ID,
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "scenario_p06_tutorial",
                "scenario",
                "scenario.import",
                0,
                "scenario_import",
                "keeper_only",
                "not_applicable",
                "imported_source",
            ),
            &ImportScenarioRequest {
                scenario_id: "scenario_p06_tutorial".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                ruleset_id: tutorial.ruleset_id,
                format_version: tutorial.format_version,
                content_hash: tutorial.content_hash,
                document_json: tutorial.canonical_json,
            },
        )
        .await
        .expect("persist validated Tutorial Scenario");

    let session_events_before_bad_scene: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM public.event_store \
         WHERE campaign_id = $1 AND event_type IN ('SessionStarted', 'SceneSwitched')",
    )
    .bind(CAMPAIGN_ID)
    .fetch_one(&primary)
    .await
    .unwrap();
    assert!(matches!(
        repository
            .start_session(
                &metadata(
                    CAMPAIGN_ID,
                    AUTHORITY_ID,
                    KEEPER_ID,
                    "human_keeper",
                    "session_p06_bad_scene",
                    "session",
                    "session.start",
                    0,
                    "session_bad_scenario_scene",
                    "party_visible",
                    "not_applicable",
                    "human_keeper_statement",
                ),
                &StartSessionRequest {
                    session_id: "session_p06_bad_scene".to_owned(),
                    campaign_id: CAMPAIGN_ID.to_owned(),
                    room_id: "room_p06_schema".to_owned(),
                    scenario_id: "scenario_p06_tutorial".to_owned(),
                    scene_id: "scene_p06_bad".to_owned(),
                    scene_key: "scene_not_in_scenario".to_owned(),
                    scene_name: "Unbound Scene".to_owned(),
                    started_at_unix_ms: NOW_MS + 1_900,
                },
            )
            .await,
        Err(CoreDomainRepositoryError::InvalidInput(
            "scenario_scene_key"
        ))
    ));
    repository
        .start_session(
            &metadata(
                CAMPAIGN_ID,
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "session_p06_schema",
                "session",
                "session.start",
                0,
                "session_start",
                "party_visible",
                "not_applicable",
                "human_keeper_statement",
            ),
            &StartSessionRequest {
                session_id: "session_p06_schema".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                room_id: "room_p06_schema".to_owned(),
                scenario_id: "scenario_p06_tutorial".to_owned(),
                scene_id: "scene_p06_front".to_owned(),
                scene_key: "scene_archive_front".to_owned(),
                scene_name: "灰港市政档案室前厅".to_owned(),
                started_at_unix_ms: NOW_MS + 2_000,
            },
        )
        .await
        .expect("start session with active scene");
    assert!(matches!(
        repository
            .switch_scene(
                &metadata(
                    CAMPAIGN_ID,
                    AUTHORITY_ID,
                    KEEPER_ID,
                    "human_keeper",
                    "session_p06_schema",
                    "session",
                    "scene.switch",
                    1,
                    "scene_switch_bad_scenario_scene",
                    "party_visible",
                    "not_applicable",
                    "human_keeper_statement",
                ),
                &SwitchSceneRequest {
                    session_id: "session_p06_schema".to_owned(),
                    campaign_id: CAMPAIGN_ID.to_owned(),
                    next_scene_id: "scene_p06_bad_switch".to_owned(),
                    next_scene_key: "scene_not_in_scenario".to_owned(),
                    next_scene_name: "Unbound Scene".to_owned(),
                    switched_at_unix_ms: NOW_MS + 2_900,
                },
            )
            .await,
        Err(CoreDomainRepositoryError::InvalidInput(
            "scenario_scene_key"
        ))
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM public.event_store \
             WHERE campaign_id = $1 \
               AND event_type IN ('SessionStarted', 'SceneSwitched')",
        )
        .bind(CAMPAIGN_ID)
        .fetch_one(&primary)
        .await
        .unwrap(),
        session_events_before_bad_scene + 1,
        "unbound start/switch scene keys must not append canonical events"
    );
    repository
        .switch_scene(
            &metadata(
                CAMPAIGN_ID,
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "session_p06_schema",
                "session",
                "scene.switch",
                1,
                "scene_switch",
                "party_visible",
                "not_applicable",
                "human_keeper_statement",
            ),
            &SwitchSceneRequest {
                session_id: "session_p06_schema".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                next_scene_id: "scene_p06_basement".to_owned(),
                next_scene_key: "scene_basement".to_owned(),
                next_scene_name: "地下盐窖".to_owned(),
                switched_at_unix_ms: NOW_MS + 3_000,
            },
        )
        .await
        .expect("switch active scene");
    repository
        .change_session_state(
            &metadata(
                CAMPAIGN_ID,
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "session_p06_schema",
                "session",
                "session.pause",
                2,
                "session_pause",
                "party_visible",
                "not_applicable",
                "human_keeper_statement",
            ),
            CAMPAIGN_ID,
            "session_p06_schema",
            SessionState::Paused,
            NOW_MS + 4_000,
        )
        .await
        .expect("pause active session");
    repository
        .change_session_state(
            &metadata(
                CAMPAIGN_ID,
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "session_p06_schema",
                "session",
                "session.resume",
                3,
                "session_resume",
                "party_visible",
                "not_applicable",
                "human_keeper_statement",
            ),
            CAMPAIGN_ID,
            "session_p06_schema",
            SessionState::Active,
            NOW_MS + 5_000,
        )
        .await
        .expect("resume paused session");

    repository
        .create_character(
            &metadata(
                CAMPAIGN_ID,
                AUTHORITY_ID,
                KEEPER_ID,
                "investigator",
                "character_p08_late_joiner",
                "character",
                "character.create",
                0,
                "character_p08_late_joiner_create",
                "private_to_player",
                KEEPER_ID,
                "user_statement",
            ),
            &CreateCharacterRequest {
                character_id: "character_p08_late_joiner".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                owner_user_id: KEEPER_ID.to_owned(),
                display_name: "Late Joining Investigator".to_owned(),
                sheet_version_id: "sheet_p08_late_joiner_v1".to_owned(),
                sheet_json: r#"{"name":"Late Joining Investigator","ruleset":"coc7","characteristics":{"power":55},"skills":{"Library Use":60}}"#.to_owned(),
            },
        )
        .await
        .expect("create an idle character after the source session starts");
    repository
        .submit_character(
            &metadata(
                CAMPAIGN_ID,
                AUTHORITY_ID,
                KEEPER_ID,
                "investigator",
                "character_p08_late_joiner",
                "character",
                "character.submit",
                1,
                "character_p08_late_joiner_submit",
                "private_to_player",
                KEEPER_ID,
                "user_statement",
            ),
            CAMPAIGN_ID,
            "character_p08_late_joiner",
        )
        .await
        .expect("submit the late-joining character without a session action");
    repository
        .approve_character_initial_version(
            &metadata(
                CAMPAIGN_ID,
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "character_p08_late_joiner",
                "character",
                "character.review_initial",
                2,
                "character_p08_late_joiner_approve",
                "private_to_player",
                KEEPER_ID,
                "human_keeper_statement",
            ),
            CAMPAIGN_ID,
            "character_p08_late_joiner",
        )
        .await
        .expect("approve the idle late joiner before the source cutoff");
    include!("05_combat_evidence_validation.rs");
}
