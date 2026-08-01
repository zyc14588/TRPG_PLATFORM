#[tokio::test(flavor = "multi_thread")]
async fn agent_job_cas_lease_recovery_and_evidence_are_durable() {
    let fixture_database_url = env::var("AR09_AGENT_JOB_FIXTURE_DATABASE_URL")
        .expect("AR09_AGENT_JOB_FIXTURE_DATABASE_URL is required for canonical fixture setup");
    let api_database_url = env::var("AR09_AGENT_JOB_API_DATABASE_URL")
        .expect("AR09_AGENT_JOB_API_DATABASE_URL is required for the API-role gate");
    let worker_database_url = env::var("AR09_AGENT_JOB_WORKER_DATABASE_URL")
        .expect("AR09_AGENT_JOB_WORKER_DATABASE_URL is required for the worker-role gate");
    let canonical_database_url = env::var("AR09_AGENT_JOB_CANONICAL_DATABASE_URL")
        .expect("AR09_AGENT_JOB_CANONICAL_DATABASE_URL is required for the canonical-role gate");
    let now = now_unix_ms();
    let suffix = format!("{}-{}", std::process::id(), now);
    let campaign_id = format!("ar09-campaign-{suffix}");
    let contract_id = format!("ar09-contract-{suffix}");
    let actor_id = format!("ar09-keeper-{suffix}");
    let player_id = format!("ar09-player-{suffix}");
    let character_id = format!("ar09-character-{suffix}");
    let job_id = format!("ar09-job-{suffix}");
    let stream_id = format!("ar09-stream-{suffix}");
    let payload = r#"{"protected_payload":{"kind":"agent_job_test_input"}}"#;

    let fixture_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&fixture_database_url)
        .await
        .unwrap();
    sqlx::query(
        r#"
        INSERT INTO authority_contracts (
            contract_id, campaign_id, authority_mode, authority_owner,
            contract_version, ruleset_version, house_rules_version,
            scenario_version, prompt_version, agent_pack_version,
            tool_schema_version, safety_profile_version,
            ai_provider_snapshot, model_route_snapshot,
            character_sheet_template_version, created_at, locked
        ) VALUES (
            $1, $2, 'AI_KP', $3, 1, 'coc7-v1', 'none-v1',
            'scenario-v1', 'prompt-v1', 'agent-pack-v1',
            'tool-schema-v1', 'safety-v1', 'provider-snapshot-v1',
            'route-snapshot-v1', 'sheet-v1', now(), TRUE
        )
        "#,
    )
    .bind(&contract_id)
    .bind(&campaign_id)
    .bind(&actor_id)
    .execute(&fixture_pool)
    .await
    .unwrap();
    let mut fixture_transaction = fixture_pool.begin().await.unwrap();
    let input_event_sequence = seed_canonical_agent_job_request(
        &mut fixture_transaction,
        &CanonicalAgentJobFixture {
            suffix: &suffix,
            campaign_id: &campaign_id,
            contract_id: &contract_id,
            actor_id: &actor_id,
            job_id: &job_id,
            stream_id: &stream_id,
            payload,
        },
    )
    .await;
    fixture_transaction.commit().await.unwrap();
    let mut projection_transaction = fixture_pool.begin().await.unwrap();
    sqlx::query("SET LOCAL session_replication_role = 'replica'")
        .execute(&mut *projection_transaction)
        .await
        .unwrap();
    sqlx::query(
        r#"
        INSERT INTO users (
            user_id, login_normalized, password_hash, global_role
        ) VALUES ($1, $2, 'ar09-fixture-hash', 'USER')
        "#,
    )
    .bind(&player_id)
    .bind(format!("ar09-player-{suffix}"))
    .execute(&mut *projection_transaction)
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO campaigns (
            campaign_id, owner_user_id, authority_contract_id, title, state,
            version, created_at, visibility_label, visibility_subject,
            provenance_kind, provenance_reference, provenance_recorded_by,
            last_event_sequence
        ) VALUES (
            $1, $2, $3, 'AR09 durable tool receipt', 'ACTIVE', 1, now(),
            'party_visible', 'not_applicable', 'system_fixture', $4, $2, $5
        )
        "#,
    )
    .bind(&campaign_id)
    .bind(&player_id)
    .bind(&contract_id)
    .bind(format!("ar09-projection-{suffix}"))
    .bind(input_event_sequence)
    .execute(&mut *projection_transaction)
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO characters (
            character_id, campaign_id, owner_user_id, display_name, state,
            current_sheet_version, initial_version_locked, version,
            visibility_label, visibility_subject, provenance_kind,
            provenance_reference, provenance_recorded_by, last_event_sequence
        ) VALUES (
            $1, $2, $3, 'AR09 Investigator', 'APPROVED', 1, TRUE, 1,
            'party_visible', 'not_applicable', 'system_fixture', $4, $3, $5
        )
        "#,
    )
    .bind(&character_id)
    .bind(&campaign_id)
    .bind(&player_id)
    .bind(format!("ar09-character-{suffix}"))
    .bind(input_event_sequence)
    .execute(&mut *projection_transaction)
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO character_sheet_versions (
            sheet_version_id, character_id, version, sheet_json, locked,
            visibility_label, visibility_subject, provenance_kind,
            provenance_reference, provenance_recorded_by, campaign_id,
            last_event_sequence
        ) VALUES (
            $1, $2, 1, '{"skills":{"Library Use":67}}'::jsonb, TRUE,
            'party_visible', 'not_applicable', 'system_fixture', $3, $4, $5, $6
        )
        "#,
    )
    .bind(format!("ar09-sheet-{suffix}"))
    .bind(&character_id)
    .bind(format!("ar09-sheet-{suffix}"))
    .bind(&player_id)
    .bind(&campaign_id)
    .bind(input_event_sequence)
    .execute(&mut *projection_transaction)
    .await
    .unwrap();
    projection_transaction.commit().await.unwrap();

    let api_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&api_database_url)
        .await
        .unwrap();
    let worker_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&worker_database_url)
        .await
        .unwrap();
    let canonical_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&canonical_database_url)
        .await
        .unwrap();
    assert_service_connection(&api_pool, "trpg_api_service").await;
    assert_service_connection(&worker_pool, "trpg_worker_service").await;
    assert_service_connection(&canonical_pool, "trpg_canonical_service").await;
    let canonical_can_insert_events: bool =
        sqlx::query_scalar("SELECT has_table_privilege(current_user, 'event_store', 'INSERT')")
            .fetch_one(&canonical_pool)
            .await
            .unwrap();
    assert!(canonical_can_insert_events);
    assert_permission_denied(
        sqlx::query(
            "UPDATE workflow_instances SET heartbeat_at = heartbeat_at WHERE workflow_id = $1",
        )
        .bind(&job_id)
        .execute(&api_pool)
        .await,
    );
    assert_permission_denied(
        sqlx::query("INSERT INTO agent_job_evidence (job_id) VALUES ($1)")
            .bind(&job_id)
            .execute(&api_pool)
            .await,
    );
    assert_permission_denied(
        sqlx::query("INSERT INTO event_store (event_type) VALUES ('AgentEscapeAttempt')")
            .execute(&worker_pool)
            .await,
    );
    assert_permission_denied(
        sqlx::query(
            "UPDATE workflow_instances SET heartbeat_at = heartbeat_at WHERE workflow_id = $1",
        )
        .bind(&job_id)
        .execute(&canonical_pool)
        .await,
    );

    let api_store = DurableWorkflowStore::connect(&api_database_url)
        .await
        .unwrap();
    let store = DurableWorkflowStore::connect(&worker_database_url)
        .await
        .unwrap();
    store.check_agent_job_readiness().await.unwrap();
    let draft = AgentJobEnqueueDraft {
        job_id: job_id.clone(),
        campaign_id: campaign_id.clone(),
        actor_id: actor_id.clone(),
        agent_kind: "ai_keeper_orchestrator".to_owned(),
        authority_contract_id: contract_id,
        authority_mode: "AI_KP".to_owned(),
        authority_contract_version: 1,
        input_event_sequence,
        input_stream_version: 1,
        visibility_scope_json:
            r#"{"allowed_labels":["party_visible"],"subject_id":null,"output_label":"party_visible"}"#
                .to_owned(),
        rag_snapshot_id: format!("ar09-rag-{suffix}"),
        provider_id: "ar09-provider".to_owned(),
        provider_type: "cloud".to_owned(),
        model_id: "ar09-model".to_owned(),
        model_artifact_sha256: format!("sha256:{}", "3".repeat(64)),
        route_authorization_event_id: format!("ar09-route-{suffix}"),
        prompt_template_id: "keeper-turn".to_owned(),
        prompt_template_version: "v1".to_owned(),
        tool_schema_version: "tool-schema-v1".to_owned(),
        idempotency_key: format!("ar09-job-idempotency-{suffix}"),
        deadline_unix_ms: now + 60_000,
    };
    let enqueued = api_store.enqueue_agent_job(&draft).await.unwrap();
    assert_eq!(enqueued.state, WorkflowState::Requested);
    assert_eq!(api_store.enqueue_agent_job(&draft).await.unwrap(), enqueued);
    let context = store.load_agent_job_context(&job_id).await.unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&context.input_payload_json).unwrap(),
        serde_json::from_str::<serde_json::Value>(payload).unwrap(),
    );
    assert!(context.chunks.is_empty());
    include!("02_durable_agent_job_contract/02_recovery_and_evidence.rs");
}
