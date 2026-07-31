use std::io::{BufRead, BufReader};
use std::os::unix::process::ExitStatusExt;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;

use super::fixture::seed_public_character_fixture;
use super::*;

const CHILD_TEST_NAME: &str = "production_kill9_test::production_agent_job_kill9_child";
const READY_PREFIX: &str = "AR09_PRODUCTION_KILL9_READY=";
const BOUNDARIES: [&str; 8] = [
    "claim_before",
    "claim_after",
    "provider_before",
    "provider_after",
    "tool_before",
    "tool_after",
    "event_commit_before",
    "event_commit_after",
];

fn crash_boundary(expected: &str) {
    if env::var("AR09_PRODUCTION_KILL9_BOUNDARY").as_deref() != Ok(expected) {
        return;
    }
    println!("{READY_PREFIX}{expected}");
    std::io::stdout()
        .flush()
        .expect("production kill9 readiness marker must flush");
    loop {
        thread::park_timeout(Duration::from_secs(60));
    }
}

struct CrashBoundaryRepository {
    inner: DurableWorkflowStore,
}

#[trpg_agent_runtime::repository_async_trait]
impl AgentJobRepository for CrashBoundaryRepository {
    async fn load(&self, job_id: &str) -> AgentJobResult<Option<DurableAgentJob>> {
        AgentJobRepository::load(&self.inner, job_id).await
    }

    async fn claim_due(
        &self,
        claim_owner: &str,
        now_unix_ms: i64,
        lease_duration_ms: i64,
    ) -> AgentJobResult<Option<DurableAgentJob>> {
        crash_boundary("claim_before");
        let claimed =
            AgentJobRepository::claim_due(&self.inner, claim_owner, now_unix_ms, lease_duration_ms)
                .await?;
        crash_boundary("claim_after");
        Ok(claimed)
    }

    async fn transition(&self, draft: &AgentJobTransitionDraft) -> AgentJobResult<DurableAgentJob> {
        AgentJobRepository::transition(&self.inner, draft).await
    }

    async fn heartbeat(
        &self,
        job_id: &str,
        claim_owner: &str,
        claim_token: &str,
        now_unix_ms: i64,
        lease_duration_ms: i64,
    ) -> AgentJobResult<bool> {
        AgentJobRepository::heartbeat(
            &self.inner,
            job_id,
            claim_owner,
            claim_token,
            now_unix_ms,
            lease_duration_ms,
        )
        .await
    }

    async fn cancellation_requested(&self, job_id: &str) -> AgentJobResult<bool> {
        AgentJobRepository::cancellation_requested(&self.inner, job_id).await
    }

    async fn load_authority(
        &self,
        campaign_id: &str,
    ) -> AgentJobResult<DurableAgentAuthoritySnapshot> {
        AgentJobRepository::load_authority(&self.inner, campaign_id).await
    }

    async fn load_context(&self, job_id: &str) -> AgentJobResult<DurableAgentContextSnapshot> {
        AgentJobRepository::load_context(&self.inner, job_id).await
    }

    async fn load_approval(&self, job_id: &str) -> AgentJobResult<Option<DurableAgentApproval>> {
        AgentJobRepository::load_approval(&self.inner, job_id).await
    }

    async fn append_evidence(&self, draft: &AgentJobEvidenceDraft) -> AgentJobResult<()> {
        AgentJobRepository::append_evidence(&self.inner, draft).await
    }
}

struct CrashBoundaryProvider {
    inner: SkillCheckProvider,
}

#[trpg_agent_runtime::repository_async_trait]
impl ExecutableModelProvider for CrashBoundaryProvider {
    fn provider_id(&self) -> &EntityId {
        self.inner.provider_id()
    }

    fn provider_type(&self) -> ProviderType {
        self.inner.provider_type()
    }

    fn model_id(&self) -> &str {
        self.inner.model_id()
    }

    fn model_artifact_sha256(&self) -> &str {
        self.inner.model_artifact_sha256()
    }

    fn startup_route_snapshot(&self) -> ExecutedModelRouteSnapshot {
        self.inner.startup_route_snapshot()
    }

    async fn probe_capabilities(
        &self,
        cancellation: &ProviderCancellation,
    ) -> ModelProviderResult<ProviderExecution<ProviderCapabilities>> {
        self.inner.probe_capabilities(cancellation).await
    }

    async fn chat(
        &self,
        request: &ModelChatRequest,
        cancellation: &ProviderCancellation,
    ) -> ModelProviderResult<ProviderExecution<ModelChatResponse>> {
        crash_boundary("provider_before");
        let execution = self.inner.chat(request, cancellation).await?;
        crash_boundary("provider_after");
        Ok(execution)
    }

    async fn stream_chat(
        &self,
        request: &ModelChatRequest,
        sink: &dyn ModelStreamSink,
        cancellation: &ProviderCancellation,
    ) -> ModelProviderResult<ExecutedModelRouteSnapshot> {
        self.inner.stream_chat(request, sink, cancellation).await
    }

    async fn embed(
        &self,
        request: &ModelEmbeddingRequest,
        cancellation: &ProviderCancellation,
    ) -> ModelProviderResult<ProviderExecution<ModelEmbeddingResponse>> {
        self.inner.embed(request, cancellation).await
    }
}

struct CrashBoundaryToolPort {
    inner: GovernedAgentJobToolPort,
}

#[trpg_agent_runtime::repository_async_trait]
impl AgentJobToolPort for CrashBoundaryToolPort {
    async fn execute(
        &self,
        job: &DurableAgentJob,
        call: &AgentJobToolCall,
        idempotency_key: &str,
        now_unix_ms: i64,
    ) -> AgentJobResult<AgentJobToolResult> {
        crash_boundary("tool_before");
        let result = self
            .inner
            .execute(job, call, idempotency_key, now_unix_ms)
            .await?;
        crash_boundary("tool_after");
        Ok(result)
    }
}

struct CrashBoundaryDecisionPort {
    inner: GovernedAgentDecisionPort,
}

#[trpg_agent_runtime::repository_async_trait]
impl AgentJobDecisionPort for CrashBoundaryDecisionPort {
    async fn authorize_execution(
        &self,
        job: &DurableAgentJob,
        now_unix_ms: i64,
    ) -> AgentJobResult<()> {
        self.inner.authorize_execution(job, now_unix_ms).await
    }

    async fn commit_ai_decision(
        &self,
        job: &DurableAgentJob,
        decision: &AgentStructuredDecision,
        tool_result: Option<&AgentJobToolResult>,
        now_unix_ms: i64,
    ) -> AgentJobResult<AgentJobCommitReceipt> {
        crash_boundary("event_commit_before");
        let receipt = self
            .inner
            .commit_ai_decision(job, decision, tool_result, now_unix_ms)
            .await?;
        crash_boundary("event_commit_after");
        Ok(receipt)
    }
}

fn production_worker_from_environment() -> (tokio::runtime::Runtime, AgentJobWorker) {
    let worker_database_url = required("AR09_PUBLIC_WORKER_DATABASE_URL");
    let canonical_database_url = required("AR09_PUBLIC_CANONICAL_DATABASE_URL");
    let witness_database_url = required("AR09_PUBLIC_WITNESS_DATABASE_URL");
    let redis_url = required("AR09_PUBLIC_REDIS_URL");
    let character_id = required("AR09_PRODUCTION_KILL9_CHARACTER_ID");
    let namespace = required("AR09_PRODUCTION_KILL9_NAMESPACE");
    let audit_path = PathBuf::from(required("AR09_PRODUCTION_KILL9_AUDIT_PATH"));
    let setup_runtime =
        tokio::runtime::Runtime::new().expect("create production kill9 setup runtime");
    let workflow = setup_runtime
        .block_on(DurableWorkflowStore::connect(&worker_database_url))
        .expect("connect production worker repository");
    setup_runtime
        .block_on(workflow.check_agent_job_readiness())
        .expect("production Agent Job schema ready");
    let canonical_store = setup_runtime
        .block_on(PostgresCanonicalStore::connect(
            &canonical_database_url,
            &witness_database_url,
            "ar09-public-integrity",
            &INTEGRITY_KEY,
            "ar09-public-payload",
            &PAYLOAD_KEY,
        ))
        .expect("connect production canonical and witness stores");
    setup_runtime
        .block_on(canonical_store.verify_integrity())
        .expect("production canonical stores ready");
    let canonical: Arc<dyn CanonicalCommitPort> = Arc::new(PostgresCanonicalCommitPort::new(
        Arc::new(Mutex::new(
            tokio::runtime::Runtime::new().expect("canonical child runtime"),
        )),
        canonical_store,
    ));
    let audit = FileAuditLog::open(&audit_path, "ar09-public-audit", &AUDIT_KEY)
        .expect("open production kill9 worker audit");
    let decisions = GovernedAgentDecisionPort::from_prepared_postgres(
        ProductionAgentIdentityConfiguration {
            database_url: &worker_database_url,
            postgres_ca_certificate_pem: None,
            redis_url: &redis_url,
            redis_namespace: &namespace,
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
        audit,
        canonical,
    )
    .expect("compose production governed decision port");
    let tools = GovernedAgentJobToolPort::new(workflow.clone(), Arc::new(Coc7AgentSkillCheckRules));
    let worker = AgentJobWorker::new(
        Arc::new(CrashBoundaryRepository { inner: workflow }),
        Arc::new(CrashBoundaryProvider {
            inner: SkillCheckProvider::new(character_id),
        }),
        Arc::new(CrashBoundaryToolPort { inner: tools }),
        Arc::new(CrashBoundaryDecisionPort { inner: decisions }),
        None,
        AgentJobExecutionConfig {
            claim_owner: "agent_worker_ar09_public".to_owned(),
            lease_duration: Duration::from_secs(30),
            heartbeat_interval: Duration::from_secs(1),
            max_attempts: 5,
            max_context_bytes: 64 * 1024,
            max_input_tokens: 1_000,
            max_output_tokens: 1_000,
            max_tool_calls: 1,
            max_tool_loops: 1,
        },
    )
    .expect("compose production kill9 Agent Job worker");
    (setup_runtime, worker)
}

#[test]
fn production_agent_job_kill9_child() {
    let now_unix_ms = match env::var("AR09_PRODUCTION_KILL9_NOW") {
        Ok(value) => value
            .parse::<i64>()
            .expect("production kill9 child time must be numeric"),
        Err(env::VarError::NotPresent) => return,
        Err(env::VarError::NotUnicode(_)) => {
            panic!("production kill9 child time must be valid Unicode")
        }
    };
    let (_setup_runtime, worker) = production_worker_from_environment();
    let worker_runtime =
        tokio::runtime::Runtime::new().expect("create production kill9 worker runtime");
    let outcome = worker_runtime
        .block_on(worker.run_once(now_unix_ms))
        .expect("production kill9 worker execution succeeds");
    assert!(matches!(outcome, AgentJobOutcome::Completed { .. }));
}

fn spawn_child(
    boundary: &str,
    now_unix_ms: i64,
    character_id: &str,
    namespace: &str,
    audit_path: &PathBuf,
) -> std::process::Child {
    Command::new(env::current_exe().expect("current test executable must be available"))
        .arg("--exact")
        .arg(CHILD_TEST_NAME)
        .arg("--nocapture")
        .arg("--test-threads=1")
        .env("AR09_PRODUCTION_KILL9_BOUNDARY", boundary)
        .env("AR09_PRODUCTION_KILL9_NOW", now_unix_ms.to_string())
        .env("AR09_PRODUCTION_KILL9_CHARACTER_ID", character_id)
        .env("AR09_PRODUCTION_KILL9_NAMESPACE", namespace)
        .env("AR09_PRODUCTION_KILL9_AUDIT_PATH", audit_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("production kill9 child must start")
}

fn recover_child(
    now_unix_ms: i64,
    character_id: &str,
    namespace: &str,
    audit_path: &PathBuf,
) -> std::process::Output {
    Command::new(env::current_exe().expect("current test executable must be available"))
        .arg("--exact")
        .arg(CHILD_TEST_NAME)
        .arg("--nocapture")
        .arg("--test-threads=1")
        .env("AR09_PRODUCTION_KILL9_BOUNDARY", "none")
        .env("AR09_PRODUCTION_KILL9_NOW", now_unix_ms.to_string())
        .env("AR09_PRODUCTION_KILL9_CHARACTER_ID", character_id)
        .env("AR09_PRODUCTION_KILL9_NAMESPACE", namespace)
        .env("AR09_PRODUCTION_KILL9_AUDIT_PATH", audit_path)
        .output()
        .expect("production kill9 recovery child must start")
}

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
