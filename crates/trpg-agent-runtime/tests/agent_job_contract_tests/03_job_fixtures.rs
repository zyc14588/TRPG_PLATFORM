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
