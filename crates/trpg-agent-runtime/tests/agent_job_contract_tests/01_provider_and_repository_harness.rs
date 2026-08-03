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
use trpg_agent_runtime::local_model_certification::LocalModelCertificationAuthority;
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

fn level4_certification(
    provider_type: ProviderType,
    model_id: &str,
) -> LocalCertificationFixture {
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
    let run = certification_support::passing_run_for_provider_blocking(
        "provider_ar09",
        provider_type,
        model_id,
        ARTIFACT,
        certification_support::RUNTIME_SHA256,
    );
    let certificate = authority
        .issue_level4_from_run(&run, Duration::from_secs(60))
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

    fn provider_runtime_sha256(&self) -> String {
        certification_support::RUNTIME_SHA256.to_owned()
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
