use std::collections::BTreeMap;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::json;
use trpg_agent_runtime::agent_job::{
    AgentJobCommitReceipt, AgentJobDecisionPort, AgentJobError, AgentJobExecutionConfig,
    AgentJobOutcome, AgentJobRepository, AgentJobToolCall, AgentJobToolPort, AgentJobToolResult,
    AgentJobWorker, AgentStructuredDecision,
};
use trpg_agent_runtime::model_provider::{
    ExecutableModelProvider, ExecutedModelRouteSnapshot, ModelChatRequest, ModelChatResponse,
    ModelEmbeddingRequest, ModelEmbeddingResponse, ModelOperation, ModelProviderResult,
    ModelStreamSink, ModelTokenUsage, ProviderCancellation, ProviderCapabilities,
    ProviderExecution, ProviderType,
};
use trpg_agent_runtime::EntityId;
use trpg_runtime::durable_workflow::{
    AgentJobEvidenceDraft, AgentJobTransitionDraft, DurableAgentApproval,
    DurableAgentAuthoritySnapshot, DurableAgentContextChunk, DurableAgentContextSnapshot,
    DurableAgentJob, WorkflowState,
};

const FIRST_RUN_NOW: i64 = 1_000_000;
const RECOVERY_NOW: i64 = 2_000_000;
const ARTIFACT: &str = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const READY_PREFIX: &str = "AR09_KILL9_READY=";
const CHILD_TEST_NAME: &str = "agent_job_kill9_child";
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

#[derive(Clone, Debug, Deserialize, Serialize)]
struct DurableTestState {
    job_id: String,
    state: String,
    resume_state: Option<String>,
    version: i64,
    claim_owner: Option<String>,
    claim_token: Option<String>,
    lease_expires_at_unix_ms: Option<i64>,
    heartbeat_at_unix_ms: Option<i64>,
    attempt: i32,
    decision_json: Option<String>,
    tool_result_json: Option<String>,
    linked_event_sequences: Vec<i64>,
    error_code: Option<String>,
    evidence: BTreeMap<String, String>,
}

impl DurableTestState {
    fn requested(job_id: String) -> Self {
        Self {
            job_id,
            state: WorkflowState::Requested.as_str().to_owned(),
            resume_state: None,
            version: 0,
            claim_owner: None,
            claim_token: None,
            lease_expires_at_unix_ms: None,
            heartbeat_at_unix_ms: None,
            attempt: 0,
            decision_json: None,
            tool_result_json: None,
            linked_event_sequences: Vec::new(),
            error_code: None,
            evidence: BTreeMap::new(),
        }
    }
}

#[derive(Clone)]
struct FileAgentJobRepository {
    state_path: PathBuf,
}

impl FileAgentJobRepository {
    fn new(root: &Path) -> Self {
        Self {
            state_path: root.join("job-state.json"),
        }
    }

    fn initialize(&self, job_id: &str) {
        self.store_state(&DurableTestState::requested(job_id.to_owned()));
    }

    fn load_state(&self) -> DurableTestState {
        serde_json::from_slice(
            &fs::read(&self.state_path).expect("durable test state must remain readable"),
        )
        .expect("durable test state must remain valid JSON")
    }

    fn store_state(&self, state: &DurableTestState) {
        let temporary = self
            .state_path
            .with_extension(format!("tmp-{}", std::process::id()));
        let encoded = serde_json::to_vec(state).expect("durable test state must serialize");
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&temporary)
            .expect("temporary durable state must open");
        file.write_all(&encoded)
            .expect("temporary durable state must be written");
        file.sync_all()
            .expect("temporary durable state must be synced");
        fs::rename(&temporary, &self.state_path).expect("durable state replacement must be atomic");
    }

    fn durable_job(&self, state: &DurableTestState) -> DurableAgentJob {
        DurableAgentJob {
            job_id: state.job_id.clone(),
            campaign_id: "campaign_ar09_kill9".to_owned(),
            actor_id: "ai_keeper_ar09_kill9".to_owned(),
            agent_kind: "ai_keeper_orchestrator".to_owned(),
            authority_contract_id: "authority_ar09_kill9".to_owned(),
            authority_mode: "AI_KP".to_owned(),
            authority_contract_version: 1,
            input_event_sequence: 10,
            input_stream_id: "session_ar09_kill9".to_owned(),
            input_stream_version: 10,
            visibility_scope_json:
                r#"{"allowed_labels":["public"],"subject_id":null,"output_label":"public"}"#
                    .to_owned(),
            rag_snapshot_id: "rag_ar09_kill9".to_owned(),
            provider_id: "provider_ar09_kill9".to_owned(),
            provider_type: "cloud".to_owned(),
            model_id: "model_ar09_kill9".to_owned(),
            model_artifact_sha256: ARTIFACT.to_owned(),
            route_authorization_event_id: "route_authorized_ar09_kill9".to_owned(),
            prompt_template_id: "npc_turn".to_owned(),
            prompt_template_version: "prompt_v1".to_owned(),
            tool_schema_version: "tool_v1".to_owned(),
            idempotency_key: format!("{}:canonical", state.job_id),
            deadline_unix_ms: RECOVERY_NOW + 1_000_000,
            state: parse_state(&state.state),
            resume_state: state.resume_state.as_deref().map(parse_state),
            version: state.version,
            claim_owner: state.claim_owner.clone(),
            claim_token: state.claim_token.clone(),
            lease_expires_at_unix_ms: state.lease_expires_at_unix_ms,
            heartbeat_at_unix_ms: state.heartbeat_at_unix_ms,
            attempt: state.attempt,
            next_attempt_at_unix_ms: None,
            decision_json: state.decision_json.clone(),
            tool_result_json: state.tool_result_json.clone(),
            linked_event_sequences: state.linked_event_sequences.clone(),
            cancellation_requested_at_unix_ms: None,
            error_code: state.error_code.clone(),
        }
    }
}
