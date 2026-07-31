use std::collections::HashMap;
use std::env;
use std::io::{Read, Write};
use std::net::SocketAddr;
use std::net::TcpStream;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use api_server::{AgentJobRouteConfiguration, ApiApplication};
use serde_json::{json, Value};
use sqlx::postgres::PgPoolOptions;
use trpg_agent_runtime::agent_job::{
    AgentJobCommitReceipt, AgentJobDecisionPort, AgentJobError, AgentJobExecutionConfig,
    AgentJobOutcome, AgentJobRepository, AgentJobResult, AgentJobToolCall, AgentJobToolPort,
    AgentJobToolResult, AgentJobWorker, AgentSkillCheckRoll, AgentSkillCheckRulePort,
    AgentStructuredDecision, GovernedAgentDecisionPort, GovernedAgentJobToolPort,
    ProductionAgentIdentityConfiguration,
};
use trpg_agent_runtime::model_provider::{
    ExecutableModelProvider, ExecutedModelRouteSnapshot, ModelChatRequest, ModelChatResponse,
    ModelEmbeddingRequest, ModelEmbeddingResponse, ModelOperation, ModelProviderResult,
    ModelStreamSink, ModelTokenUsage, ProviderCancellation, ProviderCapabilities,
    ProviderExecution, ProviderType,
};
use trpg_contracts::{HttpRequest, HttpResponse};
use trpg_data_eventing::event_store_sqlx_outbox_projection::{
    PostgresCanonicalCommitPort, PostgresCanonicalStore,
};
use trpg_identity::{CampaignRole, GlobalRole, IdentityService};
use trpg_ruleset_coc7::dice_roll_contract::{
    server_roll_skill_check, DiceAdjustment, SuccessLevel,
};
use trpg_runtime::durable_workflow::{
    AgentJobEvidenceDraft, AgentJobTransitionDraft, DurableAgentApproval,
    DurableAgentAuthoritySnapshot, DurableAgentContextSnapshot, DurableAgentJob,
    DurableWorkflowStore,
};
use trpg_security_governance::policy_adapter::{
    HttpPolicyEndpoint, OpenFgaOpaPolicyAdapter, PolicyBackend,
};
use trpg_security_governance::security_privacy::PostgresDeletionRepository;
use trpg_security_governance::tamper_evident_audit::FileAuditLog;
use trpg_shared_kernel::{
    AuthorityContract, AuthorityContractDraft, AuthorityMode, AuthorityVersionSnapshotDraft,
    CanonicalCommitKey, CanonicalCommitPort, EntityId,
};

const IDENTITY_KEY: [u8; 32] = [0x21; 32];
const INTEGRITY_KEY: [u8; 32] = [0x32; 32];
const PAYLOAD_KEY: [u8; 32] = [0x43; 32];
const AUDIT_KEY: [u8; 32] = [0x54; 32];
const ARTIFACT: &str = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const PROVIDER_ID: &str = "provider_ar09_public";
const MODEL_ID: &str = "model_ar09_public";
const ROUTE_ID: &str = "route_ar09_public";
static PUBLIC_AGENT_JOB_TEST_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug)]
struct Coc7AgentSkillCheckRules;

impl AgentSkillCheckRulePort for Coc7AgentSkillCheckRules {
    fn roll_skill_check(&self, target: u8) -> AgentJobResult<AgentSkillCheckRoll> {
        let roll = server_roll_skill_check(target, DiceAdjustment::None)
            .map_err(|_| AgentJobError::terminal("AGENT_SKILL_CHECK_RULE_FAILURE"))?;
        let outcome = roll.outcome();
        let success_level = match outcome.success_level {
            SuccessLevel::Critical => "CRITICAL",
            SuccessLevel::Extreme => "EXTREME",
            SuccessLevel::Hard => "HARD",
            SuccessLevel::Regular => "REGULAR",
            SuccessLevel::Failure => "FAILURE",
            SuccessLevel::Fumble => "FUMBLE",
        };
        Ok(AgentSkillCheckRoll {
            execution_id: roll.roll_id().to_owned(),
            roll: outcome.roll,
            selected_tens_digit: outcome.selected_tens_digit,
            ones_digit: outcome.ones_digit,
            success_level: success_level.to_owned(),
        })
    }
}

fn required(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("{name} is required for the AR09 public product gate"))
}

fn now_unix_ms() -> u64 {
    u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_millis(),
    )
    .expect("current time fits u64")
}

fn policy() -> OpenFgaOpaPolicyAdapter {
    let openfga_address = required("P02_OPENFGA_ADDRESS")
        .parse::<SocketAddr>()
        .expect("valid OpenFGA address");
    let openfga_store = required("P02_OPENFGA_STORE_ID");
    let openfga_model = required("P02_OPENFGA_MODEL_ID");
    let opa_address = required("P02_OPA_ADDRESS")
        .parse::<SocketAddr>()
        .expect("valid OPA address");
    let opa_revision = required("P02_OPA_REVISION");
    OpenFgaOpaPolicyAdapter::new(
        HttpPolicyEndpoint::new(
            openfga_address,
            format!("/stores/{openfga_store}/check"),
            PolicyBackend::OpenFga,
            openfga_model,
        )
        .expect("valid OpenFGA endpoint"),
        HttpPolicyEndpoint::new(
            opa_address,
            "/v1/data/security_governance/decision",
            PolicyBackend::Opa,
            opa_revision,
        )
        .expect("valid OPA endpoint"),
    )
    .expect("valid policy pair")
}

fn seed_workflow_policy(campaign_id: &str) {
    let address = required("P02_OPENFGA_ADDRESS")
        .parse::<SocketAddr>()
        .expect("valid OpenFGA address");
    let store_id = required("P02_OPENFGA_STORE_ID");
    let model_id = required("P02_OPENFGA_MODEL_ID");
    let body = serde_json::to_vec(&json!({
        "authorization_model_id": model_id,
        "writes": {
            "tuple_keys": [
                {
                    "user": "principal:api_core_workflow",
                    "relation": "workflow",
                    "object": format!("campaign:{campaign_id}")
                },
                {
                    "user": "principal:agent_worker_ar09_public",
                    "relation": "workflow",
                    "object": format!("campaign:{campaign_id}")
                }
            ]
        }
    }))
    .expect("serialize OpenFGA fixture");
    let path = format!("/stores/{store_id}/write");
    let mut stream =
        TcpStream::connect_timeout(&address, Duration::from_secs(2)).expect("connect OpenFGA");
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .and_then(|()| stream.set_write_timeout(Some(Duration::from_secs(2))))
        .expect("configure OpenFGA fixture timeout");
    let request = format!(
        "POST {path} HTTP/1.1\r\nHost: {address}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream
        .write_all(request.as_bytes())
        .and_then(|()| stream.write_all(&body))
        .expect("write OpenFGA fixture");
    let mut response = Vec::new();
    stream
        .take(1_048_576)
        .read_to_end(&mut response)
        .expect("read OpenFGA fixture response");
    let status = response
        .split(|byte| *byte == b'\n')
        .next()
        .and_then(|line| std::str::from_utf8(line).ok())
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|status| status.parse::<u16>().ok())
        .expect("OpenFGA fixture status");
    assert!(
        (200..300).contains(&status),
        "OpenFGA fixture rejected with status {status}"
    );
}

fn request(path: &str, token: &str, body: Value) -> HttpRequest {
    let mut headers = HashMap::new();
    headers.insert("authorization".to_owned(), format!("Bearer {token}"));
    headers.insert("content-type".to_owned(), "application/json".to_owned());
    HttpRequest {
        method: "POST".to_owned(),
        path: path.to_owned(),
        headers,
        body: serde_json::to_vec(&body).expect("serialize request"),
    }
}

fn call(application: &ApiApplication, request: &HttpRequest) -> HttpResponse {
    application
        .handle(request)
        .unwrap_or_else(|| panic!("published Agent Job route missing: {}", request.path))
}

fn authority(
    campaign_id: &str,
    authority_owner: &str,
    created_at_unix_ms: u64,
) -> AuthorityContract {
    AuthorityContract::new_locked(AuthorityContractDraft {
        contract_id: format!("authority_{campaign_id}"),
        campaign_id: campaign_id.to_owned(),
        mode: AuthorityMode::AiKp,
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
    .expect("valid AI_KP authority")
}

struct SkillCheckProvider {
    provider_id: EntityId,
    character_id: String,
    calls: AtomicU64,
}

impl SkillCheckProvider {
    fn new(character_id: String) -> Self {
        Self {
            provider_id: EntityId::new(PROVIDER_ID).expect("valid provider id"),
            character_id,
            calls: AtomicU64::new(0),
        }
    }

    fn route(&self, operation: ModelOperation) -> ExecutedModelRouteSnapshot {
        ExecutedModelRouteSnapshot {
            route_authorization_event_id: EntityId::new(ROUTE_ID).expect("valid route id"),
            provider_id: self.provider_id.clone(),
            provider_type: ProviderType::Cloud,
            model_id: MODEL_ID.to_owned(),
            operation,
            fallback_policy: "none_no_automatic_fallback",
            privacy_boundary: "explicit_route_authorization_event",
        }
    }
}

#[trpg_agent_runtime::repository_async_trait]
impl ExecutableModelProvider for SkillCheckProvider {
    fn provider_id(&self) -> &EntityId {
        &self.provider_id
    }

    fn provider_type(&self) -> ProviderType {
        ProviderType::Cloud
    }

    fn model_id(&self) -> &str {
        MODEL_ID
    }

    fn model_artifact_sha256(&self) -> &str {
        ARTIFACT
    }

    fn startup_route_snapshot(&self) -> ExecutedModelRouteSnapshot {
        self.route(ModelOperation::CapabilityProbe)
    }

    async fn probe_capabilities(
        &self,
        _cancellation: &ProviderCancellation,
    ) -> ModelProviderResult<ProviderExecution<ProviderCapabilities>> {
        Ok(ProviderExecution {
            route: self.route(ModelOperation::CapabilityProbe),
            output: ProviderCapabilities::v1_complete(),
        })
    }

    async fn chat(
        &self,
        _request: &ModelChatRequest,
        _cancellation: &ProviderCancellation,
    ) -> ModelProviderResult<ProviderExecution<ModelChatResponse>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(ProviderExecution {
            route: self.route(ModelOperation::Chat),
            output: ModelChatResponse {
                content: String::new(),
                structured_output: Some(json!({
                    "kind": "npc_turn",
                    "player_visible_text": "The investigator checks the archive index.",
                    "tool": {
                        "name": "request_skill_check",
                        "arguments": {
                            "adjustment": "NONE",
                            "character_id": self.character_id,
                            "skill_name": "Library Use"
                        }
                    }
                })),
                tool_calls: Vec::new(),
                usage: ModelTokenUsage {
                    input_tokens: 64,
                    output_tokens: 32,
                },
            },
        })
    }

    async fn stream_chat(
        &self,
        _request: &ModelChatRequest,
        _sink: &dyn ModelStreamSink,
        _cancellation: &ProviderCancellation,
    ) -> ModelProviderResult<ExecutedModelRouteSnapshot> {
        panic!("streaming is outside the bounded AR09 public test")
    }

    async fn embed(
        &self,
        _request: &ModelEmbeddingRequest,
        _cancellation: &ProviderCancellation,
    ) -> ModelProviderResult<ProviderExecution<ModelEmbeddingResponse>> {
        panic!("embedding is outside the bounded AR09 public test")
    }
}

struct LoseFirstCanonicalReceipt {
    inner: GovernedAgentDecisionPort,
    lost: AtomicBool,
}

#[trpg_agent_runtime::repository_async_trait]
impl AgentJobDecisionPort for LoseFirstCanonicalReceipt {
    async fn authorize_execution(
        &self,
        job: &DurableAgentJob,
        now_unix_ms: i64,
    ) -> Result<(), AgentJobError> {
        self.inner.authorize_execution(job, now_unix_ms).await
    }

    async fn commit_ai_decision(
        &self,
        job: &DurableAgentJob,
        decision: &AgentStructuredDecision,
        tool_result: Option<&trpg_agent_runtime::AgentJobToolResult>,
        now_unix_ms: i64,
    ) -> Result<AgentJobCommitReceipt, AgentJobError> {
        let receipt = self
            .inner
            .commit_ai_decision(job, decision, tool_result, now_unix_ms)
            .await?;
        if !self.lost.swap(true, Ordering::SeqCst) {
            return Err(AgentJobError::retryable(
                "AR09_INJECTED_RECEIPT_LOSS_AFTER_CANONICAL_COMMIT",
            ));
        }
        Ok(receipt)
    }
}

#[path = "agent_job_public_product_integration_sections/fixture.rs"]
mod fixture;
#[path = "agent_job_public_product_integration_sections/human_approval_test.rs"]
mod human_approval_test;
#[cfg(unix)]
#[path = "agent_job_public_product_integration_sections/production_kill9_test.rs"]
mod production_kill9_test;
#[path = "agent_job_public_product_integration_sections/test.rs"]
mod test;
