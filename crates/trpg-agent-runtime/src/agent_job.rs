use std::collections::HashSet;
use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use trpg_identity::{
    AgentClass as IdentityAgentClass, IdentityService, WorkloadRole as IdentityWorkloadRole,
};
use trpg_runtime::durable_workflow::{
    AgentJobEvidenceDraft, AgentJobTransitionDraft, DurableAgentApproval,
    DurableAgentAuthoritySnapshot, DurableAgentContextSnapshot, DurableAgentJob,
    DurableWorkflowStore, WorkflowState, WorkflowStoreError,
};
use trpg_security_governance::formal_commit_audit::{FormalCommitAudit, FormalCommitAuthorizer};
use trpg_security_governance::policy_adapter::OpenFgaOpaPolicyAdapter;
use trpg_security_governance::tamper_evident_audit::FileAuditLog;
use trpg_shared_kernel::{
    AuthenticatedCommandContext, CanonicalCommitPort, CommandEnvelope, CommandMetadata, EntityId,
    FactProvenance, FormalWritePath, ProvenanceKind, ResourceRef, Visibility,
};

use crate::agent_runtime::{
    evaluate_agent_tool_request, evaluate_prompt_injection, AgentDecision, AgentDecisionCommitter,
    AgentEventPayload, AgentKind, AgentTool, EventStore as AgentEventStore, ToolRequest,
};
use crate::local_model_certification::{LocalModelCertificate, LocalModelCertificationAuthority};
use crate::model_provider::{
    ExecutableModelProvider, ExecutedModelRouteSnapshot, ModelChatRequest, ModelMessage,
    ModelMessageRole, ModelOperation, ModelProviderError, ModelProviderErrorKind,
    ModelToolDefinition, ProviderCancellation, ProviderType, StructuredOutputRequest,
};

const AGENT_RUNTIME_VERSION: &str = env!("CARGO_PKG_VERSION");
const EMPTY_SHA256: &str =
    "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
const EVIDENCE_RETENTION_MS: i64 = 30 * 24 * 60 * 60 * 1_000;

pub type AgentJobResult<T> = Result<T, AgentJobError>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentJobError {
    code: &'static str,
    retryable: bool,
}

impl AgentJobError {
    pub const fn retryable(code: &'static str) -> Self {
        Self {
            code,
            retryable: true,
        }
    }

    pub const fn terminal(code: &'static str) -> Self {
        Self {
            code,
            retryable: false,
        }
    }

    pub const fn code(&self) -> &'static str {
        self.code
    }

    pub const fn is_retryable(&self) -> bool {
        self.retryable
    }
}

impl fmt::Display for AgentJobError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code)
    }
}

impl std::error::Error for AgentJobError {}

#[derive(Clone, Debug)]
pub struct AgentJobExecutionConfig {
    pub claim_owner: String,
    pub lease_duration: Duration,
    pub heartbeat_interval: Duration,
    pub max_attempts: i32,
    pub max_context_bytes: usize,
    pub max_input_tokens: u64,
    pub max_output_tokens: u64,
    pub max_tool_calls: usize,
    pub max_tool_loops: usize,
}

impl AgentJobExecutionConfig {
    pub fn validate(&self) -> AgentJobResult<()> {
        if self.claim_owner.trim().is_empty()
            || self.lease_duration.is_zero()
            || self.heartbeat_interval.is_zero()
            || self.heartbeat_interval >= self.lease_duration
            || self.max_attempts <= 0
            || self.max_context_bytes == 0
            || self.max_context_bytes > 1_048_576
            || self.max_input_tokens == 0
            || self.max_output_tokens == 0
            || self.max_tool_calls != 1
            || self.max_tool_loops != 1
        {
            return Err(AgentJobError::terminal("AGENT_JOB_CONFIGURATION_INVALID"));
        }
        Ok(())
    }
}

impl Default for AgentJobExecutionConfig {
    fn default() -> Self {
        Self {
            claim_owner: "agent-worker-primary".to_owned(),
            lease_duration: Duration::from_secs(30),
            heartbeat_interval: Duration::from_secs(5),
            max_attempts: 5,
            max_context_bytes: 256 * 1024,
            max_input_tokens: 32_768,
            max_output_tokens: 4_096,
            max_tool_calls: 1,
            max_tool_loops: 1,
        }
    }
}

#[derive(Clone)]
pub struct CertifiedLocalModel {
    authority: Arc<LocalModelCertificationAuthority>,
    certificate: LocalModelCertificate,
}

impl fmt::Debug for CertifiedLocalModel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CertifiedLocalModel")
            .field("certificate", &self.certificate)
            .field("authority", &"[CERTIFICATION AUTHORITY]")
            .finish()
    }
}

impl CertifiedLocalModel {
    pub fn new(
        authority: Arc<LocalModelCertificationAuthority>,
        certificate: LocalModelCertificate,
    ) -> Self {
        Self {
            authority,
            certificate,
        }
    }

    fn ensure_ai_keeper(&self, model_id: &str, model_artifact_sha256: &str) -> AgentJobResult<()> {
        self.authority
            .ensure_ai_keeper_model(&self.certificate, model_id, model_artifact_sha256)
            .map_err(|_| AgentJobError::terminal("LOCAL_MODEL_LEVEL_4_REQUIRED"))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentJobToolCall {
    pub name: String,
    pub arguments: Value,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentJobToolResult {
    pub execution_id: String,
    pub result_hash: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentStructuredDecision {
    pub kind: String,
    pub player_visible_text: String,
    pub tool: Option<AgentJobToolCall>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentJobCommitReceipt {
    pub event_sequences: Vec<i64>,
}

#[async_trait]
pub trait AgentJobToolPort: Send + Sync {
    async fn execute(
        &self,
        job: &DurableAgentJob,
        call: &AgentJobToolCall,
        idempotency_key: &str,
    ) -> AgentJobResult<AgentJobToolResult>;
}

#[derive(Debug, Default)]
pub struct RejectingAgentJobToolPort;

#[async_trait]
impl AgentJobToolPort for RejectingAgentJobToolPort {
    async fn execute(
        &self,
        _job: &DurableAgentJob,
        _call: &AgentJobToolCall,
        _idempotency_key: &str,
    ) -> AgentJobResult<AgentJobToolResult> {
        Err(AgentJobError::terminal("AGENT_TOOL_PERMISSION_DENIED"))
    }
}

#[async_trait]
pub trait AgentJobDecisionPort: Send + Sync {
    async fn authorize_execution(
        &self,
        job: &DurableAgentJob,
        now_unix_ms: i64,
    ) -> AgentJobResult<()>;

    async fn commit_ai_decision(
        &self,
        job: &DurableAgentJob,
        decision: &AgentStructuredDecision,
        tool_result: Option<&AgentJobToolResult>,
        now_unix_ms: i64,
    ) -> AgentJobResult<AgentJobCommitReceipt>;
}

pub struct ProductionAgentIdentityConfiguration<'a> {
    pub database_url: &'a str,
    pub postgres_ca_certificate_pem: Option<&'a [u8]>,
    pub redis_url: &'a str,
    pub redis_namespace: &'a str,
    pub signing_key: &'a [u8; 32],
    pub session_ttl_ms: u64,
    pub argon2_concurrency: usize,
    pub redis_root_certificate: Option<&'a [u8]>,
    pub redis_client_certificate: Option<&'a [u8]>,
    pub redis_client_private_key: Option<&'a [u8]>,
    pub workload_id: &'a str,
    pub internal_credential_ttl_ms: u64,
}

pub struct GovernedAgentDecisionPort {
    identity: Mutex<IdentityService>,
    committer: AgentDecisionCommitter,
    events: Mutex<AgentEventStore<AgentEventPayload>>,
    workload_id: String,
    internal_credential_ttl_ms: u64,
}

impl fmt::Debug for GovernedAgentDecisionPort {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GovernedAgentDecisionPort")
            .field("identity", &"[PERSISTENT IDENTITY SERVICE]")
            .field("committer", &self.committer)
            .field("events", &"[CANONICAL AGENT EVENT CUSTODY]")
            .field("workload_id", &self.workload_id)
            .field(
                "internal_credential_ttl_ms",
                &self.internal_credential_ttl_ms,
            )
            .finish()
    }
}

impl GovernedAgentDecisionPort {
    pub fn from_prepared_postgres(
        configuration: ProductionAgentIdentityConfiguration<'_>,
        policy: OpenFgaOpaPolicyAdapter,
        audit: FileAuditLog,
        canonical: Arc<dyn CanonicalCommitPort>,
    ) -> AgentJobResult<Self> {
        if configuration.workload_id.trim().is_empty()
            || configuration.internal_credential_ttl_ms == 0
        {
            return Err(AgentJobError::terminal(
                "AGENT_IDENTITY_CONFIGURATION_INVALID",
            ));
        }
        let identity = IdentityService::from_prepared_postgres_with_security_and_redis_tls(
            configuration.database_url,
            configuration.postgres_ca_certificate_pem,
            configuration.redis_url,
            configuration.redis_namespace,
            configuration.signing_key,
            configuration.session_ttl_ms,
            configuration.argon2_concurrency,
            configuration.redis_root_certificate,
            configuration.redis_client_certificate,
            configuration.redis_client_private_key,
        )
        .map_err(|_| AgentJobError::terminal("AGENT_IDENTITY_UNAVAILABLE"))?;
        let verifier = identity.verifier();
        let authorizer = FormalCommitAuthorizer::new(
            verifier.clone(),
            policy,
            FormalCommitAudit::from_file_log(audit),
        );
        let committer = AgentDecisionCommitter::new(verifier)
            .map_err(|_| AgentJobError::terminal("AGENT_COMMITTER_INVALID"))?;
        let events = AgentEventStore::with_formal_custody(authorizer, canonical);
        Ok(Self {
            identity: Mutex::new(identity),
            committer,
            events: Mutex::new(events),
            workload_id: configuration.workload_id.to_owned(),
            internal_credential_ttl_ms: configuration.internal_credential_ttl_ms,
        })
    }
}

#[async_trait]
impl AgentJobDecisionPort for GovernedAgentDecisionPort {
    async fn authorize_execution(
        &self,
        job: &DurableAgentJob,
        now_unix_ms: i64,
    ) -> AgentJobResult<()> {
        let now_unix_ms = u64::try_from(now_unix_ms)
            .map_err(|_| AgentJobError::terminal("AGENT_JOB_TIME_INVALID"))?;
        let expires_at_unix_ms = now_unix_ms
            .checked_add(self.internal_credential_ttl_ms)
            .ok_or_else(|| AgentJobError::terminal("AGENT_JOB_TIME_INVALID"))?;
        let campaign_id = EntityId::new(&job.campaign_id)
            .map_err(|_| AgentJobError::terminal("AGENT_JOB_CAMPAIGN_INVALID"))?;
        let agent_class = match (job.authority_mode.as_str(), job.agent_kind.as_str()) {
            ("AI_KP", "ai_keeper_orchestrator") => IdentityAgentClass::AiKeeperOrchestrator,
            ("HUMAN_KP", "keeper_copilot") => IdentityAgentClass::KeeperCopilot,
            _ => {
                return Err(AgentJobError::terminal(
                    "AGENT_JOB_AUTHORITY_SNAPSHOT_MISMATCH",
                ));
            }
        };
        let mut identity = self
            .identity
            .lock()
            .map_err(|_| AgentJobError::retryable("AGENT_IDENTITY_LOCK_UNAVAILABLE"))?;
        let contract = identity
            .authority_contract(&campaign_id)
            .map_err(|_| AgentJobError::retryable("AGENT_IDENTITY_UNAVAILABLE"))?
            .ok_or_else(|| AgentJobError::terminal("AGENT_AUTHORITY_CONTRACT_REQUIRED"))?;
        let expected_mode = if job.authority_mode == "AI_KP" {
            trpg_shared_kernel::AuthorityMode::AiKp
        } else {
            trpg_shared_kernel::AuthorityMode::HumanKp
        };
        if contract.contract_id().as_str() != job.authority_contract_id
            || contract.version()
                != u64::try_from(job.authority_contract_version)
                    .map_err(|_| AgentJobError::terminal("AGENT_JOB_AUTHORITY_SNAPSHOT_MISMATCH"))?
            || contract.mode() != &expected_mode
            || (expected_mode == trpg_shared_kernel::AuthorityMode::AiKp
                && contract.authority_owner().as_str() != job.actor_id)
        {
            return Err(AgentJobError::terminal(
                "AGENT_JOB_AUTHORITY_SNAPSHOT_MISMATCH",
            ));
        }
        let workload_credential = identity
            .issue_workload_credential(
                &self.workload_id,
                IdentityWorkloadRole::WorkflowEngine,
                now_unix_ms,
                expires_at_unix_ms,
            )
            .map_err(|_| AgentJobError::terminal("AGENT_WORKLOAD_IDENTITY_INVALID"))?;
        let workflow_authentication = identity
            .authenticate_workload(&workload_credential, now_unix_ms)
            .map_err(|_| AgentJobError::terminal("AGENT_WORKLOAD_IDENTITY_INVALID"))?;
        identity
            .command_actor(&workflow_authentication, &campaign_id, now_unix_ms)
            .map_err(|_| AgentJobError::terminal("AGENT_WORKLOAD_IDENTITY_INVALID"))?;
        let agent_credential = identity
            .issue_agent_run_credential(
                &format!("run_{}", job.job_id),
                &job.actor_id,
                &job.campaign_id,
                agent_class,
                now_unix_ms,
                expires_at_unix_ms,
            )
            .map_err(|_| AgentJobError::terminal("AGENT_RUN_IDENTITY_INVALID"))?;
        let agent_authentication = identity
            .authenticate_agent_run(&agent_credential, now_unix_ms)
            .map_err(|_| AgentJobError::terminal("AGENT_RUN_IDENTITY_INVALID"))?;
        agent_authentication
            .require_campaign(&campaign_id)
            .map_err(|_| AgentJobError::terminal("AGENT_RUN_IDENTITY_INVALID"))?;
        Ok(())
    }

    async fn commit_ai_decision(
        &self,
        job: &DurableAgentJob,
        decision: &AgentStructuredDecision,
        tool_result: Option<&AgentJobToolResult>,
        now_unix_ms: i64,
    ) -> AgentJobResult<AgentJobCommitReceipt> {
        if job.authority_mode != "AI_KP"
            || job.agent_kind != "ai_keeper_orchestrator"
            || decision.tool.is_some()
            || tool_result.is_some()
        {
            return Err(AgentJobError::terminal(
                "AGENT_TOOL_EXECUTION_PORT_REQUIRED",
            ));
        }
        let now_unix_ms = u64::try_from(now_unix_ms)
            .map_err(|_| AgentJobError::terminal("AGENT_JOB_TIME_INVALID"))?;
        let expires_at_unix_ms = now_unix_ms
            .checked_add(self.internal_credential_ttl_ms)
            .ok_or_else(|| AgentJobError::terminal("AGENT_JOB_TIME_INVALID"))?;
        let campaign_id = EntityId::new(&job.campaign_id)
            .map_err(|_| AgentJobError::terminal("AGENT_JOB_CAMPAIGN_INVALID"))?;
        let mut identity = self
            .identity
            .lock()
            .map_err(|_| AgentJobError::retryable("AGENT_IDENTITY_LOCK_UNAVAILABLE"))?;
        let contract = identity
            .authority_contract(&campaign_id)
            .map_err(|_| AgentJobError::retryable("AGENT_IDENTITY_UNAVAILABLE"))?
            .ok_or_else(|| AgentJobError::terminal("AGENT_AUTHORITY_CONTRACT_REQUIRED"))?;
        if contract.contract_id().as_str() != job.authority_contract_id
            || contract.version()
                != u64::try_from(job.authority_contract_version)
                    .map_err(|_| AgentJobError::terminal("AGENT_JOB_AUTHORITY_SNAPSHOT_MISMATCH"))?
            || contract.authority_owner().as_str() != job.actor_id
            || contract.mode() != &trpg_shared_kernel::AuthorityMode::AiKp
        {
            return Err(AgentJobError::terminal(
                "AGENT_JOB_AUTHORITY_SNAPSHOT_MISMATCH",
            ));
        }
        let workload_credential = identity
            .issue_workload_credential(
                &self.workload_id,
                IdentityWorkloadRole::WorkflowEngine,
                now_unix_ms,
                expires_at_unix_ms,
            )
            .map_err(|_| AgentJobError::terminal("AGENT_WORKLOAD_IDENTITY_INVALID"))?;
        let workflow_authentication = identity
            .authenticate_workload(&workload_credential, now_unix_ms)
            .map_err(|_| AgentJobError::terminal("AGENT_WORKLOAD_IDENTITY_INVALID"))?;
        let agent_credential = identity
            .issue_agent_run_credential(
                &format!("run_{}", job.job_id),
                &job.actor_id,
                &job.campaign_id,
                IdentityAgentClass::AiKeeperOrchestrator,
                now_unix_ms,
                expires_at_unix_ms,
            )
            .map_err(|_| AgentJobError::terminal("AGENT_RUN_IDENTITY_INVALID"))?;
        let agent_authentication = identity
            .authenticate_agent_run(&agent_credential, now_unix_ms)
            .map_err(|_| AgentJobError::terminal("AGENT_RUN_IDENTITY_INVALID"))?;
        let workflow_actor = identity
            .command_actor(&workflow_authentication, &campaign_id, now_unix_ms)
            .map_err(|_| AgentJobError::terminal("AGENT_WORKLOAD_IDENTITY_INVALID"))?;
        drop(identity);

        let scope: VisibilityScope = serde_json::from_str(&job.visibility_scope_json)
            .map_err(|_| AgentJobError::terminal("RAG_VISIBILITY_SCOPE_INVALID"))?;
        let visibility =
            Visibility::try_from_parts(&scope.output_label, scope.subject_id.as_deref())
                .map_err(|_| AgentJobError::terminal("RAG_VISIBILITY_SCOPE_INVALID"))?;
        let context = AuthenticatedCommandContext::new(
            workflow_actor,
            ResourceRef::new(&job.campaign_id, "agent_turn", &job.input_stream_id)
                .map_err(|_| AgentJobError::terminal("AGENT_JOB_RESOURCE_INVALID"))?,
            contract
                .binding()
                .map_err(|_| AgentJobError::terminal("AGENT_JOB_AUTHORITY_SNAPSHOT_MISMATCH"))?,
            format!("trace_{}", job.job_id),
            now_unix_ms,
            expires_at_unix_ms,
        )
        .map_err(|_| AgentJobError::terminal("AGENT_COMMAND_CONTEXT_INVALID"))?;
        let tool_request =
            ToolRequest::formal(AgentKind::AiKeeperOrchestrator, AgentTool::NarrationOnly);
        let agent_decision = AgentDecision::new(
            format!("decision_{}", job.job_id),
            tool_request,
            &decision.player_visible_text,
            &agent_authentication,
        )
        .map_err(|error| AgentJobError::terminal(error.code()))?;
        let command = CommandEnvelope::new(
            agent_decision.clone(),
            CommandMetadata {
                command_id: EntityId::new(format!("command_{}", job.job_id))
                    .map_err(|_| AgentJobError::terminal("AGENT_COMMAND_ID_INVALID"))?,
                idempotency_key: job.idempotency_key.clone(),
                expected_version: u64::try_from(job.input_stream_version)
                    .map_err(|_| AgentJobError::terminal("AGENT_INPUT_VERSION_INVALID"))?,
                authority_mode: trpg_shared_kernel::AuthorityMode::AiKp,
                visibility,
                fact_provenance: FactProvenance {
                    kind: ProvenanceKind::AgentProposal,
                    reference: EntityId::new(format!(
                        "event_sequence_{}",
                        job.input_event_sequence
                    ))
                    .map_err(|_| AgentJobError::terminal("AGENT_PROVENANCE_INVALID"))?,
                    recorded_by: EntityId::new(&job.actor_id)
                        .map_err(|_| AgentJobError::terminal("AGENT_PROVENANCE_INVALID"))?,
                },
                correlation_id: EntityId::new(&job.job_id)
                    .map_err(|_| AgentJobError::terminal("AGENT_CORRELATION_INVALID"))?,
                causation_id: EntityId::new(format!("event_sequence_{}", job.input_event_sequence))
                    .map_err(|_| AgentJobError::terminal("AGENT_CAUSATION_INVALID"))?,
                write_path: FormalWritePath::WorkflowDecision,
                authenticated_context: context,
            },
        );
        let mut events = self
            .events
            .lock()
            .map_err(|_| AgentJobError::retryable("AGENT_EVENT_CUSTODY_LOCK_UNAVAILABLE"))?;
        let committed = self
            .committer
            .commit(
                &mut events,
                &command,
                &workflow_authentication,
                agent_decision,
                now_unix_ms,
            )
            .map_err(|error| AgentJobError::terminal(error.code()))?;
        let event_sequences = committed
            .into_iter()
            .map(|event| {
                i64::try_from(event.sequence)
                    .map_err(|_| AgentJobError::terminal("AGENT_CANONICAL_RECEIPT_INVALID"))
            })
            .collect::<AgentJobResult<Vec<_>>>()?;
        Ok(AgentJobCommitReceipt { event_sequences })
    }
}

#[async_trait]
pub trait AgentJobRepository: Send + Sync {
    async fn load(&self, job_id: &str) -> AgentJobResult<Option<DurableAgentJob>>;

    async fn claim_due(
        &self,
        claim_owner: &str,
        now_unix_ms: i64,
        lease_duration_ms: i64,
    ) -> AgentJobResult<Option<DurableAgentJob>>;

    async fn transition(&self, draft: &AgentJobTransitionDraft) -> AgentJobResult<DurableAgentJob>;

    async fn heartbeat(
        &self,
        job_id: &str,
        claim_owner: &str,
        claim_token: &str,
        now_unix_ms: i64,
        lease_duration_ms: i64,
    ) -> AgentJobResult<bool>;

    async fn cancellation_requested(&self, job_id: &str) -> AgentJobResult<bool>;

    async fn load_authority(
        &self,
        campaign_id: &str,
    ) -> AgentJobResult<DurableAgentAuthoritySnapshot>;

    async fn load_context(&self, job_id: &str) -> AgentJobResult<DurableAgentContextSnapshot>;

    async fn load_approval(&self, job_id: &str) -> AgentJobResult<Option<DurableAgentApproval>>;

    async fn append_evidence(&self, draft: &AgentJobEvidenceDraft) -> AgentJobResult<()>;
}

#[async_trait]
impl AgentJobRepository for DurableWorkflowStore {
    async fn load(&self, job_id: &str) -> AgentJobResult<Option<DurableAgentJob>> {
        self.load_agent_job(job_id).await.map_err(map_store_error)
    }

    async fn claim_due(
        &self,
        claim_owner: &str,
        now_unix_ms: i64,
        lease_duration_ms: i64,
    ) -> AgentJobResult<Option<DurableAgentJob>> {
        self.claim_due_agent_job(claim_owner, now_unix_ms, lease_duration_ms)
            .await
            .map_err(map_store_error)
    }

    async fn transition(&self, draft: &AgentJobTransitionDraft) -> AgentJobResult<DurableAgentJob> {
        self.transition_agent_job(draft)
            .await
            .map_err(map_store_error)
    }

    async fn heartbeat(
        &self,
        job_id: &str,
        claim_owner: &str,
        claim_token: &str,
        now_unix_ms: i64,
        lease_duration_ms: i64,
    ) -> AgentJobResult<bool> {
        self.heartbeat_agent_job(
            job_id,
            claim_owner,
            claim_token,
            now_unix_ms,
            lease_duration_ms,
        )
        .await
        .map_err(map_store_error)
    }

    async fn cancellation_requested(&self, job_id: &str) -> AgentJobResult<bool> {
        self.agent_job_cancellation_requested(job_id)
            .await
            .map_err(map_store_error)
    }

    async fn load_authority(
        &self,
        campaign_id: &str,
    ) -> AgentJobResult<DurableAgentAuthoritySnapshot> {
        self.load_agent_authority_snapshot(campaign_id)
            .await
            .map_err(map_store_error)
    }

    async fn load_context(&self, job_id: &str) -> AgentJobResult<DurableAgentContextSnapshot> {
        self.load_agent_job_context(job_id)
            .await
            .map_err(map_store_error)
    }

    async fn load_approval(&self, job_id: &str) -> AgentJobResult<Option<DurableAgentApproval>> {
        self.load_agent_job_approval(job_id)
            .await
            .map_err(map_store_error)
    }

    async fn append_evidence(&self, draft: &AgentJobEvidenceDraft) -> AgentJobResult<()> {
        self.append_agent_job_evidence(draft)
            .await
            .map_err(map_store_error)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AgentJobOutcome {
    Idle,
    Completed {
        job_id: String,
        event_sequences: Vec<i64>,
    },
    AwaitingHumanApproval {
        job_id: String,
    },
    RetryScheduled {
        job_id: String,
        error_code: &'static str,
    },
    TerminalFailure {
        job_id: String,
        error_code: &'static str,
    },
}

pub struct AgentJobWorker {
    repository: Arc<dyn AgentJobRepository>,
    provider: Arc<dyn ExecutableModelProvider>,
    tools: Arc<dyn AgentJobToolPort>,
    decisions: Arc<dyn AgentJobDecisionPort>,
    local_certification: Option<CertifiedLocalModel>,
    configuration: AgentJobExecutionConfig,
}

impl fmt::Debug for AgentJobWorker {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AgentJobWorker")
            .field("repository", &"[AGENT JOB REPOSITORY]")
            .field("provider", &self.provider.startup_route_snapshot())
            .field("tools", &"[GOVERNED TOOL PORT]")
            .field("decisions", &"[CANONICAL DECISION PORT]")
            .field("local_certification", &self.local_certification)
            .field("configuration", &self.configuration)
            .finish()
    }
}

impl AgentJobWorker {
    pub fn new(
        repository: Arc<dyn AgentJobRepository>,
        provider: Arc<dyn ExecutableModelProvider>,
        tools: Arc<dyn AgentJobToolPort>,
        decisions: Arc<dyn AgentJobDecisionPort>,
        local_certification: Option<CertifiedLocalModel>,
        configuration: AgentJobExecutionConfig,
    ) -> AgentJobResult<Self> {
        configuration.validate()?;
        Ok(Self {
            repository,
            provider,
            tools,
            decisions,
            local_certification,
            configuration,
        })
    }

    pub async fn run_once(&self, now_unix_ms: i64) -> AgentJobResult<AgentJobOutcome> {
        let lease_duration_ms = duration_millis_i64(self.configuration.lease_duration)?;
        let Some(job) = self
            .repository
            .claim_due(
                &self.configuration.claim_owner,
                now_unix_ms,
                lease_duration_ms,
            )
            .await?
        else {
            return Ok(AgentJobOutcome::Idle);
        };
        match self.execute_claimed(job.clone(), now_unix_ms).await {
            Ok(outcome) => Ok(outcome),
            Err(error) => {
                let current = self
                    .repository
                    .load(&job.job_id)
                    .await?
                    .ok_or_else(|| AgentJobError::terminal("AGENT_JOB_NOT_FOUND"))?;
                self.fail_job(current, error, now_unix_ms).await
            }
        }
    }

    async fn execute_claimed(
        &self,
        mut job: DurableAgentJob,
        now_unix_ms: i64,
    ) -> AgentJobResult<AgentJobOutcome> {
        self.ensure_active(&job, now_unix_ms).await?;
        if job.attempt > self.configuration.max_attempts {
            return Err(AgentJobError::terminal("AGENT_JOB_ATTEMPT_LIMIT_EXCEEDED"));
        }
        let authority = self.repository.load_authority(&job.campaign_id).await?;
        validate_authority_snapshot(&job, &authority)?;
        self.decisions
            .authorize_execution(&job, now_unix_ms)
            .await?;
        let resume_state = job.resume_state.unwrap_or(WorkflowState::Requested);
        match resume_state {
            WorkflowState::Committing => {
                job = self
                    .transition_claimed_to(&job, WorkflowState::Committing, now_unix_ms)
                    .await?;
                self.commit_persisted(job, now_unix_ms).await
            }
            WorkflowState::AwaitingTool => {
                job = self
                    .transition_claimed_to(&job, WorkflowState::AwaitingTool, now_unix_ms)
                    .await?;
                self.continue_persisted_decision(job, now_unix_ms).await
            }
            WorkflowState::Requested
            | WorkflowState::Claimed
            | WorkflowState::AgentRunning
            | WorkflowState::RetryableFailed => {
                job = self
                    .transition_claimed_to(&job, WorkflowState::AgentRunning, now_unix_ms)
                    .await?;
                self.run_model(job, now_unix_ms).await
            }
            _ => Err(AgentJobError::terminal("AGENT_JOB_RESUME_STATE_INVALID")),
        }
    }

    async fn transition_claimed_to(
        &self,
        job: &DurableAgentJob,
        target: WorkflowState,
        now_unix_ms: i64,
    ) -> AgentJobResult<DurableAgentJob> {
        self.repository
            .transition(&self.transition_draft(
                job,
                WorkflowState::Claimed,
                target,
                phase_id(target),
                None,
                None,
                None,
                None,
                now_unix_ms,
            )?)
            .await
    }

    async fn run_model(
        &self,
        mut job: DurableAgentJob,
        now_unix_ms: i64,
    ) -> AgentJobResult<AgentJobOutcome> {
        self.validate_provider_binding(&job)?;
        self.ensure_local_model_certification(&job)?;
        let empty_hash = EMPTY_SHA256.to_owned();
        self.append_evidence(
            &job,
            "authority",
            &empty_hash,
            &empty_hash,
            &empty_hash,
            &empty_hash,
            &empty_hash,
            0,
            0,
            0,
            0,
            &[],
            now_unix_ms,
        )
        .await?;

        let context = self.repository.load_context(&job.job_id).await?;
        let visibility_scope = validate_context_scope(&job, &context)?;
        let (request, hashes) = build_model_request(
            &job,
            &context,
            &visibility_scope,
            self.configuration.max_context_bytes,
        )?;
        self.append_evidence(
            &job,
            "context",
            &hashes.prompt_template_hash,
            &hashes.tool_schema_hash,
            &hashes.retrieval_hash,
            &hashes.input_hash,
            &empty_hash,
            0,
            0,
            0,
            0,
            &[],
            now_unix_ms,
        )
        .await?;

        let started = Instant::now();
        let cancellation = ProviderCancellation::default();
        let execution = self
            .execute_provider_with_lease(&job, &request, &cancellation, now_unix_ms)
            .await?;
        let latency_ms = i64::try_from(started.elapsed().as_millis())
            .map_err(|_| AgentJobError::terminal("AGENT_JOB_LATENCY_OVERFLOW"))?;
        if execution.output.usage.input_tokens > self.configuration.max_input_tokens
            || execution.output.usage.output_tokens > self.configuration.max_output_tokens
            || execution.output.tool_calls.len() > self.configuration.max_tool_calls
        {
            return Err(AgentJobError::terminal("AGENT_JOB_BUDGET_EXCEEDED"));
        }
        let decision = validate_structured_decision(
            execution.output.structured_output.as_ref(),
            &execution.output.tool_calls,
            self.configuration.max_tool_calls,
        )?;
        let injection =
            evaluate_prompt_injection(&context.input_payload_json, &decision.player_visible_text);
        if injection.detected {
            return Err(AgentJobError::terminal("PROMPT_INJECTION_DETECTED"));
        }
        let decision_json = serde_json::to_string(&decision)
            .map_err(|_| AgentJobError::terminal("AGENT_OUTPUT_SCHEMA_INVALID"))?;
        let output_hash = sha256_label(decision_json.as_bytes());
        self.append_evidence(
            &job,
            "provider",
            &hashes.prompt_template_hash,
            &hashes.tool_schema_hash,
            &hashes.retrieval_hash,
            &hashes.input_hash,
            &output_hash,
            i64::try_from(execution.output.usage.input_tokens)
                .map_err(|_| AgentJobError::terminal("AGENT_JOB_BUDGET_EXCEEDED"))?,
            i64::try_from(execution.output.usage.output_tokens)
                .map_err(|_| AgentJobError::terminal("AGENT_JOB_BUDGET_EXCEEDED"))?,
            latency_ms,
            i32::try_from(decision.tool.iter().count())
                .map_err(|_| AgentJobError::terminal("AGENT_JOB_BUDGET_EXCEEDED"))?,
            &[],
            observed_now(now_unix_ms, started)?,
        )
        .await?;
        job = self
            .repository
            .transition(&self.transition_draft(
                &job,
                WorkflowState::AgentRunning,
                WorkflowState::AwaitingTool,
                "provider_output",
                Some(decision_json),
                None,
                None,
                None,
                observed_now(now_unix_ms, started)?,
            )?)
            .await?;
        self.continue_persisted_decision(job, observed_now(now_unix_ms, started)?)
            .await
    }

    async fn continue_persisted_decision(
        &self,
        mut job: DurableAgentJob,
        now_unix_ms: i64,
    ) -> AgentJobResult<AgentJobOutcome> {
        self.ensure_active(&job, now_unix_ms).await?;
        let decision = persisted_decision(&job)?;
        if job.authority_mode == "HUMAN_KP" {
            let Some(approval) = self.repository.load_approval(&job.job_id).await? else {
                return Ok(AgentJobOutcome::AwaitingHumanApproval { job_id: job.job_id });
            };
            job = self
                .repository
                .transition(&self.transition_draft(
                    &job,
                    WorkflowState::AwaitingTool,
                    WorkflowState::Committing,
                    "human_approval",
                    None,
                    None,
                    Some(vec![approval.approval_event_sequence]),
                    None,
                    now_unix_ms,
                )?)
                .await?;
            return self
                .complete_human_approval(job, approval, now_unix_ms)
                .await;
        }

        let tool_result = if let Some(call) = decision.tool.as_ref() {
            if self.configuration.max_tool_loops < 1 {
                return Err(AgentJobError::terminal("AGENT_TOOL_LOOP_LIMIT_EXCEEDED"));
            }
            let tool = parse_agent_tool(&call.name)?;
            let request = ToolRequest::formal(AgentKind::AiKeeperOrchestrator, tool);
            let gate =
                evaluate_agent_tool_request(&trpg_shared_kernel::AuthorityMode::AiKp, &request);
            if !gate.tool_authorized || gate.draft_only || gate.requires_human_confirmation {
                return Err(AgentJobError::terminal("AGENT_TOOL_PERMISSION_DENIED"));
            }
            let result = self
                .tools
                .execute(&job, call, &format!("{}:tool", job.idempotency_key))
                .await?;
            validate_tool_result(&result)?;
            let result_json = serde_json::to_string(&result)
                .map_err(|_| AgentJobError::terminal("AGENT_TOOL_RESULT_INVALID"))?;
            self.append_evidence(
                &job,
                "tool",
                EMPTY_SHA256,
                EMPTY_SHA256,
                EMPTY_SHA256,
                EMPTY_SHA256,
                &sha256_label(result_json.as_bytes()),
                0,
                0,
                0,
                1,
                &[],
                now_unix_ms,
            )
            .await?;
            Some((result, result_json))
        } else {
            None
        };
        job = self
            .repository
            .transition(&self.transition_draft(
                &job,
                WorkflowState::AwaitingTool,
                WorkflowState::Committing,
                "tool_complete",
                None,
                tool_result.as_ref().map(|(_, json)| json.clone()),
                None,
                None,
                now_unix_ms,
            )?)
            .await?;
        self.commit_persisted(job, now_unix_ms).await
    }

    async fn commit_persisted(
        &self,
        mut job: DurableAgentJob,
        now_unix_ms: i64,
    ) -> AgentJobResult<AgentJobOutcome> {
        self.ensure_active(&job, now_unix_ms).await?;
        let authority = self.repository.load_authority(&job.campaign_id).await?;
        validate_authority_snapshot(&job, &authority)?;
        let decision = persisted_decision(&job)?;
        let tool_result = job
            .tool_result_json
            .as_deref()
            .map(serde_json::from_str::<AgentJobToolResult>)
            .transpose()
            .map_err(|_| AgentJobError::terminal("AGENT_TOOL_RESULT_INVALID"))?;
        let receipt = self
            .decisions
            .commit_ai_decision(&job, &decision, tool_result.as_ref(), now_unix_ms)
            .await?;
        if receipt.event_sequences.is_empty()
            || receipt
                .event_sequences
                .windows(2)
                .any(|window| window[0] >= window[1])
        {
            return Err(AgentJobError::terminal("AGENT_CANONICAL_RECEIPT_INVALID"));
        }
        self.append_evidence(
            &job,
            "canonical_commit",
            EMPTY_SHA256,
            EMPTY_SHA256,
            EMPTY_SHA256,
            &sha256_label(job.decision_json.as_deref().unwrap_or_default().as_bytes()),
            EMPTY_SHA256,
            0,
            0,
            0,
            i32::from(tool_result.is_some()),
            &receipt.event_sequences,
            now_unix_ms,
        )
        .await?;
        self.append_evidence(
            &job,
            "completed",
            EMPTY_SHA256,
            EMPTY_SHA256,
            EMPTY_SHA256,
            EMPTY_SHA256,
            EMPTY_SHA256,
            0,
            0,
            0,
            0,
            &receipt.event_sequences,
            now_unix_ms,
        )
        .await?;
        job = self
            .repository
            .transition(&self.transition_draft(
                &job,
                WorkflowState::Committing,
                WorkflowState::Completed,
                "completed",
                None,
                None,
                Some(receipt.event_sequences.clone()),
                None,
                now_unix_ms,
            )?)
            .await?;
        Ok(AgentJobOutcome::Completed {
            job_id: job.job_id,
            event_sequences: receipt.event_sequences,
        })
    }

    async fn complete_human_approval(
        &self,
        mut job: DurableAgentJob,
        approval: DurableAgentApproval,
        now_unix_ms: i64,
    ) -> AgentJobResult<AgentJobOutcome> {
        let sequences = vec![approval.approval_event_sequence];
        self.append_evidence(
            &job,
            "completed",
            EMPTY_SHA256,
            EMPTY_SHA256,
            EMPTY_SHA256,
            EMPTY_SHA256,
            EMPTY_SHA256,
            0,
            0,
            0,
            0,
            &sequences,
            now_unix_ms,
        )
        .await?;
        job = self
            .repository
            .transition(&self.transition_draft(
                &job,
                WorkflowState::Committing,
                WorkflowState::Completed,
                "human_approval_completed",
                None,
                None,
                Some(sequences.clone()),
                None,
                now_unix_ms,
            )?)
            .await?;
        Ok(AgentJobOutcome::Completed {
            job_id: job.job_id,
            event_sequences: sequences,
        })
    }

    async fn execute_provider_with_lease(
        &self,
        job: &DurableAgentJob,
        request: &ModelChatRequest,
        cancellation: &ProviderCancellation,
        started_unix_ms: i64,
    ) -> AgentJobResult<
        crate::model_provider::ProviderExecution<crate::model_provider::ModelChatResponse>,
    > {
        let deadline_delay_ms = job
            .deadline_unix_ms
            .checked_sub(started_unix_ms)
            .filter(|remaining| *remaining > 0)
            .and_then(|remaining| u64::try_from(remaining).ok())
            .ok_or_else(|| AgentJobError::terminal("AGENT_JOB_DEADLINE_EXCEEDED"))?;
        let deadline = tokio::time::sleep(Duration::from_millis(deadline_delay_ms));
        tokio::pin!(deadline);
        let started = Instant::now();
        let mut capability_probe = Box::pin(self.provider.probe_capabilities(cancellation));
        let capabilities = loop {
            tokio::select! {
                biased;
                _ = &mut deadline => {
                    cancellation.cancel();
                    return Err(AgentJobError::terminal("AGENT_JOB_DEADLINE_EXCEEDED"));
                }
                result = &mut capability_probe => {
                    let capabilities = result.map_err(map_provider_error)?;
                    self.refresh_provider_lease(
                        job,
                        cancellation,
                        started_unix_ms,
                        started,
                    ).await?;
                    break capabilities;
                }
                _ = tokio::time::sleep(self.configuration.heartbeat_interval) => {
                    self.refresh_provider_lease(
                        job,
                        cancellation,
                        started_unix_ms,
                        started,
                    ).await?;
                }
            }
        };
        self.validate_executed_route(job, &capabilities.route, ModelOperation::CapabilityProbe)?;
        if !capabilities.output.chat
            || !capabilities.output.structured_output
            || !capabilities.output.tool_requests
        {
            return Err(AgentJobError::terminal(
                "MODEL_PROVIDER_CAPABILITY_REQUIRED",
            ));
        }

        let mut execution = Box::pin(self.provider.chat(request, cancellation));
        loop {
            tokio::select! {
                biased;
                _ = &mut deadline => {
                    cancellation.cancel();
                    return Err(AgentJobError::terminal("AGENT_JOB_DEADLINE_EXCEEDED"));
                }
                result = &mut execution => {
                    let result = result.map_err(map_provider_error)?;
                    self.refresh_provider_lease(
                        job,
                        cancellation,
                        started_unix_ms,
                        started,
                    ).await?;
                    self.validate_executed_route(job, &result.route, ModelOperation::Chat)?;
                    return Ok(result);
                }
                _ = tokio::time::sleep(self.configuration.heartbeat_interval) => {
                    self.refresh_provider_lease(
                        job,
                        cancellation,
                        started_unix_ms,
                        started,
                    ).await?;
                }
            }
        }
    }

    async fn refresh_provider_lease(
        &self,
        job: &DurableAgentJob,
        cancellation: &ProviderCancellation,
        started_unix_ms: i64,
        started: Instant,
    ) -> AgentJobResult<()> {
        let now_unix_ms = observed_now(started_unix_ms, started)?;
        if now_unix_ms >= job.deadline_unix_ms {
            cancellation.cancel();
            return Err(AgentJobError::terminal("AGENT_JOB_DEADLINE_EXCEEDED"));
        }
        if self.repository.cancellation_requested(&job.job_id).await? {
            cancellation.cancel();
            return Err(AgentJobError::terminal("AGENT_JOB_CANCELLED"));
        }
        let lease_duration_ms = duration_millis_i64(self.configuration.lease_duration)?;
        let claim_owner = job
            .claim_owner
            .as_deref()
            .ok_or_else(|| AgentJobError::retryable("AGENT_JOB_LEASE_LOST"))?;
        let claim_token = job
            .claim_token
            .as_deref()
            .ok_or_else(|| AgentJobError::retryable("AGENT_JOB_LEASE_LOST"))?;
        if !self
            .repository
            .heartbeat(
                &job.job_id,
                claim_owner,
                claim_token,
                now_unix_ms,
                lease_duration_ms,
            )
            .await?
        {
            cancellation.cancel();
            return Err(AgentJobError::retryable("AGENT_JOB_LEASE_LOST"));
        }
        Ok(())
    }

    async fn ensure_active(&self, job: &DurableAgentJob, now_unix_ms: i64) -> AgentJobResult<()> {
        if now_unix_ms >= job.deadline_unix_ms {
            return Err(AgentJobError::terminal("AGENT_JOB_DEADLINE_EXCEEDED"));
        }
        if self.repository.cancellation_requested(&job.job_id).await? {
            return Err(AgentJobError::terminal("AGENT_JOB_CANCELLED"));
        }
        Ok(())
    }

    fn validate_provider_binding(&self, job: &DurableAgentJob) -> AgentJobResult<()> {
        let route = self.provider.startup_route_snapshot();
        if self.provider.provider_id().as_str() != job.provider_id
            || provider_type_name(self.provider.provider_type()) != job.provider_type
            || self.provider.model_id() != job.model_id
            || self.provider.model_artifact_sha256() != job.model_artifact_sha256
        {
            return Err(AgentJobError::terminal(
                "AGENT_JOB_PROVIDER_BINDING_MISMATCH",
            ));
        }
        self.validate_executed_route(job, &route, ModelOperation::CapabilityProbe)
    }

    fn validate_executed_route(
        &self,
        job: &DurableAgentJob,
        route: &ExecutedModelRouteSnapshot,
        operation: ModelOperation,
    ) -> AgentJobResult<()> {
        if route.route_authorization_event_id.as_str() != job.route_authorization_event_id
            || route.provider_id.as_str() != job.provider_id
            || provider_type_name(route.provider_type) != job.provider_type
            || route.model_id != job.model_id
            || route.operation != operation
            || route.fallback_policy != "none_no_automatic_fallback"
            || route.privacy_boundary != "explicit_route_authorization_event"
        {
            return Err(AgentJobError::terminal(
                "AGENT_JOB_PROVIDER_BINDING_MISMATCH",
            ));
        }
        Ok(())
    }

    fn ensure_local_model_certification(&self, job: &DurableAgentJob) -> AgentJobResult<()> {
        if job.authority_mode == "AI_KP" && job.provider_type != "cloud" {
            self.local_certification
                .as_ref()
                .ok_or_else(|| AgentJobError::terminal("LOCAL_MODEL_LEVEL_4_REQUIRED"))?
                .ensure_ai_keeper(&job.model_id, &job.model_artifact_sha256)?;
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    async fn append_evidence(
        &self,
        job: &DurableAgentJob,
        phase: &str,
        prompt_template_hash: &str,
        tool_schema_hash: &str,
        retrieval_hash: &str,
        input_hash: &str,
        output_hash: &str,
        input_tokens: i64,
        output_tokens: i64,
        latency_ms: i64,
        tool_call_count: i32,
        linked_event_sequences: &[i64],
        now_unix_ms: i64,
    ) -> AgentJobResult<()> {
        let retention_until_unix_ms = now_unix_ms
            .checked_add(EVIDENCE_RETENTION_MS)
            .ok_or_else(|| AgentJobError::terminal("AGENT_EVIDENCE_RETENTION_INVALID"))?;
        let visibility_scope: VisibilityScope = serde_json::from_str(&job.visibility_scope_json)
            .map_err(|_| AgentJobError::terminal("RAG_VISIBILITY_SCOPE_INVALID"))?;
        self.repository
            .append_evidence(&AgentJobEvidenceDraft {
                job_id: job.job_id.clone(),
                attempt: job.attempt,
                phase: phase.to_owned(),
                model_id: job.model_id.clone(),
                runtime_version: AGENT_RUNTIME_VERSION.to_owned(),
                prompt_template_hash: prompt_template_hash.to_owned(),
                tool_schema_hash: tool_schema_hash.to_owned(),
                retrieval_hash: retrieval_hash.to_owned(),
                input_hash: input_hash.to_owned(),
                output_hash: output_hash.to_owned(),
                input_tokens,
                output_tokens,
                latency_ms,
                tool_call_count,
                linked_event_sequences: linked_event_sequences.to_vec(),
                visibility_label: visibility_scope.output_label,
                retention_until_unix_ms,
            })
            .await
    }

    async fn fail_job(
        &self,
        job: DurableAgentJob,
        error: AgentJobError,
        now_unix_ms: i64,
    ) -> AgentJobResult<AgentJobOutcome> {
        let target = if error.is_retryable() && job.attempt < self.configuration.max_attempts {
            WorkflowState::RetryableFailed
        } else {
            WorkflowState::TerminalFailed
        };
        let next_attempt_at = if target == WorkflowState::RetryableFailed {
            Some(
                now_unix_ms
                    .checked_add(retry_delay_ms(job.attempt))
                    .ok_or_else(|| AgentJobError::terminal("AGENT_JOB_RETRY_INVALID"))?,
            )
        } else {
            None
        };
        let current_state = if job.state == WorkflowState::Claimed {
            WorkflowState::Claimed
        } else {
            job.state
        };
        if agent_state_can_fail(current_state) {
            let failed = self
                .repository
                .transition(
                    &self
                        .transition_draft(
                            &job,
                            current_state,
                            target,
                            "failure",
                            None,
                            None,
                            None,
                            Some(error.code()),
                            now_unix_ms,
                        )?
                        .with_next_attempt(next_attempt_at),
                )
                .await?;
            let _ = self
                .append_evidence(
                    &failed,
                    "failed",
                    EMPTY_SHA256,
                    EMPTY_SHA256,
                    EMPTY_SHA256,
                    EMPTY_SHA256,
                    EMPTY_SHA256,
                    0,
                    0,
                    0,
                    0,
                    &[],
                    now_unix_ms,
                )
                .await;
        }
        Ok(if target == WorkflowState::RetryableFailed {
            AgentJobOutcome::RetryScheduled {
                job_id: job.job_id,
                error_code: error.code(),
            }
        } else {
            AgentJobOutcome::TerminalFailure {
                job_id: job.job_id,
                error_code: error.code(),
            }
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn transition_draft(
        &self,
        job: &DurableAgentJob,
        from_state: WorkflowState,
        to_state: WorkflowState,
        phase: &str,
        decision_json: Option<String>,
        tool_result_json: Option<String>,
        linked_event_sequences: Option<Vec<i64>>,
        error_code: Option<&'static str>,
        now_unix_ms: i64,
    ) -> AgentJobResult<AgentJobTransitionDraft> {
        Ok(AgentJobTransitionDraft {
            job_id: job.job_id.clone(),
            claim_owner: job
                .claim_owner
                .clone()
                .ok_or_else(|| AgentJobError::retryable("AGENT_JOB_LEASE_LOST"))?,
            claim_token: job
                .claim_token
                .clone()
                .ok_or_else(|| AgentJobError::retryable("AGENT_JOB_LEASE_LOST"))?,
            expected_version: job.version,
            from_state,
            to_state,
            idempotency_key: format!("{}:attempt:{}:{}", job.idempotency_key, job.attempt, phase),
            correlation_id: job.job_id.clone(),
            causation_id: format!("agent-job-input-{}", job.input_event_sequence),
            decision_json,
            tool_result_json,
            linked_event_sequences,
            error_code: error_code.map(str::to_owned),
            next_attempt_at_unix_ms: None,
            now_unix_ms,
        })
    }
}

trait AgentJobTransitionDraftExt {
    fn with_next_attempt(self, next_attempt_at_unix_ms: Option<i64>) -> Self;
}

impl AgentJobTransitionDraftExt for AgentJobTransitionDraft {
    fn with_next_attempt(mut self, next_attempt_at_unix_ms: Option<i64>) -> Self {
        self.next_attempt_at_unix_ms = next_attempt_at_unix_ms;
        self
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct VisibilityScope {
    allowed_labels: Vec<String>,
    subject_id: Option<String>,
    output_label: String,
}

struct ModelRequestHashes {
    prompt_template_hash: String,
    tool_schema_hash: String,
    retrieval_hash: String,
    input_hash: String,
}

fn validate_authority_snapshot(
    job: &DurableAgentJob,
    authority: &DurableAgentAuthoritySnapshot,
) -> AgentJobResult<()> {
    if authority.contract_id != job.authority_contract_id
        || authority.campaign_id != job.campaign_id
        || authority.authority_mode != job.authority_mode
        || authority.contract_version != job.authority_contract_version
        || authority.prompt_version != job.prompt_template_version
        || authority.tool_schema_version != job.tool_schema_version
        || authority.model_route_snapshot.trim().is_empty()
        || authority.agent_pack_version.trim().is_empty()
        || (job.authority_mode == "AI_KP"
            && (job.agent_kind != "ai_keeper_orchestrator"
                || job.actor_id != authority.authority_owner))
        || (job.authority_mode == "HUMAN_KP" && job.agent_kind != "keeper_copilot")
    {
        return Err(AgentJobError::terminal(
            "AGENT_JOB_AUTHORITY_SNAPSHOT_MISMATCH",
        ));
    }
    Ok(())
}

fn validate_context_scope(
    job: &DurableAgentJob,
    context: &DurableAgentContextSnapshot,
) -> AgentJobResult<VisibilityScope> {
    let scope: VisibilityScope = serde_json::from_str(&job.visibility_scope_json)
        .map_err(|_| AgentJobError::terminal("RAG_VISIBILITY_SCOPE_INVALID"))?;
    let known = [
        "public",
        "party_visible",
        "private_to_player",
        "private_to_group",
        "keeper_only",
        "ai_internal",
        "system_only",
        "spectator_visible",
        "spectator_hidden",
        "investigator_private",
        "system_private",
    ];
    let labels = scope.allowed_labels.iter().collect::<HashSet<_>>();
    if scope.allowed_labels.is_empty()
        || labels.len() != scope.allowed_labels.len()
        || scope.allowed_labels.len() > known.len()
        || scope
            .allowed_labels
            .iter()
            .any(|label| !known.contains(&label.as_str()))
        || !scope.allowed_labels.contains(&scope.output_label)
        || Visibility::try_from_parts(&scope.output_label, scope.subject_id.as_deref()).is_err()
    {
        return Err(AgentJobError::terminal("RAG_VISIBILITY_SCOPE_INVALID"));
    }
    if context.chunks.iter().any(|chunk| {
        !labels.contains(&chunk.visibility_label)
            || (chunk.visibility_subject.is_some() && chunk.visibility_subject != scope.subject_id)
            || !valid_plain_hash(&chunk.chunk_hash)
            || !valid_provenance(&chunk.fact_provenance_json)
    }) {
        return Err(AgentJobError::terminal("RAG_VISIBILITY_SCOPE_VIOLATION"));
    }
    serde_json::from_str::<Value>(&context.input_payload_json)
        .map_err(|_| AgentJobError::terminal("AGENT_JOB_INPUT_EVENT_INVALID"))?;
    Ok(scope)
}

fn build_model_request(
    job: &DurableAgentJob,
    context: &DurableAgentContextSnapshot,
    scope: &VisibilityScope,
    max_context_bytes: usize,
) -> AgentJobResult<(ModelChatRequest, ModelRequestHashes)> {
    let system = format!(
        "COC7 agent runtime. template={}:{}; authority={}; output_visibility={}; \
         Return only the requested structured decision. Never invent dice, \
         mutate state directly, reveal hidden facts, or bypass a tool.",
        job.prompt_template_id, job.prompt_template_version, job.authority_mode, scope.output_label,
    );
    let retrieved = context
        .chunks
        .iter()
        .map(|chunk| {
            format!(
                "[chunk:{} source:{} visibility:{} hash:{}]\n{}",
                chunk.chunk_id,
                chunk.source_event_sequence,
                chunk.visibility_label,
                chunk.chunk_hash,
                chunk.content
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    let user = format!(
        "Canonical input event:\n{}\n\nVisible retrieval:\n{}",
        context.input_payload_json, retrieved
    );
    let total_bytes = system
        .len()
        .checked_add(user.len())
        .ok_or_else(|| AgentJobError::terminal("AGENT_JOB_CONTEXT_BUDGET_EXCEEDED"))?;
    if total_bytes > max_context_bytes {
        return Err(AgentJobError::terminal("AGENT_JOB_CONTEXT_BUDGET_EXCEEDED"));
    }
    let structured_schema = json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["kind", "player_visible_text", "tool"],
        "properties": {
            "kind": {"const": "npc_turn"},
            "player_visible_text": {"type": "string", "minLength": 1, "maxLength": 16384},
            "tool": {
                "anyOf": [
                    {"type": "null"},
                    {
                        "type": "object",
                        "additionalProperties": false,
                        "required": ["name", "arguments"],
                        "properties": {
                            "name": {
                                "enum": [
                                    "request_skill_check", "reveal_clue",
                                    "apply_san_loss", "change_scene"
                                ]
                            },
                            "arguments": {"type": "object"}
                        }
                    }
                ]
            }
        }
    });
    let tool_schema = json!({
        "type": "object",
        "additionalProperties": false
    });
    let tools = [
        (
            "request_skill_check",
            "Request a server-side COC7 skill check",
        ),
        (
            "reveal_clue",
            "Reveal an authorized clue through the rules workflow",
        ),
        ("apply_san_loss", "Apply rules-engine validated sanity loss"),
        ("change_scene", "Request a governed scene transition"),
    ]
    .into_iter()
    .map(|(name, description)| ModelToolDefinition {
        name: name.to_owned(),
        description: description.to_owned(),
        input_schema: tool_schema.clone(),
    })
    .collect::<Vec<_>>();
    let request = ModelChatRequest {
        messages: vec![
            ModelMessage {
                role: ModelMessageRole::System,
                content: system.clone(),
            },
            ModelMessage {
                role: ModelMessageRole::User,
                content: user.clone(),
            },
        ],
        structured_output: Some(StructuredOutputRequest {
            name: "agent_npc_turn".to_owned(),
            schema: structured_schema.clone(),
        }),
        tools,
    };
    let retrieval_binding = context
        .chunks
        .iter()
        .map(|chunk| {
            format!(
                "{}:{}:{}",
                chunk.chunk_id, chunk.source_event_sequence, chunk.chunk_hash
            )
        })
        .collect::<Vec<_>>()
        .join("|");
    Ok((
        request,
        ModelRequestHashes {
            prompt_template_hash: sha256_label(
                format!(
                    "{}:{}:{}",
                    job.prompt_template_id, job.prompt_template_version, system
                )
                .as_bytes(),
            ),
            tool_schema_hash: sha256_label(
                serde_json::to_string(&(structured_schema, tool_schema))
                    .map_err(|_| AgentJobError::terminal("AGENT_TOOL_SCHEMA_INVALID"))?
                    .as_bytes(),
            ),
            retrieval_hash: sha256_label(retrieval_binding.as_bytes()),
            input_hash: sha256_label(user.as_bytes()),
        },
    ))
}

fn validate_structured_decision(
    value: Option<&Value>,
    provider_tool_calls: &[crate::model_provider::ModelToolCall],
    max_tool_calls: usize,
) -> AgentJobResult<AgentStructuredDecision> {
    if provider_tool_calls.len() > max_tool_calls {
        return Err(AgentJobError::terminal("AGENT_TOOL_CALL_LIMIT_EXCEEDED"));
    }
    let value = value.ok_or_else(|| AgentJobError::terminal("AGENT_OUTPUT_SCHEMA_INVALID"))?;
    let mut decision: AgentStructuredDecision = serde_json::from_value(value.clone())
        .map_err(|_| AgentJobError::terminal("AGENT_OUTPUT_SCHEMA_INVALID"))?;
    if decision.kind != "npc_turn"
        || decision.player_visible_text.trim().is_empty()
        || decision.player_visible_text.len() > 16_384
        || decision
            .tool
            .as_ref()
            .is_some_and(|tool| !tool.arguments.is_object())
    {
        return Err(AgentJobError::terminal("AGENT_OUTPUT_SCHEMA_INVALID"));
    }
    if let Some(provider_call) = provider_tool_calls.first() {
        let provider_tool = AgentJobToolCall {
            name: provider_call.name.clone(),
            arguments: provider_call.arguments.clone(),
        };
        if decision
            .tool
            .as_ref()
            .is_some_and(|structured| structured != &provider_tool)
        {
            return Err(AgentJobError::terminal("AGENT_OUTPUT_SCHEMA_INVALID"));
        }
        decision.tool = Some(provider_tool);
    }
    if decision.tool.iter().count() > max_tool_calls {
        return Err(AgentJobError::terminal("AGENT_TOOL_CALL_LIMIT_EXCEEDED"));
    }
    Ok(decision)
}

fn persisted_decision(job: &DurableAgentJob) -> AgentJobResult<AgentStructuredDecision> {
    serde_json::from_str(
        job.decision_json
            .as_deref()
            .ok_or_else(|| AgentJobError::terminal("AGENT_OUTPUT_SCHEMA_INVALID"))?,
    )
    .map_err(|_| AgentJobError::terminal("AGENT_OUTPUT_SCHEMA_INVALID"))
}

fn parse_agent_tool(name: &str) -> AgentJobResult<AgentTool> {
    match name {
        "request_skill_check" => Ok(AgentTool::RequestSkillCheck),
        "reveal_clue" => Ok(AgentTool::RevealClue),
        "apply_san_loss" => Ok(AgentTool::ApplySanLoss),
        "change_scene" => Ok(AgentTool::ChangeScene),
        _ => Err(AgentJobError::terminal("AGENT_TOOL_PERMISSION_DENIED")),
    }
}

fn validate_tool_result(result: &AgentJobToolResult) -> AgentJobResult<()> {
    if result.execution_id.trim().is_empty()
        || !result
            .result_hash
            .strip_prefix("sha256:")
            .is_some_and(valid_plain_hash)
    {
        return Err(AgentJobError::terminal("AGENT_TOOL_RESULT_INVALID"));
    }
    Ok(())
}

fn valid_provenance(value: &str) -> bool {
    serde_json::from_str::<Value>(value)
        .ok()
        .and_then(|value| value.as_object().cloned())
        .is_some_and(|object| {
            !object.is_empty() && (object.contains_key("kind") || object.contains_key("source"))
        })
}

fn valid_plain_hash(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn provider_type_name(provider_type: ProviderType) -> &'static str {
    match provider_type {
        ProviderType::Cloud => "cloud",
        ProviderType::Ollama => "ollama",
        ProviderType::LlamaCpp => "llama_cpp",
        ProviderType::LocalOpenAiCompatible => "local_openai_compatible",
    }
}

fn agent_state_can_fail(state: WorkflowState) -> bool {
    matches!(
        state,
        WorkflowState::Claimed
            | WorkflowState::AgentRunning
            | WorkflowState::AwaitingTool
            | WorkflowState::Committing
    )
}

fn map_provider_error(error: ModelProviderError) -> AgentJobError {
    if error.retryable() {
        return AgentJobError::retryable(error.code());
    }
    match error.kind() {
        ModelProviderErrorKind::Cancelled => AgentJobError::terminal("AGENT_JOB_CANCELLED"),
        _ => AgentJobError::terminal(error.code()),
    }
}

fn map_store_error(error: WorkflowStoreError) -> AgentJobError {
    match error {
        WorkflowStoreError::Connection
        | WorkflowStoreError::Database(_)
        | WorkflowStoreError::VersionConflict { .. }
        | WorkflowStoreError::StateConflict => {
            AgentJobError::retryable("AGENT_JOB_STORE_RETRYABLE")
        }
        WorkflowStoreError::NotFound => AgentJobError::terminal("AGENT_JOB_NOT_FOUND"),
        WorkflowStoreError::IdempotencyConflict => {
            AgentJobError::terminal("AGENT_JOB_IDEMPOTENCY_CONFLICT")
        }
        WorkflowStoreError::Configuration(_)
        | WorkflowStoreError::Migration
        | WorkflowStoreError::Validation(_)
        | WorkflowStoreError::IntegrityViolation(_) => {
            AgentJobError::terminal("AGENT_JOB_STORE_INTEGRITY_FAILURE")
        }
    }
}

fn phase_id(state: WorkflowState) -> &'static str {
    match state {
        WorkflowState::AgentRunning => "resume_running",
        WorkflowState::AwaitingTool => "resume_awaiting_tool",
        WorkflowState::Committing => "resume_committing",
        _ => "resume",
    }
}

fn retry_delay_ms(attempt: i32) -> i64 {
    let shift = u32::try_from(attempt.clamp(0, 6)).unwrap_or(0);
    1_000_i64.saturating_mul(1_i64 << shift)
}

fn duration_millis_i64(duration: Duration) -> AgentJobResult<i64> {
    i64::try_from(duration.as_millis())
        .map_err(|_| AgentJobError::terminal("AGENT_JOB_CONFIGURATION_INVALID"))
}

fn observed_now(started_unix_ms: i64, started: Instant) -> AgentJobResult<i64> {
    started_unix_ms
        .checked_add(
            i64::try_from(started.elapsed().as_millis())
                .map_err(|_| AgentJobError::terminal("AGENT_JOB_TIME_OVERFLOW"))?,
        )
        .ok_or_else(|| AgentJobError::terminal("AGENT_JOB_TIME_OVERFLOW"))
}

fn sha256_label(value: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(value))
}
