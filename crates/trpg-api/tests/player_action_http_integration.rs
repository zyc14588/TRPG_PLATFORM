// Source is organized into ordered, responsibility-focused sections.
// `include!` preserves this module's privacy boundary and public API.

#[path = "../../../apps/api-server/src/player_action.rs"]
mod production_player_action;

async fn seed_tutorial(repository: &CoreDomainRepository, primary: &PgPool) {
    for (id, login) in [
        (KEEPER_ID, "keeper-p07-http"),
        (PLAYER_ID, "player-p07-http"),
    ] {
        sqlx::query(
            "INSERT INTO public.users \
             (user_id, login_normalized, password_hash, global_role) \
             VALUES ($1, $2, 'not-used-by-p07-http-test', 'USER')",
        )
        .bind(id)
        .bind(login)
        .execute(primary)
        .await
        .unwrap();
    }
    sqlx::query(
        "INSERT INTO public.users \
         (user_id, login_normalized, password_hash, global_role) \
         VALUES ($1, $2, 'not-used-by-p07-http-test', 'USER')",
    )
    .bind(OTHER_KEEPER_ID)
    .bind("keeper-p07-http-other")
    .execute(primary)
    .await
    .unwrap();

    repository
        .create_campaign(
            &seed_metadata(
                KEEPER_ID,
                "human_keeper",
                CAMPAIGN_ID,
                "campaign",
                0,
                "p07_http_campaign",
                "party_visible",
                "not_applicable",
                "human_keeper_statement",
            ),
            &CreateCampaignRequest {
                campaign_id: CAMPAIGN_ID.to_owned(),
                owner_user_id: KEEPER_ID.to_owned(),
                title: "P07 HTTP tutorial".to_owned(),
                room_id: "room_p07_http".to_owned(),
                room_name: "P07 HTTP table".to_owned(),
                created_at_unix_ms: NOW_MS,
                authority: authority_snapshot(),
            },
        )
        .await
        .unwrap();
    let invite = repository
        .issue_invite(
            &seed_metadata(
                KEEPER_ID,
                "human_keeper",
                "invite_p07_http",
                "campaign_invite",
                0,
                "p07_http_invite",
                "private_to_player",
                PLAYER_ID,
                "human_keeper_statement",
            ),
            &IssueInviteRequest {
                invite_id: "invite_p07_http".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                invited_user_id: PLAYER_ID.to_owned(),
                role: MembershipRole::Player,
                expires_at_unix_ms: NOW_MS + 60_000,
            },
        )
        .await
        .unwrap();
    repository
        .accept_invite(
            &seed_metadata(
                PLAYER_ID,
                "investigator",
                "invite_p07_http",
                "campaign_invite",
                1,
                "p07_http_accept",
                "private_to_player",
                PLAYER_ID,
                "user_statement",
            ),
            &AcceptInviteRequest {
                campaign_id: CAMPAIGN_ID.to_owned(),
                invite_id: "invite_p07_http".to_owned(),
                accepting_user_id: PLAYER_ID.to_owned(),
                raw_token: invite.raw_token,
            },
        )
        .await
        .unwrap();
    repository
        .create_character(
            &seed_metadata(
                PLAYER_ID,
                "investigator",
                CHARACTER_ID,
                "character",
                0,
                "p07_http_character",
                "private_to_player",
                PLAYER_ID,
                "user_statement",
            ),
            &CreateCharacterRequest {
                character_id: CHARACTER_ID.to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                owner_user_id: PLAYER_ID.to_owned(),
                display_name: "Evelyn Hart".to_owned(),
                sheet_version_id: "sheet_p07_http_v1".to_owned(),
                sheet_json: character_sheet(),
            },
        )
        .await
        .unwrap();
    repository
        .submit_character(
            &seed_metadata(
                PLAYER_ID,
                "investigator",
                CHARACTER_ID,
                "character",
                1,
                "p07_http_character_submit",
                "private_to_player",
                PLAYER_ID,
                "user_statement",
            ),
            CAMPAIGN_ID,
            CHARACTER_ID,
        )
        .await
        .unwrap();
    repository
        .approve_character_initial_version(
            &seed_metadata(
                KEEPER_ID,
                "human_keeper",
                CHARACTER_ID,
                "character",
                2,
                "p07_http_character_approve",
                "private_to_player",
                PLAYER_ID,
                "human_keeper_statement",
            ),
            CAMPAIGN_ID,
            CHARACTER_ID,
        )
        .await
        .unwrap();
    let tutorial = parse_scenario_yaml(include_str!(
        "../../../fixtures/scenarios/tutorial_mist_archive.scenario.yaml"
    ))
    .unwrap();
    repository
        .import_scenario(
            &seed_metadata(
                KEEPER_ID,
                "human_keeper",
                "scenario_p07_http",
                "scenario",
                0,
                "p07_http_scenario",
                "keeper_only",
                "not_applicable",
                "imported_source",
            ),
            &ImportScenarioRequest {
                scenario_id: "scenario_p07_http".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                ruleset_id: tutorial.ruleset_id,
                format_version: tutorial.format_version,
                content_hash: tutorial.content_hash,
                document_json: tutorial.canonical_json,
            },
        )
        .await
        .unwrap();
    repository
        .start_session(
            &seed_metadata(
                KEEPER_ID,
                "human_keeper",
                "session_p07_http",
                "session",
                0,
                "p07_http_session",
                "party_visible",
                "not_applicable",
                "human_keeper_statement",
            ),
            &StartSessionRequest {
                session_id: "session_p07_http".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                room_id: "room_p07_http".to_owned(),
                scenario_id: "scenario_p07_http".to_owned(),
                scene_id: "scene_p07_http".to_owned(),
                scene_key: "scene_archive_front".to_owned(),
                scene_name: "灰港市政档案室前厅".to_owned(),
                started_at_unix_ms: NOW_MS + 2_000,
            },
        )
        .await
        .unwrap();
}

include!("player_action_http_integration/01_module_prelude.rs");
include!("player_action_http_integration/02_http_player_action_application_new.rs");
include!("player_action_http_integration/03_command.rs");
