#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentJobEnqueueDraft {
    pub job_id: String,
    pub campaign_id: String,
    pub actor_id: String,
    pub agent_kind: String,
    pub authority_contract_id: String,
    pub authority_mode: String,
    pub authority_contract_version: i64,
    pub input_event_sequence: i64,
    pub input_stream_version: i64,
    pub visibility_scope_json: String,
    pub rag_snapshot_id: String,
    pub provider_id: String,
    pub provider_type: String,
    pub model_id: String,
    pub model_artifact_sha256: String,
    pub route_authorization_event_id: String,
    pub prompt_template_id: String,
    pub prompt_template_version: String,
    pub tool_schema_version: String,
    pub idempotency_key: String,
    pub deadline_unix_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DurableAgentJob {
    pub job_id: String,
    pub campaign_id: String,
    pub actor_id: String,
    pub agent_kind: String,
    pub authority_contract_id: String,
    pub authority_mode: String,
    pub authority_contract_version: i64,
    pub input_event_sequence: i64,
    pub input_stream_id: String,
    pub input_stream_version: i64,
    pub visibility_scope_json: String,
    pub rag_snapshot_id: String,
    pub provider_id: String,
    pub provider_type: String,
    pub model_id: String,
    pub model_artifact_sha256: String,
    pub route_authorization_event_id: String,
    pub prompt_template_id: String,
    pub prompt_template_version: String,
    pub tool_schema_version: String,
    pub idempotency_key: String,
    pub deadline_unix_ms: i64,
    pub state: WorkflowState,
    pub resume_state: Option<WorkflowState>,
    pub version: i64,
    pub claim_owner: Option<String>,
    pub claim_token: Option<String>,
    pub lease_expires_at_unix_ms: Option<i64>,
    pub heartbeat_at_unix_ms: Option<i64>,
    pub attempt: i32,
    pub next_attempt_at_unix_ms: Option<i64>,
    pub decision_json: Option<String>,
    pub tool_result_json: Option<String>,
    pub linked_event_sequences: Vec<i64>,
    pub cancellation_requested_at_unix_ms: Option<i64>,
    pub error_code: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentJobTransitionDraft {
    pub job_id: String,
    pub claim_owner: String,
    pub claim_token: String,
    pub expected_version: i64,
    pub from_state: WorkflowState,
    pub to_state: WorkflowState,
    pub idempotency_key: String,
    pub correlation_id: String,
    pub causation_id: String,
    pub decision_json: Option<String>,
    pub tool_result_json: Option<String>,
    pub linked_event_sequences: Option<Vec<i64>>,
    pub error_code: Option<String>,
    pub next_attempt_at_unix_ms: Option<i64>,
    pub now_unix_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentJobEvidenceDraft {
    pub job_id: String,
    pub attempt: i32,
    pub phase: String,
    pub model_id: String,
    pub runtime_version: String,
    pub prompt_template_hash: String,
    pub tool_schema_hash: String,
    pub retrieval_hash: String,
    pub input_hash: String,
    pub output_hash: String,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub latency_ms: i64,
    pub tool_call_count: i32,
    pub linked_event_sequences: Vec<i64>,
    pub visibility_label: String,
    pub retention_until_unix_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DurableAgentContextChunk {
    pub chunk_id: String,
    pub source_event_sequence: i64,
    pub visibility_label: String,
    pub visibility_subject: Option<String>,
    pub fact_provenance_json: String,
    pub chunk_hash: String,
    pub content: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DurableAgentContextSnapshot {
    pub input_payload_json: String,
    pub chunks: Vec<DurableAgentContextChunk>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DurableAgentAuthoritySnapshot {
    pub contract_id: String,
    pub campaign_id: String,
    pub authority_mode: String,
    pub authority_owner: String,
    pub contract_version: i64,
    pub prompt_version: String,
    pub agent_pack_version: String,
    pub tool_schema_version: String,
    pub model_route_snapshot: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DurableAgentApproval {
    pub approval_id: String,
    pub approval_event_sequence: i64,
    pub approved_by: String,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentJobApprovalDraft {
    pub approval_id: String,
    pub job_id: String,
    pub approval_event_sequence: i64,
    pub approved_by: String,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentJobSkillCheckDraft {
    pub job_id: String,
    pub claim_owner: String,
    pub claim_token: String,
    pub expected_attempt: i32,
    pub idempotency_key: String,
    pub character_id: String,
    pub skill_name: String,
    pub adjustment: String,
    pub now_unix_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentJobSkillCheckRollDraft {
    pub execution_id: String,
    pub roll: u8,
    pub selected_tens_digit: u8,
    pub ones_digit: u8,
    pub success_level: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DurableAgentJobToolReceipt {
    pub execution_id: String,
    pub result_json: String,
    pub result_hash: String,
}
