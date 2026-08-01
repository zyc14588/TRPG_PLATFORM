#[test]
fn production_postgres_kill9_boundaries_recover_exactly_once() {
    let _test_guard = PUBLIC_AGENT_JOB_TEST_LOCK
        .lock()
        .expect("lock public Agent Job test");
    let fixture_database_url = required("AR09_PUBLIC_FIXTURE_DATABASE_URL");
    let api_database_url = required("AR09_PUBLIC_API_DATABASE_URL");
    let canonical_database_url = required("AR09_PUBLIC_CANONICAL_DATABASE_URL");
    let witness_database_url = required("AR09_PUBLIC_WITNESS_DATABASE_URL");
    let redis_url = required("AR09_PUBLIC_REDIS_URL");
    let runtime = tokio::runtime::Runtime::new().expect("create production kill9 parent runtime");
    let now = now_unix_ms();
    let suffix = format!("{}-{now}", std::process::id());
    let owner_id = format!("owner_ar09_production_kill9_{suffix}");
    let login = format!("owner-ar09-production-kill9-{suffix}@example.test");
    let password = "AR09 production kill9 password long enough";
    let scenarios = BOUNDARIES
        .iter()
        .enumerate()
        .map(|(index, boundary)| {
            (
                *boundary,
                format!("campaign_ar09_k9_{index}_{suffix}"),
                format!("ai_keeper_ar09_k9_{index}_{suffix}"),
                format!("character_ar09_k9_{index}_{suffix}"),
                format!("job_ar09_k9_{index}_{suffix}"),
            )
        })
        .collect::<Vec<_>>();

    let mut identity = IdentityService::from_prepared_postgres_with_security_and_redis_tls(
        &api_database_url,
        None,
        &redis_url,
        &format!("ar09:production-kill9:{suffix}"),
        &IDENTITY_KEY,
        3_600_000,
        2,
        None,
        None,
        None,
    )
    .expect("connect production kill9 API identity");
    identity
        .create_user(&owner_id, &login, password, GlobalRole::ServerOwner)
        .expect("create production kill9 owner");
    let session = identity
        .login(&login, password, now)
        .expect("login production kill9 owner");
    let token = session.token.expose().to_owned();
    let authentication = identity
        .authenticate_session(Some(&token), now + 1)
        .expect("authenticate production kill9 owner");
    for (_, campaign_id, authority_owner, _, _) in &scenarios {
        identity
            .register_authority_contract(
                &authentication,
                authority(campaign_id, authority_owner, now),
                now + 1,
            )
            .expect("register immutable production kill9 authority");
        seed_workflow_policy(campaign_id);
    }

    let canonical_store = runtime
        .block_on(PostgresCanonicalStore::connect(
            &canonical_database_url,
            &witness_database_url,
            "ar09-public-integrity",
            &INTEGRITY_KEY,
            "ar09-public-payload",
            &PAYLOAD_KEY,
        ))
        .expect("connect production kill9 canonical stores");
    runtime
        .block_on(canonical_store.verify_integrity())
        .expect("production kill9 canonical stores ready");
    let api_workflow = runtime
        .block_on(DurableWorkflowStore::connect(&api_database_url))
        .expect("connect production kill9 API store");
    let deletion_repository = runtime
        .block_on(PostgresDeletionRepository::connect(&api_database_url))
        .expect("connect production kill9 deletion repository");
    let api_audit_path = PathBuf::from(format!(
        "/tmp/trpg-ar09-production-kill9-api-{suffix}.jsonl"
    ));
    let api_audit = FileAuditLog::open(&api_audit_path, "ar09-public-audit", &AUDIT_KEY)
        .expect("open production kill9 API audit");
    let application = ApiApplication::new_production_governed_with_agent_jobs(
        identity,
        policy(),
        api_audit,
        tokio::runtime::Runtime::new().expect("production kill9 API canonical runtime"),
        canonical_store,
        tokio::runtime::Runtime::new().expect("production kill9 API privacy runtime"),
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
    .expect("compose production kill9 public Agent Job gateway");
    let fixture_pool = runtime
        .block_on(
            PgPoolOptions::new()
                .max_connections(1)
                .connect(&fixture_database_url),
        )
        .expect("connect production kill9 fixture owner");

    for (boundary, campaign_id, _, character_id, job_id) in scenarios {
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
            "input": {"kind": "npc_skill_check"},
            "deadline_unix_ms": i64::try_from(now + 240_000)
                .expect("production kill9 deadline fits i64")
        });
        let accepted = call(
            &application,
            &request(
                &format!("/api/v1/campaigns/{campaign_id}/agent-jobs"),
                &token,
                request_body,
            ),
        );
        assert_eq!(
            accepted.status, 202,
            "production kill9 public request rejected at {boundary}: {}",
            accepted.body
        );
        let input_event_sequence = accepted.body["input_event_sequence"]
            .as_i64()
            .expect("production kill9 input event sequence");
        seed_public_character_fixture(
            &runtime,
            &fixture_pool,
            job_id.clone(),
            campaign_id.clone(),
            owner_id.clone(),
            character_id.clone(),
            input_event_sequence,
        );

        let first_now = i64::try_from(now_unix_ms() + 1_000).expect("production kill9 time fits");
        let namespace = format!("ar09:production-kill9:{suffix}:{boundary}");
        let worker_audit_path = PathBuf::from(format!(
            "/tmp/trpg-ar09-production-kill9-worker-{suffix}-{boundary}.jsonl"
        ));
        let mut child = spawn_child(
            boundary,
            first_now,
            &character_id,
            &namespace,
            &worker_audit_path,
        );
        let stdout = child
            .stdout
            .take()
            .expect("production kill9 child stdout must be captured");
        let expected_marker = format!("{READY_PREFIX}{boundary}");
        let (sender, receiver) = mpsc::channel();
        let reader = thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let line = line.expect("production kill9 child output must be readable");
                if line.contains(&expected_marker) {
                    sender
                        .send(expected_marker.clone())
                        .expect("production kill9 marker receiver remains alive");
                    break;
                }
            }
        });
        let marker = receiver
            .recv_timeout(Duration::from_secs(30))
            .unwrap_or_else(|_| {
                panic!("production worker did not reach kill9 boundary {boundary}")
            });
        assert_eq!(marker, format!("{READY_PREFIX}{boundary}"));
        child.kill().expect("production child must accept SIGKILL");
        let status = child.wait().expect("production child must be reaped");
        reader.join().expect("production child reader must finish");
        assert_eq!(
            status.signal(),
            Some(9),
            "{boundary} must terminate through SIGKILL"
        );

        let recovery = recover_child(
            first_now + 120_000,
            &character_id,
            &namespace,
            &worker_audit_path,
        );
        assert!(
            recovery.status.success(),
            "production recovery failed at {boundary}: stdout={} stderr={}",
            String::from_utf8_lossy(&recovery.stdout),
            String::from_utf8_lossy(&recovery.stderr),
        );
        let event_counts: (i64, i64, i64, i64, i64) = runtime
            .block_on(
                sqlx::query_as(
                    r#"
                    SELECT
                        count(*) FILTER (WHERE event_type = 'AgentJobRequested'),
                        count(*) FILTER (WHERE event_type = 'ToolRequestApproved'),
                        count(*) FILTER (WHERE event_type = 'ToolExecutionSucceeded'),
                        count(*) FILTER (WHERE event_type = 'DecisionCommitted'),
                        count(*)
                      FROM event_store
                     WHERE campaign_id = $1 AND stream_id = $2
                    "#,
                )
                .bind(&campaign_id)
                .bind(&job_id)
                .fetch_one(&fixture_pool),
            )
            .expect("count production kill9 canonical events");
        assert_eq!(event_counts, (1, 1, 1, 1, 4), "{boundary}");
        let tool_receipts: i64 = runtime
            .block_on(
                sqlx::query_scalar(
                    "SELECT count(*) FROM agent_job_tool_receipts WHERE job_id = $1",
                )
                .bind(&job_id)
                .fetch_one(&fixture_pool),
            )
            .expect("count production kill9 tool receipts");
        assert_eq!(tool_receipts, 1, "{boundary}");
        let evidence_counts: (i64, i64) = runtime
            .block_on(
                sqlx::query_as(
                    r#"
                    SELECT
                        count(*) FILTER (WHERE phase = 'canonical_commit'),
                        count(*) FILTER (WHERE phase = 'completed')
                      FROM agent_job_evidence
                     WHERE job_id = $1
                    "#,
                )
                .bind(&job_id)
                .fetch_one(&fixture_pool),
            )
            .expect("count production kill9 decision evidence");
        assert!(
            evidence_counts.0 >= 1 && evidence_counts.1 >= 1,
            "{boundary} must retain canonical and completion evidence"
        );
        let state: String = runtime
            .block_on(
                sqlx::query_scalar("SELECT state FROM workflow_instances WHERE workflow_id = $1")
                    .bind(&job_id)
                    .fetch_one(&fixture_pool),
            )
            .expect("load production kill9 workflow state");
        assert_eq!(state, "COMPLETED", "{boundary}");
        println!(
            "AR09_PRODUCTION_KILL9_VERIFIED boundary={boundary} signal=9 canonical_events={event_counts:?} tool_receipts={tool_receipts}"
        );

        for path in [
            &worker_audit_path,
            &worker_audit_path.with_extension("jsonl.head"),
        ] {
            let _ = std::fs::remove_file(path);
        }
    }

    for path in [
        &api_audit_path,
        &api_audit_path.with_extension("jsonl.head"),
    ] {
        let _ = std::fs::remove_file(path);
    }
}
