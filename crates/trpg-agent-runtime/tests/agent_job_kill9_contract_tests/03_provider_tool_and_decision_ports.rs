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

    fn provider_runtime_sha256(&self) -> String {
        "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc".to_owned()
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
        _now_unix_ms: i64,
    ) -> Result<AgentJobToolResult, AgentJobError> {
        crash_boundary("tool_before");
        let path = self.root.join("tool-receipt");
        write_once(&path, idempotency_key.as_bytes());
        Ok(AgentJobToolResult {
            execution_id: "tool_execution_ar09_kill9".to_owned(),
            result: serde_json::json!({}),
            result_hash: "sha256:44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a"
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
