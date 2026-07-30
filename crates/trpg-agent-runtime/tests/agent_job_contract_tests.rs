use std::collections::HashMap;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use serde_json::{json, Value};
use trpg_agent_runtime::agent_job::{
    AgentJobCommitReceipt, AgentJobDecisionPort, AgentJobError, AgentJobExecutionConfig,
    AgentJobOutcome, AgentJobRepository, AgentJobToolCall, AgentJobToolPort, AgentJobToolResult,
    AgentJobWorker, AgentStructuredDecision, CertifiedLocalModel,
};
use trpg_agent_runtime::local_model_certification::{
    CertificationInput, LocalModelCertificationAuthority,
};
use trpg_agent_runtime::model_provider::{
    ExecutableModelProvider, ExecutedModelRouteSnapshot, ModelChatRequest, ModelChatResponse,
    ModelEmbeddingRequest, ModelEmbeddingResponse, ModelOperation, ModelProviderResult,
    ModelStreamSink, ModelTokenUsage, ModelToolCall, ProviderCancellation, ProviderCapabilities,
    ProviderExecution, ProviderType,
};
use trpg_agent_runtime::EntityId;
use trpg_runtime::durable_workflow::{
    AgentJobEvidenceDraft, AgentJobTransitionDraft, DurableAgentApproval,
    DurableAgentAuthoritySnapshot, DurableAgentContextChunk, DurableAgentContextSnapshot,
    DurableAgentJob, WorkflowState,
};
use trpg_security_governance::secret::{LedgerCheckpoint, LedgerCheckpointStore};
use trpg_shared_kernel::KernelResult;

const NOW: i64 = 1_000_000;
const ARTIFACT: &str = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

#[derive(Default)]
struct MemoryCheckpoint {
    value: Mutex<Option<(String, LedgerCheckpoint)>>,
}

impl LedgerCheckpointStore for MemoryCheckpoint {
    fn latest(&self, ledger_id: &str) -> KernelResult<Option<LedgerCheckpoint>> {
        Ok(self
            .value
            .lock()
            .unwrap()
            .as_ref()
            .filter(|(stored, _)| stored == ledger_id)
            .map(|(_, checkpoint)| checkpoint.clone()))
    }

    fn append(&self, ledger_id: &str, checkpoint: &LedgerCheckpoint) -> KernelResult<()> {
        *self.value.lock().unwrap() = Some((ledger_id.to_owned(), checkpoint.clone()));
        Ok(())
    }
}

struct LocalCertificationFixture {
    certification: CertifiedLocalModel,
    root: PathBuf,
}

impl Drop for LocalCertificationFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn level4_certification(model_id: &str) -> LocalCertificationFixture {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "trpg-ar09-cert-{}-{}-{}",
        std::process::id(),
        timestamp,
        NEXT.fetch_add(1, Ordering::Relaxed),
    ));
    fs::create_dir_all(&root).unwrap();
    #[cfg(unix)]
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let authority = Arc::new(
        LocalModelCertificationAuthority::new_with_checkpoint(
            "ar09-test-key",
            &[0x71; 32],
            root.join("registry.jsonl"),
            Arc::new(MemoryCheckpoint::default()),
        )
        .unwrap(),
    );
    let certificate = authority
        .issue_level4(
            &CertificationInput {
                model_id: model_id.to_owned(),
                json_schema_support: true,
                tool_call_support: true,
                visibility_tests_pass: true,
                prompt_injection_tests_pass: true,
                rules_eval_pass: true,
                latency_ms: 100,
            },
            ARTIFACT,
            "ar09-suite",
            Duration::from_secs(60),
        )
        .unwrap();
    LocalCertificationFixture {
        certification: CertifiedLocalModel::new(authority, certificate),
        root,
    }
}

struct MockProvider {
    provider_type: ProviderType,
    provider_id: EntityId,
    model_id: String,
    structured_output: Value,
    tool_calls: Vec<ModelToolCall>,
    usage: ModelTokenUsage,
    calls: AtomicU64,
}

impl MockProvider {
    fn new(provider_type: ProviderType, structured_output: Value) -> Self {
        Self {
            provider_type,
            provider_id: EntityId::new("provider_ar09").unwrap(),
            model_id: "model_ar09".to_owned(),
            structured_output,
            tool_calls: Vec::new(),
            usage: ModelTokenUsage {
                input_tokens: 50,
                output_tokens: 25,
            },
            calls: AtomicU64::new(0),
        }
    }

    fn route(&self, operation: ModelOperation) -> ExecutedModelRouteSnapshot {
        ExecutedModelRouteSnapshot {
            route_authorization_event_id: EntityId::new("route_authorized_ar09").unwrap(),
            provider_id: self.provider_id.clone(),
            provider_type: self.provider_type,
            model_id: self.model_id.clone(),
            operation,
            fallback_policy: "none_no_automatic_fallback",
            privacy_boundary: "explicit_route_authorization_event",
        }
    }
}

#[async_trait]
impl ExecutableModelProvider for MockProvider {
    fn provider_id(&self) -> &EntityId {
        &self.provider_id
    }

    fn provider_type(&self) -> ProviderType {
        self.provider_type
    }

    fn model_id(&self) -> &str {
        &self.model_id
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
                structured_output: Some(self.structured_output.clone()),
                tool_calls: self.tool_calls.clone(),
                usage: self.usage,
            },
        })
    }

    async fn stream_chat(
        &self,
        _request: &ModelChatRequest,
        _sink: &dyn ModelStreamSink,
        _cancellation: &ProviderCancellation,
    ) -> ModelProviderResult<ExecutedModelRouteSnapshot> {
        panic!("streaming is outside this bounded agent job")
    }

    async fn embed(
        &self,
        _request: &ModelEmbeddingRequest,
        _cancellation: &ProviderCancellation,
    ) -> ModelProviderResult<ProviderExecution<ModelEmbeddingResponse>> {
        panic!("embedding is outside this bounded agent job")
    }
}

struct RepositoryState {
    job: DurableAgentJob,
    authority: DurableAgentAuthoritySnapshot,
    context: DurableAgentContextSnapshot,
    approval: Option<DurableAgentApproval>,
    cancellation_requested: bool,
    transitions: HashMap<String, DurableAgentJob>,
    evidence: HashMap<(i32, String), AgentJobEvidenceDraft>,
}

struct MemoryRepository {
    state: Mutex<RepositoryState>,
}

impl MemoryRepository {
    fn new(job: DurableAgentJob) -> Self {
        let authority = DurableAgentAuthoritySnapshot {
            contract_id: job.authority_contract_id.clone(),
            campaign_id: job.campaign_id.clone(),
            authority_mode: job.authority_mode.clone(),
            authority_owner: if job.authority_mode == "AI_KP" {
                job.actor_id.clone()
            } else {
                "human_keeper_ar09".to_owned()
            },
            contract_version: job.authority_contract_version,
            prompt_version: job.prompt_template_version.clone(),
            agent_pack_version: "agent_pack_v1".to_owned(),
            tool_schema_version: job.tool_schema_version.clone(),
            model_route_snapshot: "route_snapshot_ar09".to_owned(),
        };
        Self {
            state: Mutex::new(RepositoryState {
                job,
                authority,
                context: visible_context(),
                approval: None,
                cancellation_requested: false,
                transitions: HashMap::new(),
                evidence: HashMap::new(),
            }),
        }
    }

    fn job(&self) -> DurableAgentJob {
        self.state.lock().unwrap().job.clone()
    }

    fn set_context(&self, context: DurableAgentContextSnapshot) {
        self.state.lock().unwrap().context = context;
    }

    fn approve(&self, sequence: i64) {
        self.state.lock().unwrap().approval = Some(DurableAgentApproval {
            approval_id: "approval_ar09".to_owned(),
            approval_event_sequence: sequence,
            approved_by: "human_keeper_ar09".to_owned(),
            idempotency_key: "approval_idempotency_ar09".to_owned(),
        });
    }

    fn cancel(&self) {
        self.state.lock().unwrap().cancellation_requested = true;
    }
}

#[async_trait]
impl AgentJobRepository for MemoryRepository {
    async fn load(&self, job_id: &str) -> Result<Option<DurableAgentJob>, AgentJobError> {
        let state = self.state.lock().unwrap();
        Ok((state.job.job_id == job_id).then(|| state.job.clone()))
    }

    async fn claim_due(
        &self,
        claim_owner: &str,
        now_unix_ms: i64,
        lease_duration_ms: i64,
    ) -> Result<Option<DurableAgentJob>, AgentJobError> {
        let mut state = self.state.lock().unwrap();
        if matches!(
            state.job.state,
            WorkflowState::Completed | WorkflowState::TerminalFailed
        ) || (state.job.state == WorkflowState::RetryableFailed
            && state
                .job
                .next_attempt_at_unix_ms
                .is_some_and(|next| next > now_unix_ms))
            || (state.job.state == WorkflowState::AwaitingTool
                && state.job.authority_mode == "HUMAN_KP"
                && state.approval.is_none()
                && !state.cancellation_requested
                && state.job.deadline_unix_ms > now_unix_ms)
        {
            return Ok(None);
        }
        let resume = if state.job.state == WorkflowState::RetryableFailed {
            state
                .job
                .resume_state
                .unwrap_or(WorkflowState::RetryableFailed)
        } else {
            state.job.state
        };
        state.job.state = WorkflowState::Claimed;
        state.job.resume_state = Some(resume);
        state.job.version += 1;
        state.job.attempt += 1;
        state.job.claim_owner = Some(claim_owner.to_owned());
        state.job.claim_token = Some(format!("claim_{}", state.job.attempt));
        state.job.lease_expires_at_unix_ms = Some(now_unix_ms.saturating_add(lease_duration_ms));
        Ok(Some(state.job.clone()))
    }

    async fn transition(
        &self,
        draft: &AgentJobTransitionDraft,
    ) -> Result<DurableAgentJob, AgentJobError> {
        let mut state = self.state.lock().unwrap();
        if let Some(existing) = state.transitions.get(&draft.idempotency_key) {
            return Ok(existing.clone());
        }
        if state.job.job_id != draft.job_id
            || state.job.version != draft.expected_version
            || state.job.state != draft.from_state
            || state.job.claim_owner.as_deref() != Some(draft.claim_owner.as_str())
            || state.job.claim_token.as_deref() != Some(draft.claim_token.as_str())
        {
            return Err(AgentJobError::retryable("MEMORY_REPOSITORY_CAS_CONFLICT"));
        }
        state.job.state = draft.to_state;
        state.job.resume_state = if draft.to_state == WorkflowState::RetryableFailed {
            Some(draft.from_state)
        } else {
            None
        };
        state.job.version += 1;
        if let Some(decision) = &draft.decision_json {
            state.job.decision_json = Some(decision.clone());
        }
        if let Some(tool_result) = &draft.tool_result_json {
            state.job.tool_result_json = Some(tool_result.clone());
        }
        if matches!(
            draft.to_state,
            WorkflowState::Completed | WorkflowState::TerminalFailed
        ) {
            state.job.decision_json = None;
            state.job.tool_result_json = None;
        }
        if let Some(sequences) = &draft.linked_event_sequences {
            state.job.linked_event_sequences = sequences.clone();
        }
        state.job.error_code = draft.error_code.clone();
        state.job.next_attempt_at_unix_ms = draft.next_attempt_at_unix_ms;
        if matches!(
            draft.to_state,
            WorkflowState::Completed
                | WorkflowState::RetryableFailed
                | WorkflowState::TerminalFailed
        ) {
            state.job.claim_owner = None;
            state.job.claim_token = None;
            state.job.lease_expires_at_unix_ms = None;
        }
        let updated = state.job.clone();
        state
            .transitions
            .insert(draft.idempotency_key.clone(), updated.clone());
        Ok(updated)
    }

    async fn heartbeat(
        &self,
        job_id: &str,
        claim_owner: &str,
        claim_token: &str,
        now_unix_ms: i64,
        lease_duration_ms: i64,
    ) -> Result<bool, AgentJobError> {
        let mut state = self.state.lock().unwrap();
        let matches = state.job.job_id == job_id
            && state.job.claim_owner.as_deref() == Some(claim_owner)
            && state.job.claim_token.as_deref() == Some(claim_token);
        if matches {
            state.job.heartbeat_at_unix_ms = Some(now_unix_ms);
            state.job.lease_expires_at_unix_ms =
                Some(now_unix_ms.saturating_add(lease_duration_ms));
        }
        Ok(matches)
    }

    async fn cancellation_requested(&self, _job_id: &str) -> Result<bool, AgentJobError> {
        Ok(self.state.lock().unwrap().cancellation_requested)
    }

    async fn load_authority(
        &self,
        _campaign_id: &str,
    ) -> Result<DurableAgentAuthoritySnapshot, AgentJobError> {
        Ok(self.state.lock().unwrap().authority.clone())
    }

    async fn load_context(
        &self,
        _job_id: &str,
    ) -> Result<DurableAgentContextSnapshot, AgentJobError> {
        Ok(self.state.lock().unwrap().context.clone())
    }

    async fn load_approval(
        &self,
        _job_id: &str,
    ) -> Result<Option<DurableAgentApproval>, AgentJobError> {
        Ok(self.state.lock().unwrap().approval.clone())
    }

    async fn append_evidence(&self, draft: &AgentJobEvidenceDraft) -> Result<(), AgentJobError> {
        let mut state = self.state.lock().unwrap();
        let key = (draft.attempt, draft.phase.clone());
        if let Some(existing) = state.evidence.get(&key) {
            if existing != draft {
                return Err(AgentJobError::terminal("EVIDENCE_IDEMPOTENCY_CONFLICT"));
            }
            return Ok(());
        }
        state.evidence.insert(key, draft.clone());
        Ok(())
    }
}

#[derive(Default)]
struct CountingToolPort {
    receipts: Mutex<HashMap<String, AgentJobToolResult>>,
    executions: AtomicU64,
}

#[async_trait]
impl AgentJobToolPort for CountingToolPort {
    async fn execute(
        &self,
        _job: &DurableAgentJob,
        _call: &AgentJobToolCall,
        idempotency_key: &str,
    ) -> Result<AgentJobToolResult, AgentJobError> {
        let mut receipts = self.receipts.lock().unwrap();
        if let Some(receipt) = receipts.get(idempotency_key) {
            return Ok(receipt.clone());
        }
        self.executions.fetch_add(1, Ordering::SeqCst);
        let receipt = AgentJobToolResult {
            execution_id: "tool_execution_ar09".to_owned(),
            result_hash: "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                .to_owned(),
        };
        receipts.insert(idempotency_key.to_owned(), receipt.clone());
        Ok(receipt)
    }
}

#[derive(Default)]
struct CountingDecisionPort {
    receipts: Mutex<HashMap<String, AgentJobCommitReceipt>>,
    next_sequence: AtomicI64,
    canonical_events: AtomicU64,
    authorization_checks: AtomicU64,
    deny_authorization: bool,
    fail_after_first_commit: AtomicBool,
}

impl CountingDecisionPort {
    fn crash_after_first_commit() -> Self {
        Self {
            fail_after_first_commit: AtomicBool::new(true),
            ..Self::default()
        }
    }

    fn deny_authorization() -> Self {
        Self {
            deny_authorization: true,
            ..Self::default()
        }
    }
}

#[async_trait]
impl AgentJobDecisionPort for CountingDecisionPort {
    async fn authorize_execution(
        &self,
        _job: &DurableAgentJob,
        _now_unix_ms: i64,
    ) -> Result<(), AgentJobError> {
        self.authorization_checks.fetch_add(1, Ordering::SeqCst);
        if self.deny_authorization {
            Err(AgentJobError::terminal(
                "AGENT_EXECUTION_AUTHORIZATION_DENIED",
            ))
        } else {
            Ok(())
        }
    }

    async fn commit_ai_decision(
        &self,
        job: &DurableAgentJob,
        _decision: &AgentStructuredDecision,
        _tool_result: Option<&AgentJobToolResult>,
        _now_unix_ms: i64,
    ) -> Result<AgentJobCommitReceipt, AgentJobError> {
        let mut receipts = self.receipts.lock().unwrap();
        if let Some(receipt) = receipts.get(&job.idempotency_key) {
            return Ok(receipt.clone());
        }
        let sequence = self.next_sequence.fetch_add(1, Ordering::SeqCst) + 1;
        self.canonical_events.fetch_add(1, Ordering::SeqCst);
        let receipt = AgentJobCommitReceipt {
            event_sequences: vec![sequence],
        };
        receipts.insert(job.idempotency_key.clone(), receipt.clone());
        if self.fail_after_first_commit.swap(false, Ordering::SeqCst) {
            return Err(AgentJobError::retryable(
                "INJECTED_CRASH_AFTER_CANONICAL_COMMIT",
            ));
        }
        Ok(receipt)
    }
}

fn provider_type_name(provider_type: ProviderType) -> &'static str {
    match provider_type {
        ProviderType::Cloud => "cloud",
        ProviderType::Ollama => "ollama",
        ProviderType::LlamaCpp => "llama_cpp",
        ProviderType::LocalOpenAiCompatible => "local_openai_compatible",
    }
}

fn job(provider_type: ProviderType, authority_mode: &str) -> DurableAgentJob {
    DurableAgentJob {
        job_id: format!("job_{}", provider_type_name(provider_type)),
        campaign_id: "campaign_ar09".to_owned(),
        actor_id: if authority_mode == "AI_KP" {
            "ai_keeper_ar09".to_owned()
        } else {
            "copilot_ar09".to_owned()
        },
        agent_kind: if authority_mode == "AI_KP" {
            "ai_keeper_orchestrator".to_owned()
        } else {
            "keeper_copilot".to_owned()
        },
        authority_contract_id: "authority_ar09".to_owned(),
        authority_mode: authority_mode.to_owned(),
        authority_contract_version: 1,
        input_event_sequence: 10,
        input_stream_id: "session_ar09".to_owned(),
        input_stream_version: 10,
        visibility_scope_json:
            r#"{"allowed_labels":["public"],"subject_id":null,"output_label":"public"}"#.to_owned(),
        rag_snapshot_id: "rag_ar09".to_owned(),
        provider_id: "provider_ar09".to_owned(),
        provider_type: provider_type_name(provider_type).to_owned(),
        model_id: "model_ar09".to_owned(),
        model_artifact_sha256: ARTIFACT.to_owned(),
        route_authorization_event_id: "route_authorized_ar09".to_owned(),
        prompt_template_id: "npc_turn".to_owned(),
        prompt_template_version: "prompt_v1".to_owned(),
        tool_schema_version: "tool_v1".to_owned(),
        idempotency_key: format!("idempotency_{}", provider_type_name(provider_type)),
        deadline_unix_ms: NOW + 1_000_000,
        state: WorkflowState::Requested,
        resume_state: None,
        version: 0,
        claim_owner: None,
        claim_token: None,
        lease_expires_at_unix_ms: None,
        heartbeat_at_unix_ms: None,
        attempt: 0,
        next_attempt_at_unix_ms: None,
        decision_json: None,
        tool_result_json: None,
        linked_event_sequences: Vec::new(),
        cancellation_requested_at_unix_ms: None,
        error_code: None,
    }
}

fn visible_context() -> DurableAgentContextSnapshot {
    DurableAgentContextSnapshot {
        input_payload_json: r#"{"event":"npc_turn_requested"}"#.to_owned(),
        chunks: vec![DurableAgentContextChunk {
            chunk_id: "chunk_ar09".to_owned(),
            source_event_sequence: 9,
            visibility_label: "public".to_owned(),
            visibility_subject: None,
            fact_provenance_json: r#"{"kind":"tool_result"}"#.to_owned(),
            chunk_hash: "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
                .to_owned(),
            content: "The corridor is quiet.".to_owned(),
        }],
    }
}

fn valid_decision() -> Value {
    json!({
        "kind": "npc_turn",
        "player_visible_text": "Footsteps stop beyond the door.",
        "tool": null
    })
}

fn configuration() -> AgentJobExecutionConfig {
    AgentJobExecutionConfig {
        claim_owner: "agent_worker_ar09".to_owned(),
        lease_duration: Duration::from_secs(30),
        heartbeat_interval: Duration::from_millis(10),
        max_attempts: 5,
        max_context_bytes: 64 * 1024,
        max_input_tokens: 1_000,
        max_output_tokens: 1_000,
        max_tool_calls: 1,
        max_tool_loops: 1,
    }
}

fn worker(
    repository: Arc<MemoryRepository>,
    provider: Arc<MockProvider>,
    tools: Arc<CountingToolPort>,
    decisions: Arc<CountingDecisionPort>,
    certification: Option<CertifiedLocalModel>,
    configuration: AgentJobExecutionConfig,
) -> AgentJobWorker {
    AgentJobWorker::new(
        repository,
        provider,
        tools,
        decisions,
        certification,
        configuration,
    )
    .unwrap()
}

#[tokio::test]
async fn real_agent_job_worker_completes_an_npc_turn_through_all_three_providers() {
    for provider_type in [
        ProviderType::Cloud,
        ProviderType::Ollama,
        ProviderType::LlamaCpp,
    ] {
        let repository = Arc::new(MemoryRepository::new(job(provider_type, "AI_KP")));
        let provider = Arc::new(MockProvider::new(provider_type, valid_decision()));
        let decisions = Arc::new(CountingDecisionPort::default());
        let certification_fixture =
            (provider_type != ProviderType::Cloud).then(|| level4_certification("model_ar09"));
        let certification = certification_fixture
            .as_ref()
            .map(|fixture| fixture.certification.clone());
        let outcome = worker(
            Arc::clone(&repository),
            Arc::clone(&provider),
            Arc::new(CountingToolPort::default()),
            Arc::clone(&decisions),
            certification,
            configuration(),
        )
        .run_once(NOW)
        .await
        .unwrap();
        assert!(matches!(outcome, AgentJobOutcome::Completed { .. }));
        assert_eq!(repository.job().state, WorkflowState::Completed);
        assert_eq!(repository.job().decision_json, None);
        assert_eq!(repository.job().tool_result_json, None);
        assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
        assert_eq!(decisions.authorization_checks.load(Ordering::SeqCst), 1);
        assert_eq!(decisions.canonical_events.load(Ordering::SeqCst), 1);
    }
}

#[tokio::test]
async fn crash_after_tool_and_canonical_commit_recovers_without_duplicates() {
    let mut output = valid_decision();
    output["tool"] = json!({"name":"change_scene","arguments":{}});
    let repository = Arc::new(MemoryRepository::new(job(ProviderType::Cloud, "AI_KP")));
    let provider = Arc::new(MockProvider::new(ProviderType::Cloud, output));
    let tools = Arc::new(CountingToolPort::default());
    let decisions = Arc::new(CountingDecisionPort::crash_after_first_commit());
    let worker = worker(
        Arc::clone(&repository),
        Arc::clone(&provider),
        Arc::clone(&tools),
        Arc::clone(&decisions),
        None,
        configuration(),
    );

    let first = worker.run_once(NOW).await.unwrap();
    assert!(matches!(first, AgentJobOutcome::RetryScheduled { .. }));
    let second = worker.run_once(NOW + 100_000).await.unwrap();
    assert!(matches!(second, AgentJobOutcome::Completed { .. }));
    assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
    assert_eq!(decisions.authorization_checks.load(Ordering::SeqCst), 2);
    assert_eq!(tools.executions.load(Ordering::SeqCst), 1);
    assert_eq!(decisions.canonical_events.load(Ordering::SeqCst), 1);
    assert_eq!(repository.job().linked_event_sequences.len(), 1);
}

#[tokio::test]
async fn human_kp_job_is_draft_only_until_a_separate_approval_event_exists() {
    let repository = Arc::new(MemoryRepository::new(job(ProviderType::Cloud, "HUMAN_KP")));
    let provider = Arc::new(MockProvider::new(ProviderType::Cloud, valid_decision()));
    let decisions = Arc::new(CountingDecisionPort::default());
    let worker = worker(
        Arc::clone(&repository),
        Arc::clone(&provider),
        Arc::new(CountingToolPort::default()),
        Arc::clone(&decisions),
        None,
        configuration(),
    );

    let draft = worker.run_once(NOW).await.unwrap();
    assert!(matches!(
        draft,
        AgentJobOutcome::AwaitingHumanApproval { .. }
    ));
    assert_eq!(decisions.canonical_events.load(Ordering::SeqCst), 0);
    assert_eq!(provider.calls.load(Ordering::SeqCst), 1);

    repository.approve(77);
    let approved = worker.run_once(NOW + 1).await.unwrap();
    assert_eq!(
        approved,
        AgentJobOutcome::Completed {
            job_id: "job_cloud".to_owned(),
            event_sequences: vec![77],
        }
    );
    assert_eq!(decisions.canonical_events.load(Ordering::SeqCst), 0);
    assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
    assert_eq!(repository.job().decision_json, None);
}

#[tokio::test]
async fn local_ai_keeper_without_level4_certification_fails_closed() {
    let repository = Arc::new(MemoryRepository::new(job(ProviderType::Ollama, "AI_KP")));
    let provider = Arc::new(MockProvider::new(ProviderType::Ollama, valid_decision()));
    let outcome = worker(
        Arc::clone(&repository),
        provider,
        Arc::new(CountingToolPort::default()),
        Arc::new(CountingDecisionPort::default()),
        None,
        configuration(),
    )
    .run_once(NOW)
    .await
    .unwrap();
    assert!(matches!(
        outcome,
        AgentJobOutcome::TerminalFailure {
            error_code: "LOCAL_MODEL_LEVEL_4_REQUIRED",
            ..
        }
    ));
    assert_eq!(repository.job().state, WorkflowState::TerminalFailed);
}

#[tokio::test]
async fn invisible_rag_and_unauthorized_tools_and_invalid_output_fail_closed() {
    let unauthorized_repository =
        Arc::new(MemoryRepository::new(job(ProviderType::Cloud, "AI_KP")));
    let unauthorized_provider = Arc::new(MockProvider::new(ProviderType::Cloud, valid_decision()));
    let unauthorized_outcome = worker(
        Arc::clone(&unauthorized_repository),
        Arc::clone(&unauthorized_provider),
        Arc::new(CountingToolPort::default()),
        Arc::new(CountingDecisionPort::deny_authorization()),
        None,
        configuration(),
    )
    .run_once(NOW)
    .await
    .unwrap();
    assert_eq!(
        unauthorized_outcome,
        AgentJobOutcome::TerminalFailure {
            job_id: "job_cloud".to_owned(),
            error_code: "AGENT_EXECUTION_AUTHORIZATION_DENIED",
        }
    );
    assert_eq!(unauthorized_provider.calls.load(Ordering::SeqCst), 0);

    let invisible_repository = Arc::new(MemoryRepository::new(job(ProviderType::Cloud, "AI_KP")));
    let mut invisible = visible_context();
    invisible.chunks[0].visibility_label = "keeper_only".to_owned();
    invisible_repository.set_context(invisible);
    let invisible_outcome = worker(
        Arc::clone(&invisible_repository),
        Arc::new(MockProvider::new(ProviderType::Cloud, valid_decision())),
        Arc::new(CountingToolPort::default()),
        Arc::new(CountingDecisionPort::default()),
        None,
        configuration(),
    )
    .run_once(NOW)
    .await
    .unwrap();
    assert!(matches!(
        invisible_outcome,
        AgentJobOutcome::TerminalFailure {
            error_code: "RAG_VISIBILITY_SCOPE_VIOLATION",
            ..
        }
    ));

    let unauthorized_repository =
        Arc::new(MemoryRepository::new(job(ProviderType::Cloud, "AI_KP")));
    let unauthorized = json!({
        "kind":"npc_turn",
        "player_visible_text":"No.",
        "tool":{"name":"delete_database","arguments":{}}
    });
    let unauthorized_outcome = worker(
        unauthorized_repository,
        Arc::new(MockProvider::new(ProviderType::Cloud, unauthorized)),
        Arc::new(CountingToolPort::default()),
        Arc::new(CountingDecisionPort::default()),
        None,
        configuration(),
    )
    .run_once(NOW)
    .await
    .unwrap();
    assert!(matches!(
        unauthorized_outcome,
        AgentJobOutcome::TerminalFailure {
            error_code: "AGENT_TOOL_PERMISSION_DENIED",
            ..
        }
    ));

    let invalid_repository = Arc::new(MemoryRepository::new(job(ProviderType::Cloud, "AI_KP")));
    let invalid_outcome = worker(
        invalid_repository,
        Arc::new(MockProvider::new(
            ProviderType::Cloud,
            json!({"kind":"npc_turn"}),
        )),
        Arc::new(CountingToolPort::default()),
        Arc::new(CountingDecisionPort::default()),
        None,
        configuration(),
    )
    .run_once(NOW)
    .await
    .unwrap();
    assert!(matches!(
        invalid_outcome,
        AgentJobOutcome::TerminalFailure {
            error_code: "AGENT_OUTPUT_SCHEMA_INVALID",
            ..
        }
    ));
}

#[tokio::test]
async fn token_budget_deadline_and_cancellation_are_enforced() {
    let mut loop_configuration = configuration();
    loop_configuration.max_tool_loops = 2;
    assert_eq!(
        loop_configuration.validate().unwrap_err().code(),
        "AGENT_JOB_CONFIGURATION_INVALID"
    );

    let mut exhausted_job = job(ProviderType::Cloud, "AI_KP");
    exhausted_job.state = WorkflowState::AgentRunning;
    exhausted_job.attempt = configuration().max_attempts;
    let exhausted_repository = Arc::new(MemoryRepository::new(exhausted_job));
    let exhausted_provider = Arc::new(MockProvider::new(ProviderType::Cloud, valid_decision()));
    let exhausted_outcome = worker(
        exhausted_repository,
        Arc::clone(&exhausted_provider),
        Arc::new(CountingToolPort::default()),
        Arc::new(CountingDecisionPort::default()),
        None,
        configuration(),
    )
    .run_once(NOW)
    .await
    .unwrap();
    assert_eq!(
        exhausted_outcome,
        AgentJobOutcome::TerminalFailure {
            job_id: "job_cloud".to_owned(),
            error_code: "AGENT_JOB_ATTEMPT_LIMIT_EXCEEDED",
        }
    );
    assert_eq!(exhausted_provider.calls.load(Ordering::SeqCst), 0);

    let budget_repository = Arc::new(MemoryRepository::new(job(ProviderType::Cloud, "AI_KP")));
    let mut budget_provider = MockProvider::new(ProviderType::Cloud, valid_decision());
    budget_provider.usage.output_tokens = 2_000;
    let budget_outcome = worker(
        budget_repository,
        Arc::new(budget_provider),
        Arc::new(CountingToolPort::default()),
        Arc::new(CountingDecisionPort::default()),
        None,
        configuration(),
    )
    .run_once(NOW)
    .await
    .unwrap();
    assert!(matches!(
        budget_outcome,
        AgentJobOutcome::TerminalFailure {
            error_code: "AGENT_JOB_BUDGET_EXCEEDED",
            ..
        }
    ));

    let mut expired_job = job(ProviderType::Cloud, "AI_KP");
    expired_job.deadline_unix_ms = NOW;
    let deadline_outcome = worker(
        Arc::new(MemoryRepository::new(expired_job)),
        Arc::new(MockProvider::new(ProviderType::Cloud, valid_decision())),
        Arc::new(CountingToolPort::default()),
        Arc::new(CountingDecisionPort::default()),
        None,
        configuration(),
    )
    .run_once(NOW)
    .await
    .unwrap();
    assert!(matches!(
        deadline_outcome,
        AgentJobOutcome::TerminalFailure {
            error_code: "AGENT_JOB_DEADLINE_EXCEEDED",
            ..
        }
    ));

    let cancellation_repository =
        Arc::new(MemoryRepository::new(job(ProviderType::Cloud, "AI_KP")));
    cancellation_repository.cancel();
    let cancellation_outcome = worker(
        cancellation_repository,
        Arc::new(MockProvider::new(ProviderType::Cloud, valid_decision())),
        Arc::new(CountingToolPort::default()),
        Arc::new(CountingDecisionPort::default()),
        None,
        configuration(),
    )
    .run_once(NOW)
    .await
    .unwrap();
    assert!(matches!(
        cancellation_outcome,
        AgentJobOutcome::TerminalFailure {
            error_code: "AGENT_JOB_CANCELLED",
            ..
        }
    ));
}
