{
    let recovered = sqlx::query(
        r#"
        SELECT session.state, session.version, scene.state AS scene_state
          FROM core_domain.sessions AS session
          JOIN public.scenes AS scene
            ON scene.scene_id = session.active_scene_id
         WHERE session.session_id = 'session_p06_schema'
        "#,
    )
    .fetch_one(&primary)
    .await
    .unwrap();
    assert_eq!(recovered.get::<String, _>("state"), "ENDED");
    assert_eq!(recovered.get::<i64, _>("version"), 5);
    assert_eq!(recovered.get::<String, _>("scene_state"), "CLOSED");

    repository
        .start_session(
            &metadata(
                CAMPAIGN_ID,
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "session_p08_after_growth",
                "session",
                "session.start",
                0,
                "session_p08_after_growth_start",
                "party_visible",
                "not_applicable",
                "human_keeper_statement",
            ),
            &StartSessionRequest {
                session_id: "session_p08_after_growth".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                room_id: "room_p06_schema".to_owned(),
                scenario_id: "scenario_p06_tutorial".to_owned(),
                scene_id: "scene_p08_after_growth".to_owned(),
                scene_key: "scene_archive_front".to_owned(),
                scene_name: "After Growth".to_owned(),
                started_at_unix_ms: NOW_MS + 40_000,
            },
        )
        .await
        .expect("start later gameplay after the P08 Growth event");
    repository
        .submit_player_action(
            &metadata(
                CAMPAIGN_ID,
                AUTHORITY_ID,
                PLAYER_ID,
                "investigator",
                "action_p08_after_growth_sanity",
                "player_action",
                "player_action.submit",
                0,
                "action_p08_after_growth_sanity_submit",
                "private_to_player",
                PLAYER_ID,
                "user_statement",
            ),
            &SubmitPlayerActionRequest {
                action_id: "action_p08_after_growth_sanity".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                character_id: "character_p06_player".to_owned(),
                scene_id: "scene_p08_after_growth".to_owned(),
                submitted_by: PLAYER_ID.to_owned(),
                submitted_at_unix_ms: NOW_MS + 41_000,
                intent: PlayerActionIntentRecord::SanityCheck {
                    success_loss: 1,
                    failure_loss: 1,
                    day_key: "after_growth_day".to_owned(),
                },
            },
        )
        .await
        .expect("submit a later SAN action against the Growth-derived sheet");
    repository
        .commit_sanity_execution(
            &metadata(
                CAMPAIGN_ID,
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "action_p08_after_growth_sanity",
                "player_action",
                "player_action.confirm",
                1,
                "action_p08_after_growth_sanity_confirm",
                "private_to_player",
                PLAYER_ID,
                "human_keeper_statement",
            ),
            &SanityExecutionRecord {
                action_id: "action_p08_after_growth_sanity".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                character_id: "character_p06_player".to_owned(),
                decision_id: "decision_p08_after_growth_sanity".to_owned(),
                tool_execution_id: "tool_p08_after_growth_sanity".to_owned(),
                confirmed_by: KEEPER_ID.to_owned(),
                resolved_at_unix_ms: NOW_MS + 42_000,
                dice: PlayerActionDiceRecord {
                    roll_id: "roll_p08_after_growth_sanity".to_owned(),
                    target_value: 65,
                    rolled_value: 42,
                    success_level: "REGULAR".to_owned(),
                    selected_tens_digit: 4,
                    ones_digit: 2,
                    adjustment: "NONE".to_owned(),
                },
                sanity_event_id: "sanity_p08_after_growth".to_owned(),
                sheet_version_id: "sheet_p06_player_v3_sanity".to_owned(),
                day_key: "after_growth_day".to_owned(),
                day_start_sanity: 65,
                sanity_before: 65,
                sanity_after: 64,
                sanity_loss: 1,
                day_loss: 1,
                indefinite_threshold: 13,
                madness_state: "STABLE".to_owned(),
            },
        )
        .await
        .expect("commit a later SAN mutation after Growth");
    let later_character_before_rebuild: serde_json::Value = sqlx::query_scalar(
        r#"
        SELECT jsonb_build_object(
            'character', (
                SELECT to_jsonb(character)
                  FROM public.characters AS character
                 WHERE character.character_id = 'character_p06_player'
            ),
            'growth_sheet', (
                SELECT to_jsonb(sheet)
                  FROM public.character_sheet_versions AS sheet
                 WHERE sheet.sheet_version_id = 'sheet_p06_player_v2'
            ),
            'sanity_sheet', (
                SELECT to_jsonb(sheet)
                  FROM public.character_sheet_versions AS sheet
                 WHERE sheet.sheet_version_id = 'sheet_p06_player_v3_sanity'
            ),
            'growth', (
                SELECT to_jsonb(growth)
                  FROM public.growth_events AS growth
                 WHERE growth.growth_event_id = 'growth_event_p08_schema'
            ),
            'sanity', (
                SELECT to_jsonb(sanity)
                  FROM public.sanity_events AS sanity
                 WHERE sanity.sanity_event_id = 'sanity_p08_after_growth'
            )
        )
        "#,
    )
    .fetch_one(&primary)
    .await
    .unwrap();
    let events_before_later_character_rebuild: i64 =
        sqlx::query_scalar("SELECT count(*) FROM public.event_store WHERE campaign_id = $1")
            .bind(CAMPAIGN_ID)
            .fetch_one(&primary)
            .await
            .unwrap();
    let later_character_rebuild = api_repository
        .rebuild_p08_projections(CAMPAIGN_ID)
        .await
        .expect("rebuild Growth without rewinding a later SAN mutation");
    assert_eq!(later_character_rebuild.growth_events, 1);
    let later_character_after_rebuild: serde_json::Value = sqlx::query_scalar(
        r#"
        SELECT jsonb_build_object(
            'character', (
                SELECT to_jsonb(character)
                  FROM public.characters AS character
                 WHERE character.character_id = 'character_p06_player'
            ),
            'growth_sheet', (
                SELECT to_jsonb(sheet)
                  FROM public.character_sheet_versions AS sheet
                 WHERE sheet.sheet_version_id = 'sheet_p06_player_v2'
            ),
            'sanity_sheet', (
                SELECT to_jsonb(sheet)
                  FROM public.character_sheet_versions AS sheet
                 WHERE sheet.sheet_version_id = 'sheet_p06_player_v3_sanity'
            ),
            'growth', (
                SELECT to_jsonb(growth)
                  FROM public.growth_events AS growth
                 WHERE growth.growth_event_id = 'growth_event_p08_schema'
            ),
            'sanity', (
                SELECT to_jsonb(sanity)
                  FROM public.sanity_events AS sanity
                 WHERE sanity.sanity_event_id = 'sanity_p08_after_growth'
            )
        )
        "#,
    )
    .fetch_one(&primary)
    .await
    .unwrap();
    assert_eq!(
        later_character_after_rebuild, later_character_before_rebuild,
        "P08 replay must preserve a later SAN character/sheet while reconstructing Growth"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM public.event_store WHERE campaign_id = $1",
        )
        .bind(CAMPAIGN_ID)
        .fetch_one(&primary)
        .await
        .unwrap(),
        events_before_later_character_rebuild,
        "preserving later character mutations must not rewrite canonical history"
    );
    include!("19_empty_history_rebuild_and_guards.rs");
}
