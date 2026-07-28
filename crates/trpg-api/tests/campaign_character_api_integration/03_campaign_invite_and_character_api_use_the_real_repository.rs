
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn campaign_invite_and_character_api_use_the_real_repository() {
    let primary_url =
        env::var("P06_DATABASE_URL").expect("P06_DATABASE_URL is required for the real DB gate");
    let witness_url = env::var("P06_WITNESS_DATABASE_URL")
        .expect("P06_WITNESS_DATABASE_URL is required for the independent witness gate");
    let primary_database = env::var("P06_RESET_DATABASE").unwrap();
    let witness_database = env::var("P06_WITNESS_RESET_DATABASE").unwrap();
    let primary = reset_database(&primary_url, &primary_database, false).await;
    let witness = reset_database(&witness_url, &witness_database, true).await;
    witness.close().await;
    let store = PostgresCanonicalStore::connect(
        &primary_url,
        &witness_url,
        "p06-api-integrity-key",
        INTEGRITY_KEY,
        "p06-api-payload-key",
        PAYLOAD_KEY,
    )
    .await
    .expect("connect canonical Event Store and independent witness");
    store
        .prepare_for_service()
        .await
        .expect("migrate P06 API database");
    let clock = Arc::new(TestClock(AtomicU64::new(NOW_MS)));
    let repository = CoreDomainRepository::new_with_clock(primary.clone(), store, clock.clone());
    for (user_id, login) in [(KEEPER_ID, "keeper-p06-api"), (PLAYER_ID, "player-p06-api")] {
        sqlx::query(
            r#"
            INSERT INTO public.users (
                user_id, login_normalized, password_hash, global_role
            ) VALUES ($1, $2, 'not-used-by-api-test', 'USER')
            "#,
        )
        .bind(user_id)
        .bind(login)
        .execute(&primary)
        .await
        .unwrap();
    }
    let api = CampaignCharacterApi::new(Arc::new(RepositoryCampaignCharacterPort { repository }));
    let mut decisions = RealFormalDecisionIssuer::new();
    let keeper = decisions.context(
        KEEPER_ID,
        "campaign",
        CAMPAIGN_ID,
        "party_visible",
        None,
        "keeper_create",
    );
    let campaign_request = CreateCampaignApiRequest {
        command: command("api_campaign_create", 0),
        campaign_id: CAMPAIGN_ID.to_owned(),
        owner_user_id: KEEPER_ID.to_owned(),
        title: "P06 API Campaign".to_owned(),
        room_id: "room_p06_api".to_owned(),
        room_name: "API table".to_owned(),
        created_at_unix_ms: NOW_MS,
        authority: AuthoritySnapshotApiRequest {
            contract_id: AUTHORITY_ID.to_owned(),
            authority_mode: "HUMAN_KP".to_owned(),
            authority_owner: KEEPER_ID.to_owned(),
            ruleset_version: "coc7_rules_1".to_owned(),
            house_rules_version: "house_rules_1".to_owned(),
            scenario_version: "scenario_1".to_owned(),
            prompt_version: "prompt_1".to_owned(),
            agent_pack_version: "agent_pack_1".to_owned(),
            tool_schema_version: "tool_schema_1".to_owned(),
            safety_profile_version: "safety_profile_1".to_owned(),
            ai_provider_snapshot: "provider_snapshot_1".to_owned(),
            model_route_snapshot: "model_route_1".to_owned(),
            character_sheet_template_version: "character_template_1".to_owned(),
        },
    };
    let campaign_receipt = api
        .create_campaign(&keeper, &campaign_request)
        .await
        .expect("API creates campaign and locked authority");
    assert_eq!(campaign_receipt.aggregate_version, 1);

    let player_before_membership = decisions.context(
        PLAYER_ID,
        "campaign",
        CAMPAIGN_ID,
        "party_visible",
        None,
        "player_denied_create",
    );
    let mut forbidden_campaign = campaign_request.clone();
    forbidden_campaign.owner_user_id = PLAYER_ID.to_owned();
    assert!(matches!(
        api.create_campaign(&player_before_membership, &forbidden_campaign)
            .await,
        Err(CoreApiError::Forbidden)
    ));
    let mut mismatched_campaign = campaign_request.clone();
    mismatched_campaign.campaign_id = "campaign_p06_api_mismatched".to_owned();
    assert!(matches!(
        api.create_campaign(&keeper, &mismatched_campaign).await,
        Err(CoreApiError::InvalidAuthorizationContext)
    ));

    let invite_id = "invite_p06_api_player";
    let invite_issue_context = decisions.context(
        KEEPER_ID,
        "campaign_invite",
        invite_id,
        "private_to_player",
        Some(PLAYER_ID),
        "invite_issue",
    );
    let issued = api
        .issue_invite(
            &invite_issue_context,
            &IssueInviteApiRequest {
                command: command("api_invite_issue", 0),
                campaign_id: CAMPAIGN_ID.to_owned(),
                invite_id: invite_id.to_owned(),
                invited_user_id: PLAYER_ID.to_owned(),
                role: "PLAYER".to_owned(),
                expires_at_unix_ms: NOW_MS + 60_000,
            },
        )
        .await
        .expect("API issues invitation");
    let expired_invite_context = decisions.context(
        PLAYER_ID,
        "campaign_invite",
        invite_id,
        "private_to_player",
        Some(PLAYER_ID),
        "player_expired_accept",
    );
    clock.0.store(NOW_MS + 60_000, Ordering::SeqCst);
    assert!(api
        .accept_invite(
            &expired_invite_context,
            &AcceptInviteApiRequest {
                command: command("api_invite_expired", 1),
                campaign_id: CAMPAIGN_ID.to_owned(),
                invite_id: issued.invite_id.clone(),
                accepting_user_id: PLAYER_ID.to_owned(),
                raw_token: issued.raw_token.clone(),
            },
        )
        .await
        .is_err());
    clock.0.store(NOW_MS + 1_000, Ordering::SeqCst);
    let valid_invite_context = decisions.context(
        PLAYER_ID,
        "campaign_invite",
        invite_id,
        "private_to_player",
        Some(PLAYER_ID),
        "player_valid_accept",
    );
    api.accept_invite(
        &valid_invite_context,
        &AcceptInviteApiRequest {
            command: command("api_invite_accept", 1),
            campaign_id: CAMPAIGN_ID.to_owned(),
            invite_id: issued.invite_id,
            accepting_user_id: PLAYER_ID.to_owned(),
            raw_token: issued.raw_token,
        },
    )
    .await
    .expect("API accepts valid invite into real membership");

    let invalid_character_context = decisions.context(
        PLAYER_ID,
        "character",
        "character_p06_api_invalid",
        "private_to_player",
        Some(PLAYER_ID),
        "invalid_character",
    );
    let invalid_sheet_events_before: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM public.event_store WHERE event_type = 'CharacterCreated'",
    )
    .fetch_one(&primary)
    .await
    .unwrap();
    let invalid_sheet = api
        .create_character(
            &invalid_character_context,
            &CreateCharacterApiRequest {
                command: command("api_character_invalid", 0),
                campaign_id: CAMPAIGN_ID.to_owned(),
                character_id: "character_p06_api_invalid".to_owned(),
                owner_user_id: PLAYER_ID.to_owned(),
                display_name: "Invalid".to_owned(),
                sheet_version_id: "sheet_p06_api_invalid_v1".to_owned(),
                sheet_json: r#"{"name":"Invalid","age":12}"#.to_owned(),
            },
        )
        .await;
    assert!(matches!(
        invalid_sheet,
        Err(CoreApiError::InvalidInput("coc7_character_sheet"))
    ));
    let invalid_sheet_events_after: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM public.event_store WHERE event_type = 'CharacterCreated'",
    )
    .fetch_one(&primary)
    .await
    .unwrap();
    assert_eq!(invalid_sheet_events_after, invalid_sheet_events_before);

    let character_id = "character_p06_api_player";
    let create_character_context = decisions.context(
        PLAYER_ID,
        "character",
        character_id,
        "private_to_player",
        Some(PLAYER_ID),
        "character_create",
    );
    api.create_character(
        &create_character_context,
        &CreateCharacterApiRequest {
            command: command("api_character_create", 0),
            campaign_id: CAMPAIGN_ID.to_owned(),
            character_id: character_id.to_owned(),
            owner_user_id: PLAYER_ID.to_owned(),
            display_name: "Evelyn Hart".to_owned(),
            sheet_version_id: "sheet_p06_api_player_v1".to_owned(),
            sheet_json: valid_sheet_json(),
        },
    )
    .await
    .expect("API validates and persists COC7 character");
    let submit_character_context = decisions.context(
        PLAYER_ID,
        "character",
        character_id,
        "private_to_player",
        Some(PLAYER_ID),
        "character_submit",
    );
    api.submit_character(
        &submit_character_context,
        &CharacterTransitionApiRequest {
            command: command("api_character_submit", 1),
            campaign_id: CAMPAIGN_ID.to_owned(),
            character_id: character_id.to_owned(),
        },
    )
    .await
    .expect("API submits character");
    let player_review_context = decisions.context(
        PLAYER_ID,
        "character",
        character_id,
        "private_to_player",
        Some(PLAYER_ID),
        "player_review",
    );
    assert!(matches!(
        api.review_character(
            &player_review_context,
            &CharacterTransitionApiRequest {
                command: command("api_character_player_review", 2),
                campaign_id: CAMPAIGN_ID.to_owned(),
                character_id: character_id.to_owned(),
            },
        )
        .await,
        Err(CoreApiError::Forbidden)
    ));
    let keeper_review_context = decisions.context(
        KEEPER_ID,
        "character",
        character_id,
        "private_to_player",
        Some(PLAYER_ID),
        "keeper_review",
    );
    api.review_character(
        &keeper_review_context,
        &CharacterTransitionApiRequest {
            command: command("api_character_review", 2),
            campaign_id: CAMPAIGN_ID.to_owned(),
            character_id: character_id.to_owned(),
        },
    )
    .await
    .expect("keeper reviews and locks initial character version");

    let stored = sqlx::query(
        r#"
        SELECT character.state, character.initial_version_locked,
               sheet.locked AS sheet_locked,
               event.visibility_label,
               event.visibility_subject,
               event.payload_json ? 'protected_payload' AS encrypted
          FROM public.characters AS character
          JOIN public.character_sheet_versions AS sheet
            ON sheet.character_id = character.character_id
          JOIN public.event_store AS event
            ON event.sequence = character.last_event_sequence
         WHERE character.character_id = 'character_p06_api_player'
        "#,
    )
    .fetch_one(&primary)
    .await
    .unwrap();
    assert_eq!(stored.get::<String, _>("state"), "APPROVED");
    assert!(stored.get::<bool, _>("initial_version_locked"));
    assert!(stored.get::<bool, _>("sheet_locked"));
    assert_eq!(
        stored.get::<String, _>("visibility_label"),
        "private_to_player"
    );
    assert_eq!(stored.get::<String, _>("visibility_subject"), PLAYER_ID);
    assert!(stored.get::<bool, _>("encrypted"));
    decisions.verify_and_cleanup();
}
