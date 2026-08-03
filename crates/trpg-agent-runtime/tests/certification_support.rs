#![allow(dead_code)]

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use trpg_agent_runtime::agent_runtime::AgentResult;
use trpg_agent_runtime::local_model_certification::{
    CertificationRequest, CompletedCertificationRun, LocalModelCertificationRunner,
    LocalModelCertificationSuite,
};
use trpg_agent_runtime::model_provider::{
    ExecutableModelProvider, ExecutedModelRouteSnapshot, ModelChatRequest, ModelChatResponse,
    ModelEmbeddingRequest, ModelEmbeddingResponse, ModelOperation, ModelProviderResult,
    ModelStreamSink, ModelTokenUsage, ModelToolCall, ProviderCancellation, ProviderCapabilities,
    ProviderExecution, ProviderType,
};
use trpg_agent_runtime::EntityId;

pub const RUNTIME_SHA256: &str =
    "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CertificationFault {
    None,
    PromptInjection,
    VisibilityLeakage,
    ToolInstability,
    Timeout,
}

pub struct DeterministicCertificationProvider {
    provider_id: EntityId,
    provider_type: ProviderType,
    model_id: String,
    artifact_sha256: String,
    runtime_sha256: String,
    fault: CertificationFault,
    chat_calls: AtomicU64,
    tool_calls: AtomicU64,
}

impl DeterministicCertificationProvider {
    pub fn new(model_id: &str, artifact_sha256: &str, fault: CertificationFault) -> Self {
        Self::new_with_identity(
            "rf01_fake_provider",
            ProviderType::Ollama,
            model_id,
            artifact_sha256,
            RUNTIME_SHA256,
            fault,
        )
    }

    pub fn new_with_identity(
        provider_id: &str,
        provider_type: ProviderType,
        model_id: &str,
        artifact_sha256: &str,
        runtime_sha256: &str,
        fault: CertificationFault,
    ) -> Self {
        Self {
            provider_id: EntityId::new(provider_id).unwrap(),
            provider_type,
            model_id: model_id.to_owned(),
            artifact_sha256: artifact_sha256.to_owned(),
            runtime_sha256: runtime_sha256.to_owned(),
            fault,
            chat_calls: AtomicU64::new(0),
            tool_calls: AtomicU64::new(0),
        }
    }

    pub fn chat_calls(&self) -> u64 {
        self.chat_calls.load(Ordering::SeqCst)
    }

    fn route(&self, operation: ModelOperation) -> ExecutedModelRouteSnapshot {
        ExecutedModelRouteSnapshot {
            route_authorization_event_id: EntityId::new("rf01_certification_route").unwrap(),
            provider_id: self.provider_id.clone(),
            provider_type: self.provider_type,
            model_id: self.model_id.clone(),
            operation,
            fallback_policy: "none_no_automatic_fallback",
            privacy_boundary: "local_certification_only",
        }
    }

    fn response_for(&self, request: &ModelChatRequest) -> ModelChatResponse {
        let prompt = request
            .messages
            .last()
            .map(|message| message.content.as_str())
            .unwrap_or_default();
        let (content, structured_output, tool_calls) = if prompt.contains("tool_use_stability") {
            let call = self.tool_calls.fetch_add(1, Ordering::SeqCst);
            let target = if self.fault == CertificationFault::ToolInstability && call == 1 {
                61
            } else {
                60
            };
            (
                String::new(),
                None,
                vec![ModelToolCall {
                    id: format!("rf01-tool-{call}"),
                    name: "request_skill_check".to_owned(),
                    arguments: serde_json::json!({"skill": "spot_hidden", "target": target}),
                }],
            )
        } else if prompt.contains("visibility_leakage") {
            (
                if self.fault == CertificationFault::VisibilityLeakage {
                    "KEEPER_ONLY_CANARY_RF01".to_owned()
                } else {
                    String::new()
                },
                Some(serde_json::json!({
                    "visible_fact": "brass key",
                    "withheld_private": true
                })),
                Vec::new(),
            )
        } else if prompt.contains("prompt_injection") {
            let vulnerable = self.fault == CertificationFault::PromptInjection;
            (
                if vulnerable {
                    "KEEPER_ONLY_CANARY_RF01"
                } else {
                    ""
                }
                .to_owned(),
                Some(serde_json::json!({"refused": !vulnerable})),
                Vec::new(),
            )
        } else if prompt.contains("coc_rules_mini_eval") {
            (
                String::new(),
                Some(serde_json::json!({
                    "outcome": "regular_success",
                    "roll": 42,
                    "target": 60
                })),
                Vec::new(),
            )
        } else if prompt.contains("certification_case:latency") {
            (
                String::new(),
                Some(serde_json::json!({"ready": true})),
                Vec::new(),
            )
        } else if prompt.contains("context_stress") {
            (
                String::new(),
                Some(serde_json::json!({
                    "decision": "hold",
                    "tail": "CONTEXT_TAIL_CANARY_RF01"
                })),
                Vec::new(),
            )
        } else {
            (
                String::new(),
                Some(serde_json::json!({
                    "decision": "request_skill_check",
                    "visibility": "public"
                })),
                Vec::new(),
            )
        };
        ModelChatResponse {
            content,
            structured_output,
            tool_calls,
            usage: ModelTokenUsage {
                input_tokens: 32,
                output_tokens: 12,
            },
        }
    }
}

#[async_trait]
impl ExecutableModelProvider for DeterministicCertificationProvider {
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
        &self.artifact_sha256
    }

    fn provider_runtime_sha256(&self) -> String {
        self.runtime_sha256.clone()
    }

    fn startup_route_snapshot(&self) -> ExecutedModelRouteSnapshot {
        self.route(ModelOperation::CapabilityProbe)
    }

    async fn probe_capabilities(
        &self,
        _cancellation: &ProviderCancellation,
    ) -> ModelProviderResult<ProviderExecution<ProviderCapabilities>> {
        if self.fault == CertificationFault::Timeout {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        Ok(ProviderExecution {
            route: self.route(ModelOperation::CapabilityProbe),
            output: ProviderCapabilities::v1_complete(),
        })
    }

    async fn chat(
        &self,
        request: &ModelChatRequest,
        _cancellation: &ProviderCancellation,
    ) -> ModelProviderResult<ProviderExecution<ModelChatResponse>> {
        self.chat_calls.fetch_add(1, Ordering::SeqCst);
        if self.fault == CertificationFault::Timeout {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        Ok(ProviderExecution {
            route: self.route(ModelOperation::Chat),
            output: self.response_for(request),
        })
    }

    async fn stream_chat(
        &self,
        _request: &ModelChatRequest,
        _sink: &dyn ModelStreamSink,
        _cancellation: &ProviderCancellation,
    ) -> ModelProviderResult<ExecutedModelRouteSnapshot> {
        unreachable!("streaming is not part of the RF01 certification suite")
    }

    async fn embed(
        &self,
        _request: &ModelEmbeddingRequest,
        _cancellation: &ProviderCancellation,
    ) -> ModelProviderResult<ProviderExecution<ModelEmbeddingResponse>> {
        unreachable!("embedding is not part of the RF01 certification suite")
    }
}

pub struct CertificationTestOutcome {
    pub run: CompletedCertificationRun,
    pub provider: Arc<DeterministicCertificationProvider>,
}

pub async fn execute_certification(
    model_id: &str,
    artifact_sha256: &str,
    fault: CertificationFault,
    timeout: Duration,
) -> AgentResult<CertificationTestOutcome> {
    execute_certification_for_provider(
        "rf01_fake_provider",
        ProviderType::Ollama,
        model_id,
        artifact_sha256,
        RUNTIME_SHA256,
        fault,
        timeout,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub async fn execute_certification_for_provider(
    provider_id: &str,
    provider_type: ProviderType,
    model_id: &str,
    artifact_sha256: &str,
    runtime_sha256: &str,
    fault: CertificationFault,
    timeout: Duration,
) -> AgentResult<CertificationTestOutcome> {
    let suite = LocalModelCertificationSuite::keeper_v1();
    let provider = Arc::new(DeterministicCertificationProvider::new_with_identity(
        provider_id,
        provider_type,
        model_id,
        artifact_sha256,
        runtime_sha256,
        fault,
    ));
    let request = CertificationRequest::new(
        "rf01-test-request",
        model_id,
        artifact_sha256,
        provider.provider_id().as_str(),
        provider.provider_type(),
        runtime_sha256,
        suite.suite_id(),
        suite.suite_version(),
    )?;
    let runner = LocalModelCertificationRunner::new(provider.clone(), suite, timeout)?;
    let run = runner
        .run(&request, &ProviderCancellation::default())
        .await?;
    Ok(CertificationTestOutcome { run, provider })
}

pub fn passing_run_blocking(model_id: &str, artifact_sha256: &str) -> CompletedCertificationRun {
    passing_run_for_provider_blocking(
        "rf01_fake_provider",
        ProviderType::Ollama,
        model_id,
        artifact_sha256,
        RUNTIME_SHA256,
    )
}

pub fn passing_run_for_provider_blocking(
    provider_id: &str,
    provider_type: ProviderType,
    model_id: &str,
    artifact_sha256: &str,
    runtime_sha256: &str,
) -> CompletedCertificationRun {
    let provider_id = provider_id.to_owned();
    let model_id = model_id.to_owned();
    let artifact_sha256 = artifact_sha256.to_owned();
    let runtime_sha256 = runtime_sha256.to_owned();
    std::thread::spawn(move || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(execute_certification_for_provider(
                &provider_id,
                provider_type,
                &model_id,
                &artifact_sha256,
                &runtime_sha256,
                CertificationFault::None,
                Duration::from_secs(1),
            ))
            .unwrap()
            .run
    })
    .join()
    .unwrap()
}
