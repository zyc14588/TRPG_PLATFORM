{
    api_repository
        .import_scenario(
            &metadata(
                CHILD_CAMPAIGN_ID,
                CHILD_AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "scenario_p08_post_fork",
                "scenario",
                "scenario.import",
                0,
                "scenario_p08_post_fork_import",
                "keeper_only",
                "not_applicable",
                "imported_source",
            ),
            &ImportScenarioRequest {
                scenario_id: "scenario_p08_post_fork".to_owned(),
                campaign_id: CHILD_CAMPAIGN_ID.to_owned(),
                ruleset_id: post_fork_tutorial.ruleset_id,
                format_version: post_fork_tutorial.format_version,
                content_hash: post_fork_tutorial.content_hash,
                document_json: post_fork_tutorial.canonical_json,
            },
        )
        .await
        .expect("import a scenario after the fork materialization");
    repository
        .create_character(
            &metadata(
                CHILD_CAMPAIGN_ID,
                CHILD_AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "character_p08_post_fork",
                "character",
                "character.create",
                0,
                "character_p08_post_fork_create",
                "private_to_player",
                KEEPER_ID,
                "user_statement",
            ),
            &CreateCharacterRequest {
                character_id: "character_p08_post_fork".to_owned(),
                campaign_id: CHILD_CAMPAIGN_ID.to_owned(),
                owner_user_id: KEEPER_ID.to_owned(),
                display_name: "Post-fork Investigator".to_owned(),
                sheet_version_id: "sheet_p08_post_fork_v1".to_owned(),
                sheet_json: r#"{"name":"Post-fork Investigator","ruleset":"coc7","characteristics":{"power":60},"skills":{"Library Use":70}}"#.to_owned(),
            },
        )
        .await
        .expect("create a child-owned character after the fork materialization");
    repository
        .submit_character(
            &metadata(
                CHILD_CAMPAIGN_ID,
                CHILD_AUTHORITY_ID,
                KEEPER_ID,
                "investigator",
                "character_p08_post_fork",
                "character",
                "character.submit",
                1,
                "character_p08_post_fork_submit",
                "private_to_player",
                KEEPER_ID,
                "user_statement",
            ),
            CHILD_CAMPAIGN_ID,
            "character_p08_post_fork",
        )
        .await
        .expect("submit the post-fork character");
    repository
        .approve_character_initial_version(
            &metadata(
                CHILD_CAMPAIGN_ID,
                CHILD_AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "character_p08_post_fork",
                "character",
                "character.review_initial",
                2,
                "character_p08_post_fork_approve",
                "private_to_player",
                KEEPER_ID,
                "human_keeper_statement",
            ),
            CHILD_CAMPAIGN_ID,
            "character_p08_post_fork",
        )
        .await
        .expect("approve the post-fork character");
    repository
        .start_session(
            &metadata(
                CHILD_CAMPAIGN_ID,
                CHILD_AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "session_p08_post_fork",
                "session",
                "session.start",
                0,
                "session_p08_post_fork_start",
                "party_visible",
                "not_applicable",
                "human_keeper_statement",
            ),
            &StartSessionRequest {
                session_id: "session_p08_post_fork".to_owned(),
                campaign_id: CHILD_CAMPAIGN_ID.to_owned(),
                room_id: "room_p06_fork_child".to_owned(),
                scenario_id: "scenario_p08_post_fork".to_owned(),
                scene_id: "scene_p08_post_fork".to_owned(),
                scene_key: "scene_archive_front".to_owned(),
                scene_name: "Post-fork Scene".to_owned(),
                started_at_unix_ms: NOW_MS + 30_000,
            },
        )
        .await
        .expect("start a child-owned session after the fork materialization");
    let copied_late_joiner_character_id: String = sqlx::query_scalar(
        r#"
        SELECT character_id
          FROM public.characters
         WHERE campaign_id = $1
           AND owner_user_id = $2
           AND display_name = 'Late Joining Investigator'
        "#,
    )
    .bind(CHILD_CAMPAIGN_ID)
    .bind(KEEPER_ID)
    .fetch_one(&primary)
    .await
    .expect("load the late-joining character copied into the fork");
    repository
        .submit_player_action(
            &metadata(
                CHILD_CAMPAIGN_ID,
                CHILD_AUTHORITY_ID,
                KEEPER_ID,
                "investigator",
                "action_p08_copied_character_sanity",
                "player_action",
                "player_action.submit",
                0,
                "action_p08_copied_character_sanity_submit",
                "private_to_player",
                KEEPER_ID,
                "user_statement",
            ),
            &SubmitPlayerActionRequest {
                action_id: "action_p08_copied_character_sanity".to_owned(),
                campaign_id: CHILD_CAMPAIGN_ID.to_owned(),
                character_id: copied_late_joiner_character_id.clone(),
                scene_id: "scene_p08_post_fork".to_owned(),
                submitted_by: KEEPER_ID.to_owned(),
                submitted_at_unix_ms: NOW_MS + 30_100,
                intent: PlayerActionIntentRecord::SanityCheck {
                    success_loss: 1,
                    failure_loss: 1,
                    day_key: "copied_character_day".to_owned(),
                },
            },
        )
        .await
        .expect("submit SAN against a character copied by the fork");
    repository
        .commit_sanity_execution(
            &metadata(
                CHILD_CAMPAIGN_ID,
                CHILD_AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "action_p08_copied_character_sanity",
                "player_action",
                "player_action.confirm",
                1,
                "action_p08_copied_character_sanity_confirm",
                "private_to_player",
                KEEPER_ID,
                "human_keeper_statement",
            ),
            &SanityExecutionRecord {
                action_id: "action_p08_copied_character_sanity".to_owned(),
                campaign_id: CHILD_CAMPAIGN_ID.to_owned(),
                character_id: copied_late_joiner_character_id.clone(),
                decision_id: "decision_p08_copied_character_sanity".to_owned(),
                tool_execution_id: "tool_p08_copied_character_sanity".to_owned(),
                confirmed_by: KEEPER_ID.to_owned(),
                resolved_at_unix_ms: NOW_MS + 30_200,
                dice: PlayerActionDiceRecord {
                    roll_id: "roll_p08_copied_character_sanity".to_owned(),
                    target_value: 55,
                    rolled_value: 42,
                    success_level: "REGULAR".to_owned(),
                    selected_tens_digit: 4,
                    ones_digit: 2,
                    adjustment: "NONE".to_owned(),
                },
                sanity_event_id: "sanity_p08_copied_character".to_owned(),
                sheet_version_id: "sheet_p08_copied_character_sanity".to_owned(),
                day_key: "copied_character_day".to_owned(),
                day_start_sanity: 55,
                sanity_before: 55,
                sanity_after: 54,
                sanity_loss: 1,
                day_loss: 1,
                indefinite_threshold: 11,
                madness_state: "STABLE".to_owned(),
            },
        )
        .await
        .expect("commit SAN after fork materialization on the copied character");
    repository
        .change_session_state(
            &metadata(
                CHILD_CAMPAIGN_ID,
                CHILD_AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "session_p08_post_fork",
                "session",
                "session.end",
                1,
                "session_p08_post_fork_end",
                "party_visible",
                "not_applicable",
                "human_keeper_statement",
            ),
            CHILD_CAMPAIGN_ID,
            "session_p08_post_fork",
            SessionState::Ended,
            NOW_MS + 31_000,
        )
        .await
        .expect("end the post-fork child session");
    repository
        .record_ending(
            &metadata(
                CHILD_CAMPAIGN_ID,
                CHILD_AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "ending_p08_post_fork",
                "ending",
                "ending.record",
                0,
                "ending_p08_post_fork_record",
                "party_visible",
                "not_applicable",
                "human_keeper_statement",
            ),
            &RecordEndingRequest {
                ending_event_id: "ending_p08_post_fork".to_owned(),
                campaign_id: CHILD_CAMPAIGN_ID.to_owned(),
                session_id: "session_p08_post_fork".to_owned(),
                ending_id: "ending_expose_marta".to_owned(),
                summary: "The post-fork investigator completes the case.".to_owned(),
                ended_at_unix_ms: NOW_MS + 32_000,
            },
        )
        .await
        .expect("record an ending for the post-fork child session");
    repository
        .record_growth(
            &metadata(
                CHILD_CAMPAIGN_ID,
                CHILD_AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "growth_p08_post_fork",
                "growth",
                "growth.record",
                0,
                "growth_p08_post_fork_record",
                "private_to_player",
                KEEPER_ID,
                "rules_engine_decision",
            ),
            &RecordGrowthRequest {
                growth_event_id: "growth_p08_post_fork".to_owned(),
                campaign_id: CHILD_CAMPAIGN_ID.to_owned(),
                session_id: "session_p08_post_fork".to_owned(),
                ending_event_id: "ending_p08_post_fork".to_owned(),
                character_id: "character_p08_post_fork".to_owned(),
                source_sheet_version_id: "sheet_p08_post_fork_v1".to_owned(),
                new_sheet_version_id: "sheet_p08_post_fork_v2".to_owned(),
                skill_name: "Library Use".to_owned(),
                growth_rolls: server_roll_skill_growth(70).unwrap().evidence().clone(),
            },
        )
        .await
        .expect("record Growth for a normal character created after the fork");
    let copied_fork_character_before: serde_json::Value = sqlx::query_scalar(
        r#"
        SELECT jsonb_build_object(
            'character', (
                SELECT to_jsonb(character)
                  FROM public.characters AS character
                 WHERE character.campaign_id = $1
                   AND character.character_id = $2
            ),
            'sheets', (
                SELECT jsonb_agg(to_jsonb(sheet) ORDER BY sheet.version)
                  FROM public.character_sheet_versions AS sheet
                 WHERE sheet.campaign_id = $1
                   AND sheet.character_id = $2
            ),
            'action', (
                SELECT to_jsonb(action)
                  FROM public.player_actions AS action
                 WHERE action.campaign_id = $1
                   AND action.action_id =
                       'action_p08_copied_character_sanity'
            ),
            'sanity', (
                SELECT to_jsonb(sanity)
                  FROM public.sanity_events AS sanity
                 WHERE sanity.campaign_id = $1
                   AND sanity.sanity_event_id =
                       'sanity_p08_copied_character'
            )
        )
        "#,
    )
    .bind(CHILD_CAMPAIGN_ID)
    .bind(&copied_late_joiner_character_id)
    .fetch_one(&primary)
    .await
    .unwrap();
    include!("14_post_fork_snapshot_comparison.rs");
}
