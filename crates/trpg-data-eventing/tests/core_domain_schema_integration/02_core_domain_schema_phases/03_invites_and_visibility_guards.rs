{
    assert!(matches!(
        repository
            .accept_invite(
                &wrong_subject_metadata,
                &AcceptInviteRequest {
                    campaign_id: CAMPAIGN_ID.to_owned(),
                    invite_id: "invite_p06_player".to_owned(),
                    accepting_user_id: OTHER_ID.to_owned(),
                    raw_token: issued.raw_token.clone(),
                },
            )
            .await,
        Err(CoreDomainRepositoryError::Domain(_))
    ));
    let accept_metadata = metadata(
        CAMPAIGN_ID,
        AUTHORITY_ID,
        PLAYER_ID,
        "investigator",
        "invite_p06_player",
        "campaign_invite",
        "campaign.invite.accept",
        1,
        "invite_accept",
        "private_to_player",
        PLAYER_ID,
        "user_statement",
    );
    let accept_request = AcceptInviteRequest {
        campaign_id: CAMPAIGN_ID.to_owned(),
        invite_id: "invite_p06_player".to_owned(),
        accepting_user_id: PLAYER_ID.to_owned(),
        raw_token: issued.raw_token,
    };
    let accepted = repository
        .accept_invite(&accept_metadata, &accept_request)
        .await
        .expect("accept valid invite into durable membership");
    clock.0.store(NOW_MS + 60_000, Ordering::SeqCst);
    let accepted_retry = repository
        .accept_invite(&accept_metadata, &accept_request)
        .await
        .expect("exact retry remains idempotent after the invite expires");
    assert_eq!(
        accepted_retry.last_event_sequence,
        accepted.last_event_sequence
    );
    clock.0.store(NOW_MS + 1_000, Ordering::SeqCst);

    let conflict_invite_id = "invite_p07_membership_conflict";
    let conflict_issue_metadata = metadata(
        CAMPAIGN_ID,
        AUTHORITY_ID,
        KEEPER_ID,
        "human_keeper",
        conflict_invite_id,
        "campaign_invite",
        "campaign.invite.issue",
        0,
        "invite_conflict_issue",
        "private_to_player",
        OTHER_ID,
        "human_keeper_statement",
    );
    let conflict_invite = repository
        .issue_invite(
            &conflict_issue_metadata,
            &IssueInviteRequest {
                invite_id: conflict_invite_id.to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                invited_user_id: OTHER_ID.to_owned(),
                role: MembershipRole::Player,
                expires_at_unix_ms: NOW_MS + 120_000,
            },
        )
        .await
        .expect("issue invite used by the atomic conflict probe");
    sqlx::query(
        r#"
        INSERT INTO public.campaign_memberships (
            campaign_id, user_id, role, granted_by, granted_at, revoked_at
        ) VALUES ($1, $2, 'SPECTATOR', $3, to_timestamp($4 / 1000.0),
                  to_timestamp($4 / 1000.0))
        "#,
    )
    .bind(CAMPAIGN_ID)
    .bind(OTHER_ID)
    .bind(KEEPER_ID)
    .bind(NOW_MS as f64)
    .execute(&primary)
    .await
    .expect("seed a revoked conflicting membership");
    let conflict_accept_metadata = metadata(
        CAMPAIGN_ID,
        AUTHORITY_ID,
        OTHER_ID,
        "investigator",
        conflict_invite_id,
        "campaign_invite",
        "campaign.invite.accept",
        1,
        "invite_conflict_accept",
        "private_to_player",
        OTHER_ID,
        "user_statement",
    );
    assert!(matches!(
        repository
            .accept_invite(
                &conflict_accept_metadata,
                &AcceptInviteRequest {
                    campaign_id: CAMPAIGN_ID.to_owned(),
                    invite_id: conflict_invite_id.to_owned(),
                    accepting_user_id: OTHER_ID.to_owned(),
                    raw_token: conflict_invite.raw_token,
                },
            )
            .await,
        Err(CoreDomainRepositoryError::Canonical(_))
    ));
    let consumed_event_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM public.event_store \
         WHERE campaign_id = $1 AND stream_id = $2 \
           AND event_type = 'CampaignInviteAccepted'",
    )
    .bind(CAMPAIGN_ID)
    .bind(conflict_invite_id)
    .fetch_one(&primary)
    .await
    .unwrap();
    let formal_commit_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM public.formal_commits WHERE commit_id = $1")
            .bind(&conflict_accept_metadata.commit_id)
            .fetch_one(&primary)
            .await
            .unwrap();
    let conflict_membership = sqlx::query(
        "SELECT role, revoked_at IS NOT NULL AS revoked \
         FROM public.campaign_memberships \
         WHERE campaign_id = $1 AND user_id = $2",
    )
    .bind(CAMPAIGN_ID)
    .bind(OTHER_ID)
    .fetch_one(&primary)
    .await
    .unwrap();
    assert_eq!(consumed_event_count, 0);
    assert_eq!(formal_commit_count, 0);
    assert_eq!(conflict_membership.get::<String, _>("role"), "SPECTATOR");
    assert!(conflict_membership.get::<bool, _>("revoked"));

    let character_metadata = metadata(
        CAMPAIGN_ID,
        AUTHORITY_ID,
        PLAYER_ID,
        "investigator",
        "character_p06_player",
        "character",
        "character.create",
        0,
        "character_create",
        "private_to_player",
        PLAYER_ID,
        "user_statement",
    );
    repository
        .create_character(
            &character_metadata,
            &CreateCharacterRequest {
                character_id: "character_p06_player".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                owner_user_id: PLAYER_ID.to_owned(),
                display_name: "Evelyn Hart".to_owned(),
                sheet_version_id: "sheet_p06_player_v1".to_owned(),
                sheet_json:
                    r#"{"name":"Evelyn Hart","age":31,"ruleset":"coc7","characteristics":{"power":65},"skills":{"Library Use":70,"Fighting (Brawl)":45,"Firearms (Handgun)":35,"Dodge":40,"First Aid":30,"Medicine":10},"combat_profile":{"dexterity":70,"skill_targets":{"melee":45,"firearm":35,"dodge":40,"first_aid":30,"medicine":10},"skill_target_sources":{"melee":"Fighting (Brawl)","firearm":"Firearms (Handgun)","dodge":"Dodge","first_aid":"First Aid","medicine":"Medicine"},"weapon_loadout":{"melee":{"weapon_id":"selected_melee_weapon","damage_formula":{"dice_count":1,"die_sides":6,"flat_bonus":1}},"firearm":{"weapon_id":"selected_firearm","damage_formula":{"dice_count":1,"die_sides":6,"flat_bonus":5}}},"current_hp":10,"max_hp":10,"armor":1,"condition":"ABLE"},"chase_profile":{"role":"QUARRY","movement_rate":8}}"#
                        .to_owned(),
            },
        )
        .await
        .expect("create character and initial sheet projection");
    let submit_metadata = metadata(
        CAMPAIGN_ID,
        AUTHORITY_ID,
        PLAYER_ID,
        "investigator",
        "character_p06_player",
        "character",
        "character.submit",
        1,
        "character_submit",
        "private_to_player",
        PLAYER_ID,
        "user_statement",
    );
    let submitted = repository
        .submit_character(&submit_metadata, CAMPAIGN_ID, "character_p06_player")
        .await
        .expect("submit character");
    let submitted_retry = repository
        .submit_character(&submit_metadata, CAMPAIGN_ID, "character_p06_player")
        .await
        .expect("exact character submission retry is idempotent");
    assert_eq!(
        submitted_retry.last_event_sequence,
        submitted.last_event_sequence
    );
    let review_metadata = metadata(
        CAMPAIGN_ID,
        AUTHORITY_ID,
        KEEPER_ID,
        "human_keeper",
        "character_p06_player",
        "character",
        "character.review_initial",
        2,
        "character_review",
        "private_to_player",
        PLAYER_ID,
        "human_keeper_statement",
    );
    let approved = repository
        .approve_character_initial_version(&review_metadata, CAMPAIGN_ID, "character_p06_player")
        .await
        .expect("approve and lock initial character version");
    let approved_retry = repository
        .approve_character_initial_version(&review_metadata, CAMPAIGN_ID, "character_p06_player")
        .await
        .expect("exact character approval retry is idempotent");
    assert_eq!(
        approved_retry.last_event_sequence,
        approved.last_event_sequence
    );
    repository
        .create_character(
            &metadata(
                CAMPAIGN_ID,
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "character_p08_keeper_private",
                "character",
                "character.create",
                0,
                "character_create_keeper_private",
                "keeper_only",
                "not_applicable",
                "human_keeper_statement",
            ),
            &CreateCharacterRequest {
                character_id: "character_p08_keeper_private".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                owner_user_id: KEEPER_ID.to_owned(),
                display_name: "P08 Keeper Private Sentinel".to_owned(),
                sheet_version_id: "sheet_p08_keeper_private_v1".to_owned(),
                sheet_json: r#"{"keeper_only_fork_sentinel":true}"#.to_owned(),
            },
        )
        .await
        .expect("seed a keeper-only character that a default fork must exclude");
    let character_state = sqlx::query(
        r#"
        SELECT character.state, character.initial_version_locked,
               sheet.locked AS sheet_locked
          FROM public.characters AS character
          JOIN public.character_sheet_versions AS sheet
            ON sheet.character_id = character.character_id
         WHERE character.character_id = 'character_p06_player'
           AND sheet.version = 1
        "#,
    )
    .fetch_one(&primary)
    .await
    .unwrap();
    assert_eq!(character_state.get::<String, _>("state"), "APPROVED");
    assert!(character_state.get::<bool, _>("initial_version_locked"));
    assert!(character_state.get::<bool, _>("sheet_locked"));

    let mut forged_projection = primary.begin().await.unwrap();
    sqlx::query("SET LOCAL ROLE trpg_api_service")
        .execute(&mut *forged_projection)
        .await
        .unwrap();
    let forged = sqlx::query(
        r#"
        INSERT INTO public.characters (
            character_id, campaign_id, owner_user_id, display_name,
            state, current_sheet_version, initial_version_locked, version,
            visibility_label, visibility_subject,
            provenance_kind, provenance_reference, provenance_recorded_by,
            last_event_sequence
        ) VALUES (
            'character_forged_by_reused_event', $1, $2, 'Forged',
            'APPROVED', 1, TRUE, 999,
            'private_to_player', $2,
            'human_keeper_statement', 'source_character_review', $3, $4
        )
        "#,
    )
    .bind(CAMPAIGN_ID)
    .bind(PLAYER_ID)
    .bind(KEEPER_ID)
    .bind(approved.last_event_sequence)
    .execute(&mut *forged_projection)
    .await;
    assert!(
        forged.is_err(),
        "a legitimate event for one Character must not authorize another projection row"
    );
    forged_projection.rollback().await.unwrap();

    let mut forbidden_delete = primary.begin().await.unwrap();
    sqlx::query("SET LOCAL ROLE trpg_api_service")
        .execute(&mut *forbidden_delete)
        .await
        .unwrap();
    let deleted =
        sqlx::query("DELETE FROM public.characters WHERE character_id = 'character_p06_player'")
            .execute(&mut *forbidden_delete)
            .await;
    assert!(
        deleted.is_err(),
        "the API projection role must not delete canonical projections"
    );
    forbidden_delete.rollback().await.unwrap();

    let tutorial = parse_scenario_yaml(include_str!(
        "../../../../../fixtures/scenarios/tutorial_mist_archive.scenario.yaml"
    ))
    .expect("parse raw Tutorial Scenario");
    include!("04_scenario_session_and_player_actions.rs");
}
