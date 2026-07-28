{
    let state_fork_metadata = metadata(
        STATE_RACE_CHILD_CAMPAIGN_ID,
        STATE_RACE_CHILD_AUTHORITY_ID,
        KEEPER_ID,
        "human_keeper",
        "fork_p08_state_race",
        "campaign_fork",
        "campaign.fork.record",
        0,
        "fork_p08_state_race",
        "keeper_only",
        "not_applicable",
        "human_keeper_statement",
    );
    let state_fork_request = RecordCampaignForkRequest {
        fork_id: "fork_p08_state_race".to_owned(),
        parent_campaign_id: CAMPAIGN_ID.to_owned(),
        child_campaign_id: STATE_RACE_CHILD_CAMPAIGN_ID.to_owned(),
        source_session_id: "session_p06_schema".to_owned(),
        snapshot_hash: snapshot.snapshot_hash.clone(),
        reason: "Race an ordinary child write against fork initialization".to_owned(),
        copy_scopes: snapshot.copy_scopes.clone(),
    };
    let state_race_lock_key = format!("p08-campaign-fork-empty:{STATE_RACE_CHILD_CAMPAIGN_ID}");
    let mut state_race_barrier = primary.begin().await.unwrap();
    let blocked_advisory_locks_before: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM pg_locks \
         WHERE locktype = 'advisory' AND NOT granted",
    )
    .fetch_one(&mut *state_race_barrier)
    .await
    .unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(&state_race_lock_key)
        .execute(&mut *state_race_barrier)
        .await
        .unwrap();

    let state_write_task = tokio::spawn(async move {
        state_write_repository
            .import_scenario(&state_write_metadata, &state_write_request)
            .await
    });
    let wait_for_blocked_locks = |expected: i64| {
        let primary = primary.clone();
        async move {
            for _ in 0..200 {
                let blocked: i64 = sqlx::query_scalar(
                    "SELECT count(*) FROM pg_locks \
                     WHERE locktype = 'advisory' AND NOT granted",
                )
                .fetch_one(&primary)
                .await
                .unwrap();
                if blocked >= expected {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            panic!("timed out waiting for {expected} blocked advisory locks");
        }
    };
    wait_for_blocked_locks(blocked_advisory_locks_before + 1).await;

    let state_fork_task = tokio::spawn(async move {
        state_fork_repository
            .record_campaign_fork(&state_fork_metadata, &state_fork_request)
            .await
    });
    wait_for_blocked_locks(blocked_advisory_locks_before + 2).await;
    state_race_barrier.commit().await.unwrap();

    let (state_write_result, state_fork_result) =
        tokio::time::timeout(Duration::from_secs(30), async {
            (
                state_write_task.await.expect("state-write task must join"),
                state_fork_task.await.expect("fork task must join"),
            )
        })
        .await
        .expect("serialized fork-versus-state race must complete");
    state_write_result.expect("the ordinary child write queued first must commit");
    assert!(
        matches!(
            &state_fork_result,
            Err(CoreDomainRepositoryError::Canonical(_))
        ),
        "a fork whose preflight raced a committed child write must be rejected by the canonical insert guard: {state_fork_result:?}"
    );
    let state_race_counts: (i64, i64, i64) = sqlx::query_as(
        r#"
        SELECT
            (SELECT count(*) FROM public.event_store
              WHERE campaign_id = $1
                AND event_type = 'ScenarioImported'),
            (SELECT count(*) FROM public.event_store
              WHERE campaign_id = $1
                AND event_type = 'CampaignForkRecorded'),
            (SELECT count(*) FROM public.scenarios
              WHERE campaign_id = $1
                AND scenario_id = 'scenario_p08_fork_state_race')
        "#,
    )
    .bind(STATE_RACE_CHILD_CAMPAIGN_ID)
    .fetch_one(&primary)
    .await
    .unwrap();
    assert_eq!(
        state_race_counts,
        (1, 0, 1),
        "canonical and projected ordinary state must win without admitting a second initialization history"
    );

    let single_connection_child = "campaign_p08_fork_single_connection";
    let single_connection_authority = "authority_contract_campaign_p08_fork_single_connection_1";
    create_campaign(
        &repository,
        single_connection_child,
        single_connection_authority,
        "room_p08_fork_single_connection",
        "fork_single_connection_child_create",
    )
    .await;
    let single_connection_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(PgConnectOptions::from_str(&primary_url).unwrap())
        .await
        .unwrap();
    let single_connection_repository = CoreDomainRepository::new_with_clock(
        single_connection_pool,
        canonical_reader.clone(),
        clock.clone(),
    );
    tokio::time::timeout(
        Duration::from_secs(30),
        single_connection_repository.record_campaign_fork(
            &metadata(
                single_connection_child,
                single_connection_authority,
                KEEPER_ID,
                "human_keeper",
                "fork_p08_single_connection",
                "campaign_fork",
                "campaign.fork.record",
                0,
                "fork_single_connection",
                "keeper_only",
                "not_applicable",
                "human_keeper_statement",
            ),
            &RecordCampaignForkRequest {
                fork_id: "fork_p08_single_connection".to_owned(),
                parent_campaign_id: CAMPAIGN_ID.to_owned(),
                child_campaign_id: single_connection_child.to_owned(),
                source_session_id: "session_p06_schema".to_owned(),
                snapshot_hash: snapshot.snapshot_hash.clone(),
                reason: "Prove fork construction never nests projection-pool leases".to_owned(),
                copy_scopes: snapshot.copy_scopes.clone(),
            },
        ),
    )
    .await
    .expect("fork must not deadlock even when the projection pool has one connection")
    .expect("single-connection fork must materialize successfully");

    let keeper_private_event_sequence: i64 = sqlx::query_scalar(
        "SELECT sequence FROM public.event_store \
         WHERE campaign_id = $1 \
           AND stream_id = 'character_p08_keeper_private' \
           AND visibility_label = 'keeper_only' \
         ORDER BY sequence LIMIT 1",
    )
    .bind(CAMPAIGN_ID)
    .fetch_one(&primary)
    .await
    .expect("load a keeper-only canonical source event");
    let reconsideration_events_before_visibility_attack: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM public.event_store \
         WHERE campaign_id = $1 AND event_type = 'ReconsiderationRequested'",
    )
    .bind(CAMPAIGN_ID)
    .fetch_one(&primary)
    .await
    .unwrap();
    assert!(matches!(
        repository
            .request_reconsideration(
                &metadata(
                    CAMPAIGN_ID,
                    AUTHORITY_ID,
                    PLAYER_ID,
                    "investigator",
                    "reconsideration_p08_hidden_source",
                    "reconsideration",
                    "reconsideration.request",
                    0,
                    "reconsideration_hidden_source_rejected",
                    "party_visible",
                    "not_applicable",
                    "user_statement",
                ),
                &RequestReconsiderationRequest {
                    reconsideration_id: "reconsideration_p08_hidden_source".to_owned(),
                    campaign_id: CAMPAIGN_ID.to_owned(),
                    original_event_sequence: keeper_private_event_sequence,
                    requested_by: PLAYER_ID.to_owned(),
                    reason: "Attempt to reveal a guessed hidden event".to_owned(),
                },
            )
            .await,
        Err(CoreDomainRepositoryError::NotFound(
            "reconsideration_source_event"
        ))
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM public.event_store \
             WHERE campaign_id = $1 AND event_type = 'ReconsiderationRequested'",
        )
        .bind(CAMPAIGN_ID)
        .fetch_one(&primary)
        .await
        .unwrap(),
        reconsideration_events_before_visibility_attack,
        "an unauthorized source sequence must not reveal itself through a formal request"
    );

    let reconsideration_request_metadata = metadata(
        CAMPAIGN_ID,
        AUTHORITY_ID,
        PLAYER_ID,
        "investigator",
        "reconsideration_p06_schema",
        "reconsideration",
        "reconsideration.request",
        0,
        "reconsideration_request",
        "party_visible",
        "not_applicable",
        "user_statement",
    );
    let reconsideration_request = RequestReconsiderationRequest {
        reconsideration_id: "reconsideration_p06_schema".to_owned(),
        campaign_id: CAMPAIGN_ID.to_owned(),
        original_event_sequence: campaign_event_sequence,
        requested_by: PLAYER_ID.to_owned(),
        reason: "Review the opening ruling".to_owned(),
    };
    let reconsideration_requested = repository
        .request_reconsideration(&reconsideration_request_metadata, &reconsideration_request)
        .await
        .expect("append reconsideration request");
    let reconsideration_retry = repository
        .request_reconsideration(&reconsideration_request_metadata, &reconsideration_request)
        .await
        .expect("exact repeated reconsideration request is idempotent");
    assert_eq!(
        reconsideration_retry.last_event_sequence,
        reconsideration_requested.last_event_sequence
    );
    let source_after_reconsideration = repository
        .preview_campaign_fork(CAMPAIGN_ID, "session_p06_schema", KEEPER_ID)
        .await
        .expect("recompute the source snapshot after a later reconsideration");
    assert_ne!(
        source_after_reconsideration.snapshot_hash, snapshot.snapshot_hash,
        "a later relevant parent event must prove that the source snapshot is mutable"
    );
    repository
        .record_campaign_fork(
            &metadata(
                CHILD_CAMPAIGN_ID,
                CHILD_AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "fork_p06_schema",
                "campaign_fork",
                "campaign.fork.record",
                0,
                "fork_record",
                "keeper_only",
                "not_applicable",
                "human_keeper_statement",
            ),
            &RecordCampaignForkRequest {
                fork_id: "fork_p06_schema".to_owned(),
                parent_campaign_id: CAMPAIGN_ID.to_owned(),
                child_campaign_id: CHILD_CAMPAIGN_ID.to_owned(),
                source_session_id: "session_p06_schema".to_owned(),
                snapshot_hash: snapshot.snapshot_hash.clone(),
                reason: "Preserve an alternate ruling".to_owned(),
                copy_scopes: snapshot.copy_scopes.clone(),
            },
        )
        .await
        .expect("an exact retry must replay the recorded fork, not the mutable parent snapshot");
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM public.event_store WHERE campaign_id = $1",
        )
        .bind(CHILD_CAMPAIGN_ID)
        .fetch_one(&primary)
        .await
        .unwrap(),
        child_events_before_fork_retry,
        "retrying after later parent activity must not append canonical child history"
    );
    assert!(matches!(
        repository
            .review_reconsideration(
                &metadata(
                    CAMPAIGN_ID,
                    AUTHORITY_ID,
                    KEEPER_ID,
                    "human_keeper",
                    "reconsideration_p06_schema",
                    "reconsideration",
                    "reconsideration.review",
                    1,
                    "reconsideration_visibility_widen_rejected",
                    "keeper_only",
                    "not_applicable",
                    "human_keeper_statement",
                ),
                &ReviewReconsiderationRequest {
                    reconsideration_id: "reconsideration_p06_schema".to_owned(),
                    campaign_id: CAMPAIGN_ID.to_owned(),
                    review_event_id: "review_event_visibility_widen_rejected".to_owned(),
                    review_summary: "Attempt to move a party chain into another scope".to_owned(),
                },
            )
            .await,
        Err(CoreDomainRepositoryError::Forbidden)
    ));
    include!("16_reconsideration_workflow.rs");
}
