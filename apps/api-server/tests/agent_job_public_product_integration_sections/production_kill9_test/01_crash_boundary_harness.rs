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
