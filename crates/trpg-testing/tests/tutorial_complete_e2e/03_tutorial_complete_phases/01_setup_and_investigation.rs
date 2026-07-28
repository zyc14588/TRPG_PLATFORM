{
    let primary_url =
        env::var("P08_DATABASE_URL").expect("P08_DATABASE_URL is required for the real E2E gate");
    let witness_url = env::var("P08_WITNESS_DATABASE_URL")
        .expect("P08_WITNESS_DATABASE_URL is required for the independent witness gate");
    let primary_database =
        env::var("P08_RESET_DATABASE").expect("P08_RESET_DATABASE must name the dedicated DB");
    let witness_database = env::var("P08_WITNESS_RESET_DATABASE")
        .expect("P08_WITNESS_RESET_DATABASE must name the dedicated witness DB");
    let primary = reset_database(&primary_url, &primary_database, false).await;
    let witness = reset_database(&witness_url, &witness_database, true).await;
    witness.close().await;

    let canonical = PostgresCanonicalStore::connect(
        &primary_url,
        &witness_url,
        "p08-tutorial-integrity-key",
        INTEGRITY_KEY,
        "p08-tutorial-payload-key",
        PAYLOAD_KEY,
    )
    .await
    .expect("connect independent primary and Witness services");
    canonical
        .prepare_for_service()
        .await
        .expect("apply the full forward migration chain");
    let integrity_verifier = canonical.clone();
    let repository = CoreDomainRepository::new(primary.clone(), canonical);

    for (user_id, login) in [
        (KEEPER_ID, "keeper-p08-tutorial"),
        (PLAYER_ID, "player-p08-tutorial"),
    ] {
        sqlx::query(
            "INSERT INTO public.users \
             (user_id, login_normalized, password_hash, global_role) \
             VALUES ($1, $2, 'not-used-by-p08-e2e', 'USER')",
        )
        .bind(user_id)
        .bind(login)
        .execute(&primary)
        .await
        .expect("seed an identity referenced by the production repository");
    }

    let campaign_event_sequence = create_campaign(
        &repository,
        CAMPAIGN_ID,
        AUTHORITY_ID,
        "room_p08_tutorial",
        "p08_campaign_create",
    )
    .await;
    let invite = repository
        .issue_invite(
            &metadata(
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "invite_p08_tutorial",
                "campaign_invite",
                0,
                "p08_invite_issue",
                "private_to_player",
                PLAYER_ID,
                "human_keeper_statement",
            ),
            &IssueInviteRequest {
                invite_id: "invite_p08_tutorial".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                invited_user_id: PLAYER_ID.to_owned(),
                role: MembershipRole::Player,
                expires_at_unix_ms: NOW_MS + 60_000,
            },
        )
        .await
        .expect("issue a real single-use Campaign invite");
    repository
        .accept_invite(
            &metadata(
                AUTHORITY_ID,
                PLAYER_ID,
                "investigator",
                "invite_p08_tutorial",
                "campaign_invite",
                1,
                "p08_invite_accept",
                "private_to_player",
                PLAYER_ID,
                "user_statement",
            ),
            &AcceptInviteRequest {
                campaign_id: CAMPAIGN_ID.to_owned(),
                invite_id: "invite_p08_tutorial".to_owned(),
                accepting_user_id: PLAYER_ID.to_owned(),
                raw_token: invite.raw_token,
            },
        )
        .await
        .expect("accept the invite into durable membership");
    repository
        .create_character(
            &metadata(
                AUTHORITY_ID,
                PLAYER_ID,
                "investigator",
                CHARACTER_ID,
                "character",
                0,
                "p08_character_create",
                "private_to_player",
                PLAYER_ID,
                "user_statement",
            ),
            &CreateCharacterRequest {
                character_id: CHARACTER_ID.to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                owner_user_id: PLAYER_ID.to_owned(),
                display_name: "Evelyn Hart".to_owned(),
                sheet_version_id: "sheet_p08_evelyn_v1".to_owned(),
                sheet_json: character_sheet(),
            },
        )
        .await
        .expect("create the Tutorial investigator");
    repository
        .submit_character(
            &metadata(
                AUTHORITY_ID,
                PLAYER_ID,
                "investigator",
                CHARACTER_ID,
                "character",
                1,
                "p08_character_submit",
                "private_to_player",
                PLAYER_ID,
                "user_statement",
            ),
            CAMPAIGN_ID,
            CHARACTER_ID,
        )
        .await
        .expect("submit the Tutorial investigator");
    repository
        .approve_character_initial_version(
            &metadata(
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                CHARACTER_ID,
                "character",
                2,
                "p08_character_approve",
                "private_to_player",
                PLAYER_ID,
                "human_keeper_statement",
            ),
            CAMPAIGN_ID,
            CHARACTER_ID,
        )
        .await
        .expect("approve and lock the initial Character Sheet");

    let scenario = parse_scenario_yaml(TUTORIAL).expect("validate the actual Tutorial Scenario");
    assert_eq!(scenario.opening_scene_id, "scene_archive_front");
    assert!(scenario
        .encounter_ids
        .contains(&"encounter_basement_confrontation".to_owned()));
    assert!(scenario
        .encounter_ids
        .contains(&"encounter_archive_escape".to_owned()));
    assert!(scenario
        .ending_ids
        .contains(&"ending_expose_marta".to_owned()));
    assert!(scenario.growth_skills.contains(&"Library Use".to_owned()));
    repository
        .import_scenario(
            &metadata(
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "scenario_p08_tutorial",
                "scenario",
                0,
                "p08_scenario_import",
                "keeper_only",
                "not_applicable",
                "imported_source",
            ),
            &ImportScenarioRequest {
                scenario_id: "scenario_p08_tutorial".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                ruleset_id: scenario.ruleset_id,
                format_version: scenario.format_version,
                content_hash: scenario.content_hash,
                document_json: scenario.canonical_json,
            },
        )
        .await
        .expect("import the validated Tutorial document");
    repository
        .start_session(
            &metadata(
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                SESSION_ID,
                "session",
                0,
                "p08_session_start",
                "party_visible",
                "not_applicable",
                "human_keeper_statement",
            ),
            &StartSessionRequest {
                session_id: SESSION_ID.to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                room_id: "room_p08_tutorial".to_owned(),
                scenario_id: "scenario_p08_tutorial".to_owned(),
                scene_id: "scene_p08_front".to_owned(),
                scene_key: "scene_archive_front".to_owned(),
                scene_name: "灰港市政档案室前厅".to_owned(),
                started_at_unix_ms: NOW_MS + 2_000,
            },
        )
        .await
        .expect("start the real Tutorial Session");

    repository
        .submit_player_action(
            &metadata(
                AUTHORITY_ID,
                PLAYER_ID,
                "investigator",
                "action_p08_investigation",
                "player_action",
                0,
                "p08_investigation_submit",
                "party_visible",
                "not_applicable",
                "user_statement",
            ),
            &SubmitPlayerActionRequest {
                action_id: "action_p08_investigation".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                character_id: CHARACTER_ID.to_owned(),
                scene_id: "scene_p08_front".to_owned(),
                submitted_by: PLAYER_ID.to_owned(),
                submitted_at_unix_ms: NOW_MS + 3_000,
                intent: PlayerActionIntentRecord::Investigation {
                    skill_name: "Library Use".to_owned(),
                    clue_id: "clue_wrong_signature".to_owned(),
                    clue_importance: "CORE".to_owned(),
                    adjustment: "NONE".to_owned(),
                },
            },
        )
        .await
        .expect("submit a real investigation action");
    let investigation_roll =
        server_roll_skill_check(70, DiceAdjustment::None).expect("server investigation roll");
    let investigation_succeeded = succeeded(investigation_roll.outcome().success_level);
    repository
        .commit_investigation_execution(
            &metadata(
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "action_p08_investigation",
                "player_action",
                1,
                "p08_investigation_confirm",
                "party_visible",
                "not_applicable",
                "human_keeper_statement",
            ),
            &InvestigationExecutionRecord {
                action_id: "action_p08_investigation".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                character_id: CHARACTER_ID.to_owned(),
                decision_id: "decision_p08_investigation".to_owned(),
                tool_execution_id: "tool_execution_p08_investigation".to_owned(),
                confirmed_by: KEEPER_ID.to_owned(),
                resolved_at_unix_ms: NOW_MS + 4_000,
                dice: server_dice_record(&investigation_roll),
                skill_name: "Library Use".to_owned(),
                clue_record_id: "clue_result_p08_wrong_signature".to_owned(),
                clue_id: "clue_wrong_signature".to_owned(),
                clue_importance: "CORE".to_owned(),
                clue_outcome: if investigation_succeeded {
                    "REVEALED"
                } else {
                    "REVEALED_WITH_COST"
                }
                .to_owned(),
                clue_cost: (!investigation_succeeded).then_some("time_or_complication".to_owned()),
                revealed_to_party: true,
            },
        )
        .await
        .expect("commit investigation Decision, Dice and Clue atomically");

    repository
        .submit_player_action(
            &metadata(
                AUTHORITY_ID,
                PLAYER_ID,
                "investigator",
                "action_p08_sanity",
                "player_action",
                0,
                "p08_sanity_submit",
                "private_to_player",
                PLAYER_ID,
                "user_statement",
            ),
            &SubmitPlayerActionRequest {
                action_id: "action_p08_sanity".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                character_id: CHARACTER_ID.to_owned(),
                scene_id: "scene_p08_front".to_owned(),
                submitted_by: PLAYER_ID.to_owned(),
                submitted_at_unix_ms: NOW_MS + 5_000,
                intent: PlayerActionIntentRecord::SanityCheck {
                    success_loss: 0,
                    failure_loss: 3,
                    day_key: "tutorial_day_1".to_owned(),
                },
            },
        )
        .await
        .expect("submit a real SAN action");
    let sanity_roll = server_roll_skill_check(65, DiceAdjustment::None).expect("server SAN roll");
    let sanity_loss = if succeeded(sanity_roll.outcome().success_level) {
        0
    } else {
        3
    };
    include!("02_sanity_combat_and_chase.rs");
}
