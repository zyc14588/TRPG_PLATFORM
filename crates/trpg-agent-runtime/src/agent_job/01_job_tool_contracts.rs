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

#[async_trait]
impl AgentJobToolPort for GovernedAgentJobToolPort {
    async fn execute(
        &self,
        job: &DurableAgentJob,
        call: &AgentJobToolCall,
        idempotency_key: &str,
        now_unix_ms: i64,
    ) -> AgentJobResult<AgentJobToolResult> {
        if job.authority_mode != "AI_KP"
            || job.agent_kind != "ai_keeper_orchestrator"
            || idempotency_key != format!("{}:tool", job.idempotency_key)
        {
            return Err(AgentJobError::terminal("AGENT_TOOL_PERMISSION_DENIED"));
        }
        if matches!(
            call.name.as_str(),
            "resolve_npc_interaction" | "resolve_combat_round" | "resolve_chase_segment"
        ) {
            let context = self
                .context
                .as_ref()
                .ok_or_else(|| AgentJobError::terminal("AGENT_TOOL_PERMISSION_DENIED"))?
                .load_context(&job.job_id)
                .await?;
            validate_gameplay_tool_binding(&context.input_payload_json, call)?;
            let result = self.gameplay.resolve(job, call).await?;
            if !result.is_object() {
                return Err(AgentJobError::terminal("AGENT_TOOL_RESULT_INVALID"));
            }
            let encoded = serde_json::to_vec(&result)
                .map_err(|_| AgentJobError::terminal("AGENT_TOOL_RESULT_INVALID"))?;
            let digest = format!(
                "{:x}",
                Sha256::digest([job.job_id.as_bytes(), &encoded].concat())
            );
            return Ok(AgentJobToolResult {
                execution_id: format!("gameplay_{}", &digest[..32]),
                result,
                result_hash: format!("sha256:{:x}", Sha256::digest(&encoded)),
            });
        }
        if call.name != "request_skill_check" {
            return Err(AgentJobError::terminal("AGENT_TOOL_PERMISSION_DENIED"));
        }
        let arguments: SkillCheckToolArguments = serde_json::from_value(call.arguments.clone())
            .map_err(|_| AgentJobError::terminal("AGENT_TOOL_ARGUMENTS_INVALID"))?;
        if arguments.adjustment != "NONE" {
            return Err(AgentJobError::terminal("AGENT_TOOL_ARGUMENTS_INVALID"));
        }
        let rules = Arc::clone(&self.rules);
        let receipt = self
            .workflow
            .execute_agent_job_skill_check(
                &AgentJobSkillCheckDraft {
                    job_id: job.job_id.clone(),
                    claim_owner: job
                        .claim_owner
                        .clone()
                        .ok_or_else(|| AgentJobError::retryable("AGENT_JOB_LEASE_LOST"))?,
                    claim_token: job
                        .claim_token
                        .clone()
                        .ok_or_else(|| AgentJobError::retryable("AGENT_JOB_LEASE_LOST"))?,
                    expected_attempt: job.attempt,
                    idempotency_key: idempotency_key.to_owned(),
                    character_id: arguments.character_id,
                    skill_name: arguments.skill_name,
                    adjustment: arguments.adjustment,
                    now_unix_ms,
                },
                move |target| {
                    let roll = rules.roll_skill_check(target).map_err(|_| {
                        WorkflowStoreError::IntegrityViolation("agent_skill_check_rule_failure")
                    })?;
                    Ok(AgentJobSkillCheckRollDraft {
                        execution_id: roll.execution_id,
                        roll: roll.roll,
                        selected_tens_digit: roll.selected_tens_digit,
                        ones_digit: roll.ones_digit,
                        success_level: roll.success_level,
                    })
                },
            )
            .await
            .map_err(|error| match error {
                WorkflowStoreError::NotFound => {
                    AgentJobError::terminal("AGENT_SKILL_TARGET_NOT_FOUND")
                }
                other => map_store_error(other),
            })?;
        let result = serde_json::from_str(&receipt.result_json)
            .map_err(|_| AgentJobError::terminal("AGENT_TOOL_RESULT_INVALID"))?;
        Ok(AgentJobToolResult {
            execution_id: receipt.execution_id,
            result,
            result_hash: receipt.result_hash,
        })
    }
}

fn validate_gameplay_tool_binding(
    input_payload_json: &str,
    call: &AgentJobToolCall,
) -> AgentJobResult<()> {
    let payload: Value = serde_json::from_str(input_payload_json)
        .map_err(|_| AgentJobError::terminal("AGENT_JOB_INPUT_EVENT_INVALID"))?;
    let input = payload
        .get("input")
        .and_then(Value::as_object)
        .filter(|input| input.get("kind").and_then(Value::as_str) == Some("player_action"))
        .ok_or_else(|| AgentJobError::terminal("AGENT_TOOL_ARGUMENTS_INVALID"))?;
    let intent = input
        .get("intent")
        .and_then(Value::as_object)
        .ok_or_else(|| AgentJobError::terminal("AGENT_TOOL_ARGUMENTS_INVALID"))?;
    let arguments = call
        .arguments
        .as_object()
        .ok_or_else(|| AgentJobError::terminal("AGENT_TOOL_ARGUMENTS_INVALID"))?;
    let intent_kind = intent
        .get("kind")
        .and_then(Value::as_str)
        .ok_or_else(|| AgentJobError::terminal("AGENT_TOOL_ARGUMENTS_INVALID"))?;
    let (expected_tool, required_keys): (&str, &[&str]) = match intent_kind {
        "NPC_INTERACTION" => (
            "resolve_npc_interaction",
            &[
                "session_id",
                "character_id",
                "npc_id",
                "approach",
                "public_response",
            ],
        ),
        "COMBAT_ROUND" => (
            "resolve_combat_round",
            &[
                "session_id",
                "character_id",
                "npc_id",
                "action_kind",
                "defense",
            ],
        ),
        "CHASE_SEGMENT" => (
            "resolve_chase_segment",
            &[
                "session_id",
                "character_id",
                "npc_id",
                "initial_range",
                "obstacle_id",
                "obstacle_cost",
            ],
        ),
        _ => return Err(AgentJobError::terminal("AGENT_TOOL_ARGUMENTS_INVALID")),
    };
    if call.name != expected_tool
        || arguments.len() != required_keys.len()
        || required_keys
            .iter()
            .any(|key| !arguments.contains_key(*key))
        || input.get("session_id") != arguments.get("session_id")
        || input.get("character_id") != arguments.get("character_id")
        || intent.get("npc_id") != arguments.get("npc_id")
    {
        return Err(AgentJobError::terminal("AGENT_TOOL_ARGUMENTS_INVALID"));
    }
    let bounded_choices_match = match intent_kind {
        "NPC_INTERACTION" => {
            intent.get("approach") == arguments.get("approach")
                && arguments
                    .get("public_response")
                    .and_then(Value::as_str)
                    .is_some_and(|value| !value.trim().is_empty() && value.len() <= 2_000)
        }
        "COMBAT_ROUND" => {
            intent.get("action_kind") == arguments.get("action_kind")
                && intent.get("defense") == arguments.get("defense")
        }
        "CHASE_SEGMENT" => {
            intent.get("initial_range") == arguments.get("initial_range")
                && intent.get("obstacle_id") == arguments.get("obstacle_id")
                && intent.get("obstacle_cost") == arguments.get("obstacle_cost")
        }
        _ => false,
    };
    if !bounded_choices_match {
        return Err(AgentJobError::terminal("AGENT_TOOL_ARGUMENTS_INVALID"));
    }
    Ok(())
}

#[cfg(test)]
mod public_gameplay_tool_binding_tests {
    use super::*;

    fn payload(intent: Value) -> String {
        json!({
            "input": {
                "kind": "player_action",
                "session_id": "session_rf04",
                "character_id": "character_rf04",
                "intent": intent
            }
        })
        .to_string()
    }

    #[test]
    fn each_public_gameplay_tool_is_bound_to_the_canonical_input() {
        let cases = [
            (
                payload(json!({
                    "kind": "NPC_INTERACTION",
                    "npc_id": "npc_rf04",
                    "approach": "Ask about the archive"
                })),
                AgentJobToolCall {
                    name: "resolve_npc_interaction".to_owned(),
                    arguments: json!({
                        "session_id": "session_rf04",
                        "character_id": "character_rf04",
                        "npc_id": "npc_rf04",
                        "approach": "Ask about the archive",
                        "public_response": "Marta points toward the locked stacks."
                    }),
                },
            ),
            (
                payload(json!({
                    "kind": "COMBAT_ROUND",
                    "npc_id": "npc_rf04",
                    "action_kind": "MELEE",
                    "defense": "DODGE"
                })),
                AgentJobToolCall {
                    name: "resolve_combat_round".to_owned(),
                    arguments: json!({
                        "session_id": "session_rf04",
                        "character_id": "character_rf04",
                        "npc_id": "npc_rf04",
                        "action_kind": "MELEE",
                        "defense": "DODGE"
                    }),
                },
            ),
            (
                payload(json!({
                    "kind": "CHASE_SEGMENT",
                    "npc_id": "npc_rf04",
                    "initial_range": 2,
                    "obstacle_id": "stairs",
                    "obstacle_cost": 1
                })),
                AgentJobToolCall {
                    name: "resolve_chase_segment".to_owned(),
                    arguments: json!({
                        "session_id": "session_rf04",
                        "character_id": "character_rf04",
                        "npc_id": "npc_rf04",
                        "initial_range": 2,
                        "obstacle_id": "stairs",
                        "obstacle_cost": 1
                    }),
                },
            ),
        ];
        for (input, call) in cases {
            validate_gameplay_tool_binding(&input, &call).unwrap();
        }
    }

    #[test]
    fn model_cannot_change_canonical_gameplay_ids_or_choices() {
        let input = payload(json!({
            "kind": "COMBAT_ROUND",
            "npc_id": "npc_rf04",
            "action_kind": "MELEE",
            "defense": "DODGE"
        }));
        for arguments in [
            json!({
                "session_id": "session_other",
                "character_id": "character_rf04",
                "npc_id": "npc_rf04",
                "action_kind": "MELEE",
                "defense": "DODGE"
            }),
            json!({
                "session_id": "session_rf04",
                "character_id": "character_rf04",
                "npc_id": "npc_rf04",
                "action_kind": "FIREARM",
                "defense": "DODGE"
            }),
        ] {
            let error = validate_gameplay_tool_binding(
                &input,
                &AgentJobToolCall {
                    name: "resolve_combat_round".to_owned(),
                    arguments,
                },
            )
            .unwrap_err();
            assert_eq!(error.code(), "AGENT_TOOL_ARGUMENTS_INVALID");
        }
    }
}
