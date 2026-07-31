use super::*;

fn human_authority(
    campaign_id: &str,
    authority_owner: &str,
    created_at_unix_ms: u64,
) -> AuthorityContract {
    AuthorityContract::new_locked(AuthorityContractDraft {
        contract_id: format!("authority_{}", campaign_id),
        campaign_id: campaign_id.to_owned(),
        mode: AuthorityMode::HumanKp,
        authority_owner: authority_owner.to_owned(),
        version: 1,
        snapshot: AuthorityVersionSnapshotDraft {
            ruleset_version: "coc7-v1".to_owned(),
            house_rules_version: "none-v1".to_owned(),
            scenario_version: "scenario-v1".to_owned(),
            prompt_version: "prompt-v1".to_owned(),
            agent_pack_version: "agent-pack-v1".to_owned(),
            tool_schema_version: "tool-schema-v1".to_owned(),
            safety_profile_version: "safety-v1".to_owned(),
            ai_provider_snapshot: PROVIDER_ID.to_owned(),
            model_route_snapshot: ROUTE_ID.to_owned(),
            character_sheet_template_version: "sheet-v1".to_owned(),
        },
        created_at_unix_ms,
    })
    .expect("valid HUMAN_KP authority")
}

#[test]
fn human_keeper_public_approval_adds_exactly_one_canonical_event() {
    let _test_guard = PUBLIC_AGENT_JOB_TEST_LOCK
        .lock()
        .expect("lock public Agent Job test");
    let fixture_database_url = required("AR09_PUBLIC_FIXTURE_DATABASE_URL");
    let api_database_url = required("AR09_PUBLIC_API_DATABASE_URL");
    let worker_database_url = required("AR09_PUBLIC_WORKER_DATABASE_URL");
    let canonical_database_url = required("AR09_PUBLIC_CANONICAL_DATABASE_URL");
    let witness_database_url = required("AR09_PUBLIC_WITNESS_DATABASE_URL");
    let redis_url = required("AR09_PUBLIC_REDIS_URL");
    let runtime = tokio::runtime::Runtime::new().expect("create setup runtime");
    let now = now_unix_ms();
    let suffix = format!("{}-{now}", std::process::id());
    let campaign_id = format!("campaign_ar09_human_{suffix}");
    let server_owner_id = format!("server_owner_ar09_human_{suffix}");
    let keeper_id = format!("keeper_ar09_human_{suffix}");
    let character_id = format!("character_ar09_human_{suffix}");
    let job_id = format!("job_ar09_human_{suffix}");
    let server_owner_login = format!("server-owner-ar09-human-{suffix}@example.test");
    let keeper_login = format!("keeper-ar09-human-{suffix}@example.test");
    let password = "AR09 human approval integration password";

    let mut identity = IdentityService::from_prepared_postgres_with_security_and_redis_tls(
        &api_database_url,
        None,
        &redis_url,
        &format!("ar09:human:{suffix}"),
        &IDENTITY_KEY,
        3_600_000,
        2,
        None,
        None,
        None,
    )
    .expect("connect API identity with least-privilege login");
    identity
        .create_user(
            &server_owner_id,
            &server_owner_login,
            password,
            GlobalRole::ServerOwner,
        )
        .expect("create server owner");
    identity
        .create_user(&keeper_id, &keeper_login, password, GlobalRole::User)
        .expect("create human keeper");
    let server_owner_session = identity
        .login(&server_owner_login, password, now)
        .expect("login server owner");
    let server_owner = identity
        .authenticate_session(Some(server_owner_session.token.expose()), now + 1)
        .expect("authenticate server owner");
    identity
        .grant_membership(
            &server_owner,
            &campaign_id,
            &keeper_id,
            CampaignRole::HumanKeeper,
            now + 1,
        )
        .expect("grant HUMAN_KEEPER membership");
    identity
        .register_authority_contract(
            &server_owner,
            human_authority(&campaign_id, &keeper_id, now),
            now + 1,
        )
        .expect("register immutable HUMAN_KP authority");
    let keeper_session = identity
        .login(&keeper_login, password, now)
        .expect("login human keeper");
    let keeper_token = keeper_session.token.expose().to_owned();
    seed_workflow_policy(&campaign_id);

    let canonical_store = runtime
        .block_on(PostgresCanonicalStore::connect(
            &canonical_database_url,
            &witness_database_url,
            "ar09-public-integrity",
            &INTEGRITY_KEY,
            "ar09-public-payload",
            &PAYLOAD_KEY,
        ))
        .expect("connect canonical and independent witness stores");
    runtime
        .block_on(canonical_store.verify_integrity())
        .expect("canonical stores ready");
    let api_workflow = runtime
        .block_on(DurableWorkflowStore::connect(&api_database_url))
        .expect("connect API Agent Job store");
    runtime
        .block_on(api_workflow.check_agent_job_readiness())
        .expect("Agent Job schema ready");
    let deletion_repository = runtime
        .block_on(PostgresDeletionRepository::connect(&api_database_url))
        .expect("connect API deletion repository");
    let api_audit_path = PathBuf::from(format!("/tmp/trpg-ar09-human-api-audit-{suffix}.jsonl"));
    let worker_audit_path =
        PathBuf::from(format!("/tmp/trpg-ar09-human-worker-audit-{suffix}.jsonl"));
    let api_audit = FileAuditLog::open(&api_audit_path, "ar09-public-audit", &AUDIT_KEY)
        .expect("open API audit");
    let application = ApiApplication::new_production_governed_with_agent_jobs(
        identity,
        policy(),
        api_audit,
        tokio::runtime::Runtime::new().expect("API canonical runtime"),
        canonical_store.clone(),
        tokio::runtime::Runtime::new().expect("API privacy runtime"),
        deletion_repository,
        None,
        api_workflow,
        AgentJobRouteConfiguration {
            provider_id: PROVIDER_ID.to_owned(),
            provider_type: "cloud".to_owned(),
            model_id: MODEL_ID.to_owned(),
            model_artifact_sha256: ARTIFACT.to_owned(),
            route_authorization_event_id: ROUTE_ID.to_owned(),
        },
    )
    .expect("compose public HUMAN_KP Agent Job gateway");

    let request_body = json!({
        "command": {
            "command_id": format!("command_{job_id}"),
            "idempotency_key": format!("idempotency_{job_id}"),
            "expected_version": 0,
            "correlation_id": format!("correlation_{job_id}"),
            "causation_id": format!("causation_{job_id}"),
            "trace_id": format!("trace_{job_id}")
        },
        "campaign_id": campaign_id,
        "job_id": job_id,
        "rag_snapshot_id": format!("rag_{job_id}"),
        "input": {"kind": "keeper_draft"},
        "deadline_unix_ms": i64::try_from(now + 240_000).expect("deadline fits i64")
    });
    let public_request = request(
        &format!("/api/v1/campaigns/{campaign_id}/agent-jobs"),
        &keeper_token,
        request_body,
    );
    let accepted = call(&application, &public_request);
    assert_eq!(
        accepted.status, 202,
        "public HUMAN_KP Agent Job rejected: {}",
        accepted.body
    );
    assert_eq!(accepted.body["state"], "REQUESTED");

    let fixture_pool = runtime
        .block_on(
            PgPoolOptions::new()
                .max_connections(1)
                .connect(&fixture_database_url),
        )
        .expect("connect fixture owner");
    super::fixture::retire_stale_agent_jobs(&runtime, &fixture_pool, &job_id);
    let event_count_before_worker: i64 = runtime
        .block_on(
            sqlx::query_scalar(
                "SELECT count(*) FROM event_store \
                  WHERE campaign_id = $1 AND stream_id = $2",
            )
            .bind(&campaign_id)
            .bind(&job_id)
            .fetch_one(&fixture_pool),
        )
        .expect("count canonical events before HUMAN_KP draft");
    assert_eq!(event_count_before_worker, 1);

    let worker_workflow = runtime
        .block_on(DurableWorkflowStore::connect(&worker_database_url))
        .expect("connect production worker store");
    let canonical_port_runtime = Arc::new(Mutex::new(
        tokio::runtime::Runtime::new().expect("canonical port runtime"),
    ));
    let canonical_port: Arc<dyn CanonicalCommitPort> = Arc::new(PostgresCanonicalCommitPort::new(
        canonical_port_runtime,
        canonical_store,
    ));
    let worker_audit = FileAuditLog::open(&worker_audit_path, "ar09-public-audit", &AUDIT_KEY)
        .expect("open worker audit");
    let decisions: Arc<dyn AgentJobDecisionPort> = Arc::new(
        GovernedAgentDecisionPort::from_prepared_postgres(
            ProductionAgentIdentityConfiguration {
                database_url: &worker_database_url,
                postgres_ca_certificate_pem: None,
                redis_url: &redis_url,
                redis_namespace: &format!("ar09:human:{suffix}"),
                signing_key: &IDENTITY_KEY,
                session_ttl_ms: 3_600_000,
                argon2_concurrency: 2,
                redis_root_certificate: None,
                redis_client_certificate: None,
                redis_client_private_key: None,
                workload_id: "agent_worker_ar09_public",
                internal_credential_ttl_ms: 60_000,
            },
            policy(),
            worker_audit,
            canonical_port,
        )
        .expect("compose production governed decision port"),
    );
    let tools: Arc<dyn AgentJobToolPort> = Arc::new(GovernedAgentJobToolPort::new(
        worker_workflow.clone(),
        Arc::new(Coc7AgentSkillCheckRules),
    ));
    let provider = Arc::new(SkillCheckProvider::new(character_id));
    let worker = AgentJobWorker::new(
        Arc::new(worker_workflow.clone()),
        provider.clone(),
        tools,
        decisions,
        None,
        AgentJobExecutionConfig {
            claim_owner: "agent_worker_ar09_public".to_owned(),
            lease_duration: Duration::from_secs(30),
            heartbeat_interval: Duration::from_millis(10),
            max_attempts: 5,
            max_context_bytes: 64 * 1024,
            max_input_tokens: 1_000,
            max_output_tokens: 1_000,
            max_tool_calls: 1,
            max_tool_loops: 1,
        },
    )
    .expect("compose production HUMAN_KP Agent Job worker");

    let draft = runtime
        .block_on(worker.run_once(i64::try_from(now_unix_ms() + 1_000).expect("time fits")))
        .expect("execute HUMAN_KP draft");
    assert_eq!(
        draft,
        AgentJobOutcome::AwaitingHumanApproval {
            job_id: job_id.clone()
        }
    );
    assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
    let event_count_before_approval: i64 = runtime
        .block_on(
            sqlx::query_scalar(
                "SELECT count(*) FROM event_store \
                  WHERE campaign_id = $1 AND stream_id = $2",
            )
            .bind(&campaign_id)
            .bind(&job_id)
            .fetch_one(&fixture_pool),
        )
        .expect("count canonical events after unapproved draft");
    assert_eq!(event_count_before_approval, event_count_before_worker);
    let awaiting_job = runtime
        .block_on(worker_workflow.load_agent_job(&job_id))
        .expect("reload awaiting HUMAN_KP Agent Job")
        .expect("awaiting HUMAN_KP Agent Job exists");
    assert_eq!(awaiting_job.state.as_str(), "AWAITING_TOOL");
    assert!(awaiting_job.decision_json.is_some());

    let approval_body = json!({
        "command": {
            "command_id": format!("command_approve_{job_id}"),
            "idempotency_key": format!("idempotency_approve_{job_id}"),
            "expected_version": awaiting_job.input_stream_version,
            "correlation_id": format!("correlation_approve_{job_id}"),
            "causation_id": format!("causation_approve_{job_id}"),
            "trace_id": format!("trace_approve_{job_id}")
        },
        "campaign_id": campaign_id,
        "job_id": job_id
    });
    let approval_request = request(
        &format!("/api/v1/campaigns/{campaign_id}/agent-jobs/{job_id}/approve"),
        &keeper_token,
        approval_body,
    );
    let approved = call(&application, &approval_request);
    assert_eq!(
        approved.status, 202,
        "public HUMAN_KP approval rejected: {}",
        approved.body
    );
    assert_eq!(approved.body["state"], "APPROVED");
    let approval_sequence = approved.body["approval_event_sequence"]
        .as_i64()
        .expect("approval event sequence");
    let event_counts_after_approval: (i64, i64) = runtime
        .block_on(
            sqlx::query_as(
                r#"
                SELECT count(*),
                       count(*) FILTER (WHERE event_type = 'AgentDraftApproved')
                  FROM event_store
                 WHERE campaign_id = $1 AND stream_id = $2
                "#,
            )
            .bind(&campaign_id)
            .bind(&job_id)
            .fetch_one(&fixture_pool),
        )
        .expect("count canonical approval event");
    assert_eq!(
        event_counts_after_approval,
        (event_count_before_approval + 1, 1)
    );

    let approval_retry = call(&application, &approval_request);
    assert_eq!(approval_retry.status, 202);
    assert_eq!(
        approval_retry.body["approval_event_sequence"],
        approved.body["approval_event_sequence"]
    );
    let event_count_after_replay: i64 = runtime
        .block_on(
            sqlx::query_scalar(
                "SELECT count(*) FROM event_store \
                  WHERE campaign_id = $1 AND stream_id = $2",
            )
            .bind(&campaign_id)
            .bind(&job_id)
            .fetch_one(&fixture_pool),
        )
        .expect("count canonical events after approval replay");
    assert_eq!(event_count_after_replay, event_count_before_approval + 1);

    let completed = runtime
        .block_on(worker.run_once(i64::try_from(now_unix_ms() + 1_000).expect("time fits")))
        .expect("complete approved HUMAN_KP draft");
    assert_eq!(
        completed,
        AgentJobOutcome::Completed {
            job_id: job_id.clone(),
            event_sequences: vec![approval_sequence],
        }
    );
    assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
    let final_event_counts: (i64, i64, i64, i64, i64) = runtime
        .block_on(
            sqlx::query_as(
                r#"
                SELECT
                    count(*) FILTER (WHERE event_type = 'AgentJobRequested'),
                    count(*) FILTER (WHERE event_type = 'AgentDraftApproved'),
                    count(*) FILTER (WHERE event_type = 'DecisionCommitted'),
                    count(*) FILTER (WHERE event_type = 'ToolExecutionSucceeded'),
                    count(*)
                  FROM event_store
                 WHERE campaign_id = $1 AND stream_id = $2
                "#,
            )
            .bind(&campaign_id)
            .bind(&job_id)
            .fetch_one(&fixture_pool),
        )
        .expect("count final HUMAN_KP canonical events");
    assert_eq!(final_event_counts, (1, 1, 0, 0, 2));

    for path in [
        &api_audit_path,
        &api_audit_path.with_extension("jsonl.head"),
        &worker_audit_path,
        &worker_audit_path.with_extension("jsonl.head"),
    ] {
        let _ = std::fs::remove_file(path);
    }
}
