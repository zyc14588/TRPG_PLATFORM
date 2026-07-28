{
    let rejected = repository
        .create_campaign(
            &direct_user_write,
            &CreateCampaignRequest {
                campaign_id: CAMPAIGN_ID.to_owned(),
                owner_user_id: KEEPER_ID.to_owned(),
                title: "Direct user write must fail".to_owned(),
                room_id: "room_direct_user_write_rejected".to_owned(),
                room_name: "Rejected room".to_owned(),
                created_at_unix_ms: NOW_MS,
                authority: authority(AUTHORITY_ID),
            },
        )
        .await;
    assert!(matches!(
        rejected,
        Err(CoreDomainRepositoryError::PolicyEvidenceMismatch)
    ));
    let events_after_direct_user_write: i64 =
        sqlx::query_scalar("SELECT count(*) FROM public.event_store")
            .fetch_one(&primary)
            .await
            .unwrap();
    assert_eq!(
        events_after_direct_user_write, 0,
        "direct business writes must not reach the canonical Event Store"
    );

    let recovery_campaign_id = "campaign_p06_projection_recovery";
    let recovery_authority_id = "authority_campaign_p06_projection_recovery_1";
    let recovery_metadata = campaign_metadata(
        recovery_campaign_id,
        recovery_authority_id,
        "campaign_projection_recovery",
    );
    let recovery_request = CreateCampaignRequest {
        campaign_id: recovery_campaign_id.to_owned(),
        owner_user_id: KEEPER_ID.to_owned(),
        title: "Recoverable projection Campaign".to_owned(),
        room_id: "room_p06_projection_recovery".to_owned(),
        room_name: "Recovery table".to_owned(),
        created_at_unix_ms: NOW_MS,
        authority: authority(recovery_authority_id),
    };
    sqlx::raw_sql(
        r#"
        CREATE FUNCTION public.reject_p06_projection_for_test()
        RETURNS trigger
        LANGUAGE plpgsql
        AS $$
        BEGIN
            RAISE EXCEPTION 'injected projection failure';
        END;
        $$;
        CREATE TRIGGER zz_reject_p06_projection_for_test
        BEFORE INSERT ON public.campaigns
        FOR EACH ROW EXECUTE FUNCTION public.reject_p06_projection_for_test();
        "#,
    )
    .execute(&primary)
    .await
    .expect("install session-local projection failure injection");
    assert!(matches!(
        repository
            .create_campaign(&recovery_metadata, &recovery_request)
            .await,
        Err(CoreDomainRepositoryError::Database("insert_campaign"))
    ));
    let recovery_event_count_after_failure: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM public.event_store \
         WHERE campaign_id = $1 AND event_type = 'CampaignCreated'",
    )
    .bind(recovery_campaign_id)
    .fetch_one(&primary)
    .await
    .unwrap();
    let recovery_projection_count_after_failure: i64 =
        sqlx::query_scalar("SELECT count(*) FROM public.campaigns WHERE campaign_id = $1")
            .bind(recovery_campaign_id)
            .fetch_one(&primary)
            .await
            .unwrap();
    assert_eq!(recovery_event_count_after_failure, 1);
    assert_eq!(
        recovery_projection_count_after_failure, 0,
        "a failed projection transaction must not leave partial business rows"
    );
    sqlx::raw_sql(
        r#"
        DROP TRIGGER zz_reject_p06_projection_for_test ON public.campaigns;
        DROP FUNCTION public.reject_p06_projection_for_test();
        "#,
    )
    .execute(&primary)
    .await
    .expect("remove session-local projection failure injection");

    let recovery_event_sequence: i64 = sqlx::query_scalar(
        "SELECT sequence FROM public.event_store \
         WHERE campaign_id = $1 AND event_type = 'CampaignCreated'",
    )
    .bind(recovery_campaign_id)
    .fetch_one(&primary)
    .await
    .unwrap();
    let mut exact_target_forgery = primary.begin().await.unwrap();
    sqlx::query("SET LOCAL ROLE trpg_api_service")
        .execute(&mut *exact_target_forgery)
        .await
        .unwrap();
    sqlx::query(
        r#"
        INSERT INTO public.campaign_memberships (
            campaign_id, user_id, role, granted_by, granted_at
        ) VALUES ($1, $2, 'HUMAN_KEEPER', $2, to_timestamp($3::double precision / 1000.0))
        "#,
    )
    .bind(recovery_campaign_id)
    .bind(KEEPER_ID)
    .bind(NOW_MS as i64)
    .execute(&mut *exact_target_forgery)
    .await
    .expect("seed attacker-controlled authority prerequisite inside rollback");
    sqlx::query(
        r#"
        INSERT INTO public.authority_contracts (
            contract_id, campaign_id, authority_mode, authority_owner,
            contract_version, ruleset_version, house_rules_version,
            scenario_version, prompt_version, agent_pack_version,
            tool_schema_version, safety_profile_version,
            ai_provider_snapshot, model_route_snapshot,
            character_sheet_template_version, created_at, locked, change_policy
        ) VALUES (
            $1, $2, 'HUMAN_KP', $3, 1, 'coc7-rules-1', 'house-rules-1',
            'scenario-1', 'prompt-1', 'agent-pack-1', 'tool-schema-1',
            'safety-1', 'not_applicable', 'not_applicable', 'coc7-sheet-1',
            to_timestamp($4::double precision / 1000.0), TRUE, 'FORK_ONLY'
        )
        "#,
    )
    .bind(recovery_authority_id)
    .bind(recovery_campaign_id)
    .bind(KEEPER_ID)
    .bind(NOW_MS as i64)
    .execute(&mut *exact_target_forgery)
    .await
    .expect("seed attacker-controlled authority inside rollback");
    let forged_exact_target = sqlx::query(
        r#"
        INSERT INTO public.campaigns (
            campaign_id, owner_user_id, authority_contract_id, title,
            state, version, created_at,
            visibility_label, visibility_subject,
            provenance_kind, provenance_reference, provenance_recorded_by,
            last_event_sequence
        ) VALUES (
            $1, $2, $3, 'Forged exact-target contents', 'ACTIVE', 999,
            to_timestamp($4::double precision / 1000.0),
            'party_visible', 'not_applicable',
            'human_keeper_statement', 'source_campaign_projection_recovery',
            $2, $5
        )
        "#,
    )
    .bind(recovery_campaign_id)
    .bind(KEEPER_ID)
    .bind(recovery_authority_id)
    .bind(NOW_MS as i64)
    .bind(recovery_event_sequence)
    .execute(&mut *exact_target_forgery)
    .await;
    assert!(
        forged_exact_target.is_err(),
        "knowing an exact event target must not let the API database role consume its secret capability"
    );
    exact_target_forgery.rollback().await.unwrap();

    let recovered_projection = repository
        .create_campaign(&recovery_metadata, &recovery_request)
        .await
        .expect("exact retry recovers the projection from the durable canonical event");
    assert_eq!(
        recovered_projection.last_event_sequence,
        sqlx::query_scalar::<_, i64>(
            "SELECT last_event_sequence FROM public.campaigns WHERE campaign_id = $1",
        )
        .bind(recovery_campaign_id)
        .fetch_one(&primary)
        .await
        .unwrap()
    );
    let recovery_event_count_after_retry: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM public.event_store \
         WHERE campaign_id = $1 AND event_type = 'CampaignCreated'",
    )
    .bind(recovery_campaign_id)
    .fetch_one(&primary)
    .await
    .unwrap();
    assert_eq!(
        recovery_event_count_after_retry, 1,
        "projection recovery must reuse, not duplicate, the canonical event"
    );

    let campaign_event_sequence = create_campaign(
        &repository,
        CAMPAIGN_ID,
        AUTHORITY_ID,
        "room_p06_schema",
        "campaign_create",
    )
    .await;
    let authority_row = sqlx::query(
        r#"
        SELECT authority_mode, authority_owner, locked, change_policy
          FROM public.authority_contracts
         WHERE campaign_id = $1
        "#,
    )
    .bind(CAMPAIGN_ID)
    .fetch_one(&primary)
    .await
    .expect("load locked campaign authority");
    assert_eq!(authority_row.get::<String, _>("authority_mode"), "HUMAN_KP");
    assert_eq!(authority_row.get::<String, _>("authority_owner"), KEEPER_ID);
    assert!(authority_row.get::<bool, _>("locked"));
    assert_eq!(authority_row.get::<String, _>("change_policy"), "FORK_ONLY");
    assert!(
        sqlx::query(
            "UPDATE public.authority_contracts SET authority_owner = $1 WHERE campaign_id = $2"
        )
        .bind(OTHER_ID)
        .bind(CAMPAIGN_ID)
        .execute(&primary)
        .await
        .is_err(),
        "locked Authority Contract mutation must fail in PostgreSQL"
    );
    let protected_payload: bool = sqlx::query_scalar(
        "SELECT payload_json ? 'protected_payload' FROM public.event_store WHERE sequence = $1",
    )
    .bind(campaign_event_sequence)
    .fetch_one(&primary)
    .await
    .unwrap();
    assert!(
        protected_payload,
        "canonical event payload must be encrypted"
    );

    let invite_metadata = metadata(
        CAMPAIGN_ID,
        AUTHORITY_ID,
        KEEPER_ID,
        "human_keeper",
        "invite_p06_player",
        "campaign_invite",
        "campaign.invite.issue",
        0,
        "invite_issue",
        "private_to_player",
        PLAYER_ID,
        "human_keeper_statement",
    );
    let invite_request = IssueInviteRequest {
        invite_id: "invite_p06_player".to_owned(),
        campaign_id: CAMPAIGN_ID.to_owned(),
        invited_user_id: PLAYER_ID.to_owned(),
        role: MembershipRole::Player,
        expires_at_unix_ms: NOW_MS + 60_000,
    };
    let issued = repository
        .issue_invite(&invite_metadata, &invite_request)
        .await
        .expect("issue non-persisted raw-token invitation");
    let issued_retry = repository
        .issue_invite(&invite_metadata, &invite_request)
        .await
        .expect("exact invite retry is idempotent");
    assert_eq!(issued_retry.raw_token, issued.raw_token);
    assert_eq!(
        issued_retry.persisted.last_event_sequence,
        issued.persisted.last_event_sequence
    );
    let invite_issue_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM public.event_store \
         WHERE campaign_id = $1 AND event_type = 'CampaignInviteIssued'",
    )
    .bind(CAMPAIGN_ID)
    .fetch_one(&primary)
    .await
    .unwrap();
    assert_eq!(
        invite_issue_count, 1,
        "invite retry must not append an event"
    );
    let expired_metadata = metadata(
        CAMPAIGN_ID,
        AUTHORITY_ID,
        PLAYER_ID,
        "investigator",
        "invite_p06_player",
        "campaign_invite",
        "campaign.invite.accept",
        1,
        "invite_accept_expired",
        "private_to_player",
        PLAYER_ID,
        "user_statement",
    );
    clock.0.store(NOW_MS + 60_000, Ordering::SeqCst);
    assert!(matches!(
        repository
            .accept_invite(
                &expired_metadata,
                &AcceptInviteRequest {
                    campaign_id: CAMPAIGN_ID.to_owned(),
                    invite_id: "invite_p06_player".to_owned(),
                    accepting_user_id: PLAYER_ID.to_owned(),
                    raw_token: issued.raw_token.clone(),
                },
            )
            .await,
        Err(CoreDomainRepositoryError::Domain(_))
    ));
    clock.0.store(NOW_MS + 1_000, Ordering::SeqCst);
    let wrong_subject_metadata = metadata(
        CAMPAIGN_ID,
        AUTHORITY_ID,
        OTHER_ID,
        "investigator",
        "invite_p06_player",
        "campaign_invite",
        "campaign.invite.accept",
        1,
        "invite_accept_wrong_subject",
        "private_to_player",
        OTHER_ID,
        "user_statement",
    );
    include!("03_invites_and_visibility_guards.rs");
}
