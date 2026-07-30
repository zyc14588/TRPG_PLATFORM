#![cfg(unix)]

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

#[async_trait]
impl AgentJobRepository for FileAgentJobRepository {
    async fn load(&self, job_id: &str) -> Result<Option<DurableAgentJob>, AgentJobError> {
        let state = self.load_state();
        Ok((state.job_id == job_id).then(|| self.durable_job(&state)))
    }

    async fn claim_due(
        &self,
        claim_owner: &str,
        now_unix_ms: i64,
        lease_duration_ms: i64,
    ) -> Result<Option<DurableAgentJob>, AgentJobError> {
        crash_boundary("claim_before");
        let mut state = self.load_state();
        let current = parse_state(&state.state);
        let due = match current {
            WorkflowState::Requested | WorkflowState::RetryableFailed => true,
            WorkflowState::Claimed
            | WorkflowState::AgentRunning
            | WorkflowState::AwaitingTool
            | WorkflowState::Committing => state
                .lease_expires_at_unix_ms
                .is_some_and(|expires_at| expires_at <= now_unix_ms),
            _ => false,
        };
        if !due {
            return Ok(None);
        }
        let resume = match current {
            WorkflowState::Requested => WorkflowState::Requested,
            WorkflowState::Claimed => state
                .resume_state
                .as_deref()
                .map(parse_state)
                .unwrap_or(WorkflowState::Requested),
            WorkflowState::RetryableFailed => state
                .resume_state
                .as_deref()
                .map(parse_state)
                .unwrap_or(WorkflowState::AgentRunning),
            active => active,
        };
        state.attempt += 1;
        state.version += 1;
        state.state = WorkflowState::Claimed.as_str().to_owned();
        state.resume_state = Some(resume.as_str().to_owned());
        state.claim_owner = Some(claim_owner.to_owned());
        state.claim_token = Some(format!("claim-token-{}", state.attempt));
        state.heartbeat_at_unix_ms = Some(now_unix_ms);
        state.lease_expires_at_unix_ms = Some(
            now_unix_ms
                .checked_add(lease_duration_ms)
                .expect("test lease must not overflow"),
        );
        self.store_state(&state);
        crash_boundary("claim_after");
        Ok(Some(self.durable_job(&state)))
    }

    async fn transition(
        &self,
        draft: &AgentJobTransitionDraft,
    ) -> Result<DurableAgentJob, AgentJobError> {
        if draft.from_state == WorkflowState::AgentRunning
            && draft.to_state == WorkflowState::AwaitingTool
        {
            crash_boundary("provider_after");
        }
        if draft.from_state == WorkflowState::AwaitingTool
            && draft.to_state == WorkflowState::Committing
        {
            crash_boundary("tool_after");
        }
        let mut state = self.load_state();
        if state.job_id != draft.job_id
            || parse_state(&state.state) != draft.from_state
            || state.version != draft.expected_version
            || state.claim_owner.as_deref() != Some(draft.claim_owner.as_str())
            || state.claim_token.as_deref() != Some(draft.claim_token.as_str())
        {
            return Err(AgentJobError::retryable("AGENT_JOB_CAS_CONFLICT"));
        }
        state.version += 1;
        state.state = draft.to_state.as_str().to_owned();
        state.resume_state = if draft.to_state == WorkflowState::RetryableFailed {
            Some(draft.from_state.as_str().to_owned())
        } else {
            None
        };
        if let Some(decision) = &draft.decision_json {
            state.decision_json = Some(decision.clone());
        }
        if let Some(tool_result) = &draft.tool_result_json {
            state.tool_result_json = Some(tool_result.clone());
        }
        if let Some(linked) = &draft.linked_event_sequences {
            state.linked_event_sequences = linked.clone();
        }
        state.error_code = draft.error_code.clone();
        if matches!(
            draft.to_state,
            WorkflowState::Completed
                | WorkflowState::RetryableFailed
                | WorkflowState::TerminalFailed
        ) {
            state.claim_owner = None;
            state.claim_token = None;
            state.lease_expires_at_unix_ms = None;
            state.heartbeat_at_unix_ms = None;
        }
        if matches!(
            draft.to_state,
            WorkflowState::Completed | WorkflowState::TerminalFailed
        ) {
            state.decision_json = None;
            state.tool_result_json = None;
        }
        self.store_state(&state);
        Ok(self.durable_job(&state))
    }

    async fn heartbeat(
        &self,
        job_id: &str,
        claim_owner: &str,
        claim_token: &str,
        now_unix_ms: i64,
        lease_duration_ms: i64,
    ) -> Result<bool, AgentJobError> {
        let mut state = self.load_state();
        if state.job_id != job_id
            || state.claim_owner.as_deref() != Some(claim_owner)
            || state.claim_token.as_deref() != Some(claim_token)
        {
            return Ok(false);
        }
        state.heartbeat_at_unix_ms = Some(now_unix_ms);
        state.lease_expires_at_unix_ms = Some(
            now_unix_ms
                .checked_add(lease_duration_ms)
                .expect("test lease must not overflow"),
        );
        self.store_state(&state);
        Ok(true)
    }

    async fn cancellation_requested(&self, job_id: &str) -> Result<bool, AgentJobError> {
        Ok(self.load_state().job_id != job_id)
    }

    async fn load_authority(
        &self,
        campaign_id: &str,
    ) -> Result<DurableAgentAuthoritySnapshot, AgentJobError> {
        assert_eq!(campaign_id, "campaign_ar09_kill9");
        Ok(DurableAgentAuthoritySnapshot {
            contract_id: "authority_ar09_kill9".to_owned(),
            campaign_id: campaign_id.to_owned(),
            authority_mode: "AI_KP".to_owned(),
            authority_owner: "ai_keeper_ar09_kill9".to_owned(),
            contract_version: 1,
            prompt_version: "prompt_v1".to_owned(),
            agent_pack_version: "agent_pack_v1".to_owned(),
            tool_schema_version: "tool_v1".to_owned(),
            model_route_snapshot: "route_snapshot_ar09_kill9".to_owned(),
        })
    }

    async fn load_context(
        &self,
        job_id: &str,
    ) -> Result<DurableAgentContextSnapshot, AgentJobError> {
        assert_eq!(self.load_state().job_id, job_id);
        Ok(DurableAgentContextSnapshot {
            input_payload_json: r#"{"event":"npc_turn_requested"}"#.to_owned(),
            chunks: vec![DurableAgentContextChunk {
                chunk_id: "chunk_ar09_kill9".to_owned(),
                source_event_sequence: 9,
                visibility_label: "public".to_owned(),
                visibility_subject: None,
                fact_provenance_json: r#"{"kind":"tool_result"}"#.to_owned(),
                chunk_hash: "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
                    .to_owned(),
                content: "The corridor is quiet.".to_owned(),
            }],
        })
    }

    async fn load_approval(
        &self,
        _job_id: &str,
    ) -> Result<Option<DurableAgentApproval>, AgentJobError> {
        Ok(None)
    }

    async fn append_evidence(&self, draft: &AgentJobEvidenceDraft) -> Result<(), AgentJobError> {
        let mut state = self.load_state();
        let key = format!("{}:{}", draft.attempt, draft.phase);
        let encoded = serde_json::to_string(&json!({
            "model_id": draft.model_id,
            "runtime_version": draft.runtime_version,
            "prompt_template_hash": draft.prompt_template_hash,
            "tool_schema_hash": draft.tool_schema_hash,
            "retrieval_hash": draft.retrieval_hash,
            "input_hash": draft.input_hash,
            "output_hash": draft.output_hash,
            "input_tokens": draft.input_tokens,
            "output_tokens": draft.output_tokens,
            "latency_ms": draft.latency_ms,
            "tool_call_count": draft.tool_call_count,
            "linked_event_sequences": draft.linked_event_sequences,
            "visibility_label": draft.visibility_label,
            "retention_until_unix_ms": draft.retention_until_unix_ms,
        }))
        .expect("evidence must serialize");
        if let Some(existing) = state.evidence.get(&key) {
            if existing != &encoded {
                return Err(AgentJobError::terminal("EVIDENCE_IDEMPOTENCY_CONFLICT"));
            }
            return Ok(());
        }
        state.evidence.insert(key, encoded);
        self.store_state(&state);
        Ok(())
    }
}

struct Kill9Provider {
    provider_id: EntityId,
}

impl Kill9Provider {
    fn new() -> Self {
        Self {
            provider_id: EntityId::new("provider_ar09_kill9")
                .expect("test provider id must be valid"),
        }
    }

    fn route(&self, operation: ModelOperation) -> ExecutedModelRouteSnapshot {
        ExecutedModelRouteSnapshot {
            route_authorization_event_id: EntityId::new("route_authorized_ar09_kill9")
                .expect("test route id must be valid"),
            provider_id: self.provider_id.clone(),
            provider_type: ProviderType::Cloud,
            model_id: "model_ar09_kill9".to_owned(),
            operation,
            fallback_policy: "none_no_automatic_fallback",
            privacy_boundary: "explicit_route_authorization_event",
        }
    }
}

#[async_trait]
impl ExecutableModelProvider for Kill9Provider {
    fn provider_id(&self) -> &EntityId {
        &self.provider_id
    }

    fn provider_type(&self) -> ProviderType {
        ProviderType::Cloud
    }

    fn model_id(&self) -> &str {
        "model_ar09_kill9"
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
        crash_boundary("provider_before");
        Ok(ProviderExecution {
            route: self.route(ModelOperation::Chat),
            output: ModelChatResponse {
                content: String::new(),
                structured_output: Some(json!({
                    "kind": "npc_turn",
                    "player_visible_text": "Footsteps stop beyond the door.",
                    "tool": {"name":"change_scene","arguments":{}}
                })),
                tool_calls: Vec::new(),
                usage: ModelTokenUsage {
                    input_tokens: 50,
                    output_tokens: 25,
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
        panic!("streaming is outside this bounded kill9 test")
    }

    async fn embed(
        &self,
        _request: &ModelEmbeddingRequest,
        _cancellation: &ProviderCancellation,
    ) -> ModelProviderResult<ProviderExecution<ModelEmbeddingResponse>> {
        panic!("embedding is outside this bounded kill9 test")
    }
}

struct DurableToolPort {
    root: PathBuf,
}

#[async_trait]
impl AgentJobToolPort for DurableToolPort {
    async fn execute(
        &self,
        _job: &DurableAgentJob,
        _call: &AgentJobToolCall,
        idempotency_key: &str,
    ) -> Result<AgentJobToolResult, AgentJobError> {
        crash_boundary("tool_before");
        let path = self.root.join("tool-receipt");
        write_once(&path, idempotency_key.as_bytes());
        Ok(AgentJobToolResult {
            execution_id: "tool_execution_ar09_kill9".to_owned(),
            result_hash: "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                .to_owned(),
        })
    }
}

struct DurableDecisionPort {
    root: PathBuf,
}

#[async_trait]
impl AgentJobDecisionPort for DurableDecisionPort {
    async fn authorize_execution(
        &self,
        _job: &DurableAgentJob,
        _now_unix_ms: i64,
    ) -> Result<(), AgentJobError> {
        Ok(())
    }

    async fn commit_ai_decision(
        &self,
        job: &DurableAgentJob,
        _decision: &AgentStructuredDecision,
        _tool_result: Option<&AgentJobToolResult>,
        _now_unix_ms: i64,
    ) -> Result<AgentJobCommitReceipt, AgentJobError> {
        crash_boundary("event_commit_before");
        let path = self.root.join("canonical-event-receipt");
        write_once(&path, job.idempotency_key.as_bytes());
        crash_boundary("event_commit_after");
        Ok(AgentJobCommitReceipt {
            event_sequences: vec![9_001],
        })
    }
}

fn write_once(path: &Path, contents: &[u8]) {
    match OpenOptions::new().create_new(true).write(true).open(path) {
        Ok(mut file) => {
            file.write_all(contents)
                .expect("durable receipt must be written");
            file.sync_all().expect("durable receipt must be synced");
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            assert_eq!(
                fs::read(path).expect("existing durable receipt must be readable"),
                contents
            );
        }
        Err(error) => panic!("durable receipt must be created: {error}"),
    }
}

fn parse_state(value: &str) -> WorkflowState {
    match value {
        "REQUESTED" => WorkflowState::Requested,
        "CLAIMED" => WorkflowState::Claimed,
        "AGENT_RUNNING" => WorkflowState::AgentRunning,
        "AWAITING_TOOL" => WorkflowState::AwaitingTool,
        "COMMITTING" => WorkflowState::Committing,
        "COMPLETED" => WorkflowState::Completed,
        "RETRYABLE_FAILED" => WorkflowState::RetryableFailed,
        "TERMINAL_FAILED" => WorkflowState::TerminalFailed,
        other => panic!("unexpected kill9 workflow state: {other}"),
    }
}

fn crash_boundary(expected: &str) {
    if env::var("AR09_KILL9_BOUNDARY").as_deref() != Ok(expected) {
        return;
    }
    println!("{READY_PREFIX}{expected}");
    std::io::stdout()
        .flush()
        .expect("kill9 readiness marker must flush");
    loop {
        thread::park_timeout(Duration::from_secs(60));
    }
}

fn worker(root: &Path) -> AgentJobWorker {
    AgentJobWorker::new(
        Arc::new(FileAgentJobRepository::new(root)),
        Arc::new(Kill9Provider::new()),
        Arc::new(DurableToolPort {
            root: root.to_owned(),
        }),
        Arc::new(DurableDecisionPort {
            root: root.to_owned(),
        }),
        None,
        AgentJobExecutionConfig {
            claim_owner: "agent_worker_ar09_kill9".to_owned(),
            lease_duration: Duration::from_millis(100),
            heartbeat_interval: Duration::from_millis(10),
            max_attempts: 5,
            max_context_bytes: 64 * 1024,
            max_input_tokens: 1_000,
            max_output_tokens: 1_000,
            max_tool_calls: 1,
            max_tool_loops: 1,
        },
    )
    .expect("kill9 worker configuration must be valid")
}

#[tokio::test]
async fn agent_job_kill9_child() {
    let Ok(root) = env::var("AR09_KILL9_ROOT") else {
        return;
    };
    let now_unix_ms = env::var("AR09_KILL9_NOW")
        .expect("kill9 child now is required")
        .parse()
        .expect("kill9 child now must be numeric");
    let outcome = worker(Path::new(&root))
        .run_once(now_unix_ms)
        .await
        .expect("kill9 recovery worker must execute");
    assert!(matches!(outcome, AgentJobOutcome::Completed { .. }));
}

struct TemporaryRoot(PathBuf);

impl Drop for TemporaryRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn temporary_root() -> TemporaryRoot {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock must be after Unix epoch")
        .as_nanos();
    let root = env::temp_dir().join(format!(
        "trpg-ar09-kill9-{}-{timestamp}",
        std::process::id()
    ));
    fs::create_dir(&root).expect("unique kill9 root must be created");
    TemporaryRoot(root)
}

fn spawn_child(root: &Path, boundary: &str, now_unix_ms: i64) -> std::process::Child {
    Command::new(env::current_exe().expect("current test executable must be available"))
        .arg("--exact")
        .arg(CHILD_TEST_NAME)
        .arg("--nocapture")
        .arg("--test-threads=1")
        .env("AR09_KILL9_ROOT", root)
        .env("AR09_KILL9_BOUNDARY", boundary)
        .env("AR09_KILL9_NOW", now_unix_ms.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("kill9 child must start")
}

#[test]
fn kill9_at_claim_provider_tool_and_event_boundaries_is_exactly_once() {
    let temporary = temporary_root();
    for boundary in BOUNDARIES {
        let scenario_root = temporary.0.join(boundary);
        fs::create_dir(&scenario_root).expect("kill9 scenario root must be created");
        let repository = FileAgentJobRepository::new(&scenario_root);
        repository.initialize(&format!("job_ar09_kill9_{boundary}"));

        let mut child = spawn_child(&scenario_root, boundary, FIRST_RUN_NOW);
        let stdout = child
            .stdout
            .take()
            .expect("kill9 child stdout must be captured");
        let (sender, receiver) = mpsc::channel();
        let expected_marker = format!("{READY_PREFIX}{boundary}");
        let reader = thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let line = line.expect("kill9 child output must be readable");
                if line.contains(&expected_marker) {
                    sender
                        .send(expected_marker.clone())
                        .expect("kill9 readiness receiver must remain alive");
                    break;
                }
            }
        });
        let marker = receiver
            .recv_timeout(Duration::from_secs(15))
            .unwrap_or_else(|_| panic!("child did not reach kill9 boundary {boundary}"));
        assert_eq!(marker, format!("{READY_PREFIX}{boundary}"));
        child.kill().expect("kill9 child must accept SIGKILL");
        let status = child.wait().expect("kill9 child must be reaped");
        reader.join().expect("kill9 output reader must finish");
        assert_eq!(
            status.signal(),
            Some(9),
            "{boundary} must terminate through SIGKILL"
        );

        let recovery = Command::new(
            env::current_exe().expect("current test executable must be available for recovery"),
        )
        .arg("--exact")
        .arg(CHILD_TEST_NAME)
        .arg("--nocapture")
        .arg("--test-threads=1")
        .env("AR09_KILL9_ROOT", &scenario_root)
        .env("AR09_KILL9_BOUNDARY", "none")
        .env("AR09_KILL9_NOW", RECOVERY_NOW.to_string())
        .output()
        .expect("kill9 recovery child must start");
        assert!(
            recovery.status.success(),
            "recovery failed at {boundary}: stdout={} stderr={}",
            String::from_utf8_lossy(&recovery.stdout),
            String::from_utf8_lossy(&recovery.stderr),
        );

        let state = repository.load_state();
        assert_eq!(state.state, WorkflowState::Completed.as_str(), "{boundary}");
        assert_eq!(state.linked_event_sequences, vec![9_001], "{boundary}");
        assert_eq!(state.decision_json, None, "{boundary}");
        assert_eq!(state.tool_result_json, None, "{boundary}");
        assert!(
            state
                .evidence
                .keys()
                .any(|key| key.ends_with(":canonical_commit")),
            "{boundary} must retain canonical evidence"
        );
        assert!(
            state.evidence.keys().any(|key| key.ends_with(":completed")),
            "{boundary} must retain completion evidence"
        );
        assert!(
            scenario_root.join("tool-receipt").is_file(),
            "{boundary} must have one idempotent tool receipt"
        );
        assert!(
            scenario_root.join("canonical-event-receipt").is_file(),
            "{boundary} must have one idempotent canonical receipt"
        );
        let canonical_receipts = fs::read_dir(&scenario_root)
            .expect("kill9 scenario root must remain readable")
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name() == "canonical-event-receipt")
            .count();
        assert_eq!(
            canonical_receipts, 1,
            "{boundary} must produce at most one canonical event receipt"
        );
        println!(
            "AR09_KILL9_VERIFIED boundary={boundary} signal=9 canonical_event_receipts={canonical_receipts}"
        );
    }
}
