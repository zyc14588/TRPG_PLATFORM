use std::collections::{HashMap, HashSet};
use std::fmt;
use std::future::Future;
use std::pin::Pin;
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
    AgentJobEvidenceDraft, AgentJobSkillCheckDraft, AgentJobSkillCheckRollDraft,
    AgentJobTransitionDraft, DurableAgentApproval, DurableAgentAuthoritySnapshot,
    DurableAgentContextSnapshot, DurableAgentJob, DurableWorkflowStore, WorkflowState,
    WorkflowStoreError,
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
    AgentError, AgentEventPayload, AgentKind, AgentTool, AgentToolExecutionOutput,
    AgentToolExecutor, EventStore as AgentEventStore, ToolRequest,
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

    fn ensure_ai_keeper(&self, provider: &dyn ExecutableModelProvider) -> AgentJobResult<()> {
        self.authority
            .ensure_ai_keeper_provider(&self.certificate, provider)
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
    pub result: Value,
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
        now_unix_ms: i64,
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
        _now_unix_ms: i64,
    ) -> AgentJobResult<AgentJobToolResult> {
        Err(AgentJobError::terminal("AGENT_TOOL_PERMISSION_DENIED"))
    }
}

#[derive(Clone)]
pub struct GovernedAgentJobToolPort {
    workflow: DurableWorkflowStore,
    context: Option<Arc<dyn AgentJobRepository>>,
    rules: Arc<dyn AgentSkillCheckRulePort>,
    gameplay: Arc<dyn AgentGameplayRulePort>,
}

impl fmt::Debug for GovernedAgentJobToolPort {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GovernedAgentJobToolPort")
            .field("workflow", &self.workflow)
            .field("context", &self.context.as_ref().map(|_| "[GOVERNED]"))
            .field("rules", &self.rules)
            .field("gameplay", &self.gameplay)
            .finish()
    }
}

impl GovernedAgentJobToolPort {
    pub fn new(workflow: DurableWorkflowStore, rules: Arc<dyn AgentSkillCheckRulePort>) -> Self {
        Self {
            workflow,
            context: None,
            rules,
            gameplay: Arc::new(RejectingAgentGameplayRulePort),
        }
    }

    pub fn with_gameplay(
        workflow: DurableWorkflowStore,
        context: Arc<dyn AgentJobRepository>,
        rules: Arc<dyn AgentSkillCheckRulePort>,
        gameplay: Arc<dyn AgentGameplayRulePort>,
    ) -> Self {
        Self {
            workflow,
            context: Some(context),
            rules,
            gameplay,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentSkillCheckRoll {
    pub execution_id: String,
    pub roll: u8,
    pub selected_tens_digit: u8,
    pub ones_digit: u8,
    pub success_level: String,
}

pub trait AgentSkillCheckRulePort: fmt::Debug + Send + Sync {
    fn roll_skill_check(&self, target: u8) -> AgentJobResult<AgentSkillCheckRoll>;
}

pub type AgentGameplayRuleFuture<'a> =
    Pin<Box<dyn Future<Output = AgentJobResult<Value>> + Send + 'a>>;

pub trait AgentGameplayRulePort: fmt::Debug + Send + Sync {
    fn resolve<'a>(
        &'a self,
        job: &'a DurableAgentJob,
        call: &'a AgentJobToolCall,
    ) -> AgentGameplayRuleFuture<'a>;
}

#[derive(Debug, Default)]
struct RejectingAgentGameplayRulePort;

impl AgentGameplayRulePort for RejectingAgentGameplayRulePort {
    fn resolve<'a>(
        &'a self,
        _job: &'a DurableAgentJob,
        _call: &'a AgentJobToolCall,
    ) -> AgentGameplayRuleFuture<'a> {
        Box::pin(async { Err(AgentJobError::terminal("AGENT_TOOL_PERMISSION_DENIED")) })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SkillCheckToolArguments {
    character_id: String,
    skill_name: String,
    adjustment: String,
}

include!("01_job_tool_contracts/01_governed_tool_execution.rs");
