use super::atomic_agent_job_request_rollback::assert_agent_job_request_rollback;
use super::fixture::seed_public_character_fixture;
use super::*;

#[test]
fn public_agent_job_recovers_with_production_tool_and_canonical_ports() {
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
    let campaign_id = format!("campaign_ar09_public_{suffix}");
    let owner_id = format!("owner_ar09_public_{suffix}");
    let authority_owner = format!("ai_keeper_ar09_public_{suffix}");
    let character_id = format!("character_ar09_public_{suffix}");
    let job_id = format!("job_ar09_public_{suffix}");
    let login = format!("owner-ar09-public-{suffix}@example.test");
    let password = "AR09 public integration password long enough";

    let mut identity = IdentityService::from_prepared_postgres_with_security_and_redis_tls(
        &api_database_url,
        None,
        &redis_url,
        &format!("ar09:public:{suffix}"),
        &IDENTITY_KEY,
        3_600_000,
        2,
        None,
        None,
        None,
    )
    .expect("connect API identity with least-privilege login");
    identity
        .create_user(&owner_id, &login, password, GlobalRole::ServerOwner)
        .expect("create public-path owner");
    let session = identity
        .login(&login, password, now)
        .expect("login public-path owner");
    let token = session.token.expose().to_owned();
    let authentication = identity
        .authenticate_session(Some(&token), now + 1)
        .expect("authenticate public-path owner");
    identity
        .register_authority_contract(
            &authentication,
            authority(&campaign_id, &authority_owner, now),
            now + 1,
        )
        .expect("register immutable AI_KP authority");
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
    let api_audit_path = PathBuf::from(format!("/tmp/trpg-ar09-public-api-audit-{suffix}.jsonl"));
    let worker_audit_path =
        PathBuf::from(format!("/tmp/trpg-ar09-public-worker-audit-{suffix}.jsonl"));
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
    .expect("compose public Agent Job gateway");

    let deadline = i64::try_from(now + 240_000).expect("deadline fits i64");
    let body = json!({
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
        "input": {"kind": "npc_skill_check"},
        "deadline_unix_ms": deadline
    });
    let public_request = request(
        &format!("/api/v1/campaigns/{campaign_id}/agent-jobs"),
        &token,
        body,
    );
    let accepted = call(&application, &public_request);
    assert_eq!(
        accepted.status, 202,
        "public Agent Job rejected: {}",
        accepted.body
    );
    assert_eq!(accepted.body["state"], "REQUESTED");
    let accepted_retry = call(&application, &public_request);
    assert_eq!(accepted_retry.status, 202);
    assert_eq!(
        accepted_retry.body["input_event_sequence"],
        accepted.body["input_event_sequence"]
    );
    let input_event_sequence = accepted.body["input_event_sequence"]
        .as_i64()
        .expect("input event sequence");

    let fixture_pool = runtime
        .block_on(
            PgPoolOptions::new()
                .max_connections(1)
                .connect(&fixture_database_url),
        )
        .expect("connect fixture owner");
    seed_public_character_fixture(
        &runtime,
        &fixture_pool,
        job_id.clone(),
        campaign_id.clone(),
        owner_id.clone(),
        character_id.clone(),
        input_event_sequence,
    );

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
    let canonical_verifier = Arc::clone(&canonical_port);
    let worker_audit = FileAuditLog::open(&worker_audit_path, "ar09-public-audit", &AUDIT_KEY)
        .expect("open worker audit");
    let production_decisions = GovernedAgentDecisionPort::from_prepared_postgres(
        ProductionAgentIdentityConfiguration {
            database_url: &worker_database_url,
            postgres_ca_certificate_pem: None,
            redis_url: &redis_url,
            redis_namespace: &format!("ar09:public:{suffix}"),
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
    .expect("compose production governed decision port");
    let decisions: Arc<dyn AgentJobDecisionPort> = Arc::new(LoseFirstCanonicalReceipt {
        inner: production_decisions,
        lost: AtomicBool::new(false),
    });
    let tools: Arc<dyn AgentJobToolPort> = Arc::new(GovernedAgentJobToolPort::new(
        worker_workflow.clone(),
        Arc::new(Coc7AgentSkillCheckRules),
    ));
    let provider = Arc::new(SkillCheckProvider::new(character_id.clone()));
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
    .expect("compose production Agent Job worker");

    let first_worker_now = i64::try_from(now_unix_ms() + 1_000).expect("current worker time fits");
    let first = runtime
        .block_on(worker.run_once(first_worker_now))
        .expect("first worker execution");
    assert!(
        matches!(
            &first,
            AgentJobOutcome::RetryScheduled {
                error_code: "AR09_INJECTED_RECEIPT_LOSS_AFTER_CANONICAL_COMMIT",
                ..
            }
        ),
        "unexpected first worker outcome: {first:?}"
    );
    let event_count_after_loss: i64 = runtime
        .block_on(
            sqlx::query_scalar(
                "SELECT count(*) FROM event_store \
                  WHERE campaign_id = $1 AND stream_id = $2 \
                    AND event_type = 'ToolExecutionSucceeded'",
            )
            .bind(&campaign_id)
            .bind(&job_id)
            .fetch_one(&fixture_pool),
        )
        .expect("count canonical tool events after injected loss");
    assert_eq!(event_count_after_loss, 1);
    let receipt_count_after_loss: i64 = runtime
        .block_on(
            sqlx::query_scalar("SELECT count(*) FROM agent_job_tool_receipts WHERE job_id = $1")
                .bind(&job_id)
                .fetch_one(&fixture_pool),
        )
        .expect("count durable tool receipts");
    assert_eq!(receipt_count_after_loss, 1);

    let recovered = runtime
        .block_on(worker.run_once(first_worker_now + 120_000))
        .expect("recover worker after canonical receipt loss");
    assert!(matches!(recovered, AgentJobOutcome::Completed { .. }));
    assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
    let duplicate_counts: (i64, i64, i64) = runtime
        .block_on(
            sqlx::query_as(
                r#"
                SELECT
                    count(*) FILTER (
                        WHERE event_type = 'AgentJobRequested'
                    ),
                    count(*) FILTER (
                        WHERE event_type = 'ToolExecutionSucceeded'
                    ),
                    count(*) FILTER (
                        WHERE event_type = 'DecisionCommitted'
                    )
                  FROM event_store
                 WHERE campaign_id = $1 AND stream_id = $2
                "#,
            )
            .bind(&campaign_id)
            .bind(&job_id)
            .fetch_one(&fixture_pool),
        )
        .expect("count exact canonical events");
    assert_eq!(duplicate_counts, (1, 1, 1));

    let completed_job = runtime
        .block_on(worker_workflow.load_agent_job(&job_id))
        .expect("reload completed Agent Job")
        .expect("completed Agent Job exists");
    let canonical_receipt = canonical_verifier
        .load_receipt(&CanonicalCommitKey {
            commit_id: format!("{campaign_id}_command_agent_decision_{job_id}"),
            campaign_id: campaign_id.clone(),
            stream_id: job_id.clone(),
            idempotency_key: format!("idempotency_{job_id}:canonical"),
            expected_version: u64::try_from(completed_job.input_stream_version)
                .expect("input stream version fits u64"),
        })
        .expect("load trusted canonical receipt")
        .expect("canonical receipt exists");
    let canonical_tool_event = canonical_receipt
        .events
        .iter()
        .find(|event| event.event_type == "ToolExecutionSucceeded")
        .expect("canonical tool event exists");
    let canonical_payload: Value = serde_json::from_str(&canonical_tool_event.payload_json)
        .expect("decode trusted canonical payload");
    let event_result = canonical_payload["ToolExecutionSucceeded"]["result"].clone();
    let event_hash = canonical_payload["ToolExecutionSucceeded"]["result_hash"]
        .as_str()
        .expect("canonical result hash")
        .to_owned();
    let (receipt_result, receipt_hash): (Value, String) = runtime
        .block_on(
            sqlx::query_as(
                r#"
                SELECT result_json, result_hash
                  FROM agent_job_tool_receipts
                 WHERE job_id = $1
                "#,
            )
            .bind(&job_id)
            .fetch_one(&fixture_pool),
        )
        .expect("load canonical and durable tool results");
    assert_eq!(event_result, receipt_result);
    assert_eq!(event_hash, receipt_hash);
    assert_eq!(event_result["target"], 72);
    assert_eq!(event_result["random_source"], "SERVER_OS_CSPRNG");
    assert_eq!(
        runtime
            .block_on(worker.run_once(first_worker_now + 121_000))
            .expect("completed job is not reclaimed"),
        AgentJobOutcome::Idle
    );

    assert_agent_job_request_rollback(
        &application,
        &runtime,
        &fixture_pool,
        &campaign_id,
        &token,
        &suffix,
    );

    for path in [
        &api_audit_path,
        &api_audit_path.with_extension("jsonl.head"),
        &worker_audit_path,
        &worker_audit_path.with_extension("jsonl.head"),
    ] {
        let _ = std::fs::remove_file(path);
    }
}
