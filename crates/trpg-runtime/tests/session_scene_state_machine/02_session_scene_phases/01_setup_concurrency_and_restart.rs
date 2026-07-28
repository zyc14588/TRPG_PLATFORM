{
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
        "p06-runtime-integrity-key",
        INTEGRITY_KEY,
        "p06-runtime-payload-key",
        PAYLOAD_KEY,
    )
    .await
    .expect("connect canonical Event Store and independent witness");
    store
        .prepare_for_service()
        .await
        .expect("migrate P06 runtime database");
    let repository = CoreDomainRepository::new(primary.clone(), store);
    sqlx::query(
        r#"
        INSERT INTO public.users (
            user_id, login_normalized, password_hash, global_role
        ) VALUES ($1, 'keeper-p06-runtime', 'not-used-by-runtime-test', 'USER')
        "#,
    )
    .bind(KEEPER_ID)
    .execute(&primary)
    .await
    .unwrap();
    repository
        .create_campaign(
            &metadata(
                CAMPAIGN_ID,
                "campaign",
                "campaign.create",
                0,
                "runtime_campaign_create",
            ),
            &CreateCampaignRequest {
                campaign_id: CAMPAIGN_ID.to_owned(),
                owner_user_id: KEEPER_ID.to_owned(),
                title: "P06 Runtime Campaign".to_owned(),
                room_id: ROOM_ID.to_owned(),
                room_name: "Runtime table".to_owned(),
                created_at_unix_ms: NOW_MS,
                authority: AuthorityContractSnapshot {
                    contract_id: AUTHORITY_ID.to_owned(),
                    authority_mode: "HUMAN_KP".to_owned(),
                    authority_owner: KEEPER_ID.to_owned(),
                    ruleset_version: "coc7-1".to_owned(),
                    house_rules_version: "none-1".to_owned(),
                    scenario_version: "tutorial-0.1.0".to_owned(),
                    prompt_version: "p06-1".to_owned(),
                    agent_pack_version: "none-1".to_owned(),
                    tool_schema_version: "tools-1".to_owned(),
                    safety_profile_version: "safety-1".to_owned(),
                    ai_provider_snapshot: "not_applicable".to_owned(),
                    model_route_snapshot: "not_applicable".to_owned(),
                    character_sheet_template_version: "coc7-sheet-1".to_owned(),
                },
            },
        )
        .await
        .expect("create governed runtime campaign");
    let scenario = parse_scenario_yaml(include_str!(
        "../../../../../fixtures/scenarios/tutorial_mist_archive.scenario.yaml"
    ))
    .unwrap();
    repository
        .import_scenario(
            &metadata(
                SCENARIO_ID,
                "scenario",
                "scenario.import",
                0,
                "runtime_scenario_import",
            ),
            &ImportScenarioRequest {
                scenario_id: SCENARIO_ID.to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                ruleset_id: scenario.ruleset_id,
                format_version: scenario.format_version,
                content_hash: scenario.content_hash,
                document_json: scenario.canonical_json,
            },
        )
        .await
        .expect("import validated runtime scenario");

    let repository_a = repository.clone();
    let repository_b = repository.clone();
    let metadata_a = metadata(
        "session_p06_runtime_a",
        "session",
        "session.start",
        0,
        "concurrent_start_a",
    );
    let metadata_b = metadata(
        "session_p06_runtime_b",
        "session",
        "session.start",
        0,
        "concurrent_start_b",
    );
    let request_a = session_request("session_p06_runtime_a", "scene_p06_runtime_a", 10);
    let request_b = session_request("session_p06_runtime_b", "scene_p06_runtime_b", 11);
    let (start_a, start_b) = tokio::join!(
        repository_a.start_session(&metadata_a, &request_a),
        repository_b.start_session(&metadata_b, &request_b)
    );
    assert_ne!(
        start_a.is_ok(),
        start_b.is_ok(),
        "exactly one concurrent start may win the room lock"
    );
    let loser = if start_a.is_err() { start_a } else { start_b };
    assert!(matches!(
        loser,
        Err(CoreDomainRepositoryError::ConcurrentStart)
    ));
    let live = sqlx::query(
        r#"
        SELECT session_id, active_scene_id, version
          FROM core_domain.sessions
         WHERE campaign_id = $1 AND state = 'ACTIVE'
        "#,
    )
    .bind(CAMPAIGN_ID)
    .fetch_one(&primary)
    .await
    .expect("one durable active session");
    let winning_session_id: String = live.get("session_id");
    let winning_scene_id: String = live.get::<Option<String>, _>("active_scene_id").unwrap();
    assert_eq!(live.get::<i64, _>("version"), 1);
    let started_event_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM public.event_store \
         WHERE campaign_id = $1 AND event_type = 'SessionStarted'",
    )
    .bind(CAMPAIGN_ID)
    .fetch_one(&primary)
    .await
    .unwrap();
    assert_eq!(
        started_event_count, 1,
        "losing concurrent start must not append a canonical event"
    );

    let duplicate = repository
        .start_session(
            &metadata(
                &winning_session_id,
                "session",
                "session.start",
                0,
                "duplicate_start",
            ),
            &session_request(&winning_session_id, "scene_duplicate_start", 12),
        )
        .await;
    assert!(matches!(
        duplicate,
        Err(CoreDomainRepositoryError::ConcurrentStart)
    ));

    let restarted_store = PostgresCanonicalStore::connect(
        &primary_url,
        &witness_url,
        "p06-runtime-integrity-key",
        INTEGRITY_KEY,
        "p06-runtime-payload-key",
        PAYLOAD_KEY,
    )
    .await
    .expect("reconnect after simulated process restart");
    restarted_store
        .prepare_for_service()
        .await
        .expect("reconcile canonical store after restart");
    let restarted_repository = CoreDomainRepository::new(primary.clone(), restarted_store);
    let recovery = restarted_repository
        .rebuild_session_scene_projection(CAMPAIGN_ID)
        .await
        .expect("rebuild active Session/Scene projection after restart");
    assert_eq!(recovery.restored_sessions, 1);
    assert_eq!(recovery.restored_scenes, 1);
    let recovered = sqlx::query(
        r#"
        SELECT session.state, session.version, session.active_scene_id,
               scene.scene_key, scene.state AS scene_state
          FROM core_domain.sessions AS session
          JOIN public.scenes AS scene
            ON scene.scene_id = session.active_scene_id
         WHERE session.session_id = $1
        "#,
    )
    .bind(&winning_session_id)
    .fetch_one(&primary)
    .await
    .unwrap();
    assert_eq!(recovered.get::<String, _>("state"), "ACTIVE");
    assert_eq!(recovered.get::<String, _>("scene_state"), "ACTIVE");
    assert_eq!(
        recovered.get::<Option<String>, _>("active_scene_id"),
        Some(winning_scene_id.clone())
    );
    let recovered_scene_key: String = recovered.get("scene_key");
    let mut machine = SessionSceneStateMachine::recover(
        winning_session_id.clone(),
        DurableSessionState::Active,
        Some((
            winning_scene_id.clone(),
            recovered_scene_key,
            DurableSceneState::Active,
        )),
        Vec::new(),
        1,
    )
    .expect("recover runtime state machine from durable projection");
    assert!(matches!(
        machine.resume(),
        Err(SessionSceneStateError::InvalidTransition { .. })
    ));
    let event_count_before_illegal: i64 =
        sqlx::query_scalar("SELECT count(*) FROM public.event_store WHERE campaign_id = $1")
            .bind(CAMPAIGN_ID)
            .fetch_one(&primary)
            .await
            .unwrap();
    assert!(restarted_repository
        .change_session_state(
            &metadata(
                &winning_session_id,
                "session",
                "session.resume",
                1,
                "illegal_resume",
            ),
            CAMPAIGN_ID,
            &winning_session_id,
            SessionState::Active,
            NOW_MS + 20,
        )
        .await
        .is_err());
    let event_count_after_illegal: i64 =
        sqlx::query_scalar("SELECT count(*) FROM public.event_store WHERE campaign_id = $1")
            .bind(CAMPAIGN_ID)
            .fetch_one(&primary)
            .await
            .unwrap();
    assert_eq!(event_count_after_illegal, event_count_before_illegal);

    let switch_metadata = metadata(
        &winning_session_id,
        "session",
        "scene.switch",
        1,
        "runtime_scene_switch",
    );
    let switch_request = SwitchSceneRequest {
        session_id: winning_session_id.clone(),
        campaign_id: CAMPAIGN_ID.to_owned(),
        next_scene_id: "scene_p06_runtime_next".to_owned(),
        next_scene_key: "scene_basement".to_owned(),
        next_scene_name: "地下盐窖".to_owned(),
        switched_at_unix_ms: NOW_MS + 21,
    };
    let switched = restarted_repository
        .switch_scene(&switch_metadata, &switch_request)
        .await
        .expect("switch scene through canonical repository");
    let switched_retry = restarted_repository
        .switch_scene(&switch_metadata, &switch_request)
        .await
        .expect("exact scene switch retry is idempotent");
    assert_eq!(
        switched_retry.last_event_sequence,
        switched.last_event_sequence
    );
    machine
        .switch_scene("scene_p06_runtime_next", "scene_basement")
        .unwrap();
    assert_eq!(machine.version(), 2);

    let pause_metadata = metadata(
        &winning_session_id,
        "session",
        "session.pause",
        2,
        "runtime_pause",
    );
    let paused = restarted_repository
        .change_session_state(
            &pause_metadata,
            CAMPAIGN_ID,
            &winning_session_id,
            SessionState::Paused,
            NOW_MS + 22,
        )
        .await
        .expect("pause active session");
    let paused_retry = restarted_repository
        .change_session_state(
            &pause_metadata,
            CAMPAIGN_ID,
            &winning_session_id,
            SessionState::Paused,
            NOW_MS + 22,
        )
        .await
        .expect("exact pause retry is idempotent");
    assert_eq!(paused_retry.last_event_sequence, paused.last_event_sequence);
    machine.pause().unwrap();
    assert!(machine
        .switch_scene("scene_p06_runtime_illegal", "illegal_while_paused")
        .is_err());
    include!("02_state_transitions_and_recovery.rs");
}
