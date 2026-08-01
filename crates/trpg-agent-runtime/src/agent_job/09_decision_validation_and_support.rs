fn validate_structured_decision(
    value: Option<&Value>,
    provider_tool_calls: &[crate::model_provider::ModelToolCall],
    max_tool_calls: usize,
) -> AgentJobResult<AgentStructuredDecision> {
    if provider_tool_calls.len() > max_tool_calls {
        return Err(AgentJobError::terminal("AGENT_TOOL_CALL_LIMIT_EXCEEDED"));
    }
    let value = value.ok_or_else(|| AgentJobError::terminal("AGENT_OUTPUT_SCHEMA_INVALID"))?;
    let mut decision: AgentStructuredDecision = serde_json::from_value(value.clone())
        .map_err(|_| AgentJobError::terminal("AGENT_OUTPUT_SCHEMA_INVALID"))?;
    if decision.kind != "npc_turn"
        || decision.player_visible_text.trim().is_empty()
        || decision.player_visible_text.len() > 16_384
        || decision
            .tool
            .as_ref()
            .is_some_and(|tool| !tool.arguments.is_object())
    {
        return Err(AgentJobError::terminal("AGENT_OUTPUT_SCHEMA_INVALID"));
    }
    if let Some(provider_call) = provider_tool_calls.first() {
        let provider_tool = AgentJobToolCall {
            name: provider_call.name.clone(),
            arguments: provider_call.arguments.clone(),
        };
        if decision
            .tool
            .as_ref()
            .is_some_and(|structured| structured != &provider_tool)
        {
            return Err(AgentJobError::terminal("AGENT_OUTPUT_SCHEMA_INVALID"));
        }
        decision.tool = Some(provider_tool);
    }
    if decision.tool.iter().count() > max_tool_calls {
        return Err(AgentJobError::terminal("AGENT_TOOL_CALL_LIMIT_EXCEEDED"));
    }
    Ok(decision)
}

fn persisted_decision(job: &DurableAgentJob) -> AgentJobResult<AgentStructuredDecision> {
    serde_json::from_str(
        job.decision_json
            .as_deref()
            .ok_or_else(|| AgentJobError::terminal("AGENT_OUTPUT_SCHEMA_INVALID"))?,
    )
    .map_err(|_| AgentJobError::terminal("AGENT_OUTPUT_SCHEMA_INVALID"))
}

fn parse_agent_tool(name: &str) -> AgentJobResult<AgentTool> {
    match name {
        "request_skill_check" => Ok(AgentTool::RequestSkillCheck),
        "reveal_clue" => Ok(AgentTool::RevealClue),
        "apply_san_loss" => Ok(AgentTool::ApplySanLoss),
        "change_scene" => Ok(AgentTool::ChangeScene),
        _ => Err(AgentJobError::terminal("AGENT_TOOL_PERMISSION_DENIED")),
    }
}

fn validate_tool_result(result: &AgentJobToolResult) -> AgentJobResult<()> {
    let result_json = serde_json::to_vec(&result.result)
        .map_err(|_| AgentJobError::terminal("AGENT_TOOL_RESULT_INVALID"))?;
    if result.execution_id.trim().is_empty()
        || !result.result.is_object()
        || result.result_hash != sha256_label(&result_json)
    {
        return Err(AgentJobError::terminal("AGENT_TOOL_RESULT_INVALID"));
    }
    Ok(())
}

fn verify_committed_tool_result(
    events: &[trpg_shared_kernel::EventEnvelope<AgentEventPayload>],
    expected: Option<&AgentJobToolResult>,
) -> AgentJobResult<()> {
    let committed = events.iter().find_map(|event| match &event.payload {
        AgentEventPayload::ToolExecutionSucceeded {
            execution_id,
            result,
            result_hash,
            ..
        } => Some((execution_id.as_str(), result, result_hash.as_str())),
        _ => None,
    });
    match (expected, committed) {
        (None, None) => Ok(()),
        (Some(expected), Some((execution_id, result, result_hash)))
            if execution_id == expected.execution_id
                && result == &expected.result
                && result_hash == expected.result_hash =>
        {
            Ok(())
        }
        _ => Err(AgentJobError::terminal(
            "AGENT_CANONICAL_TOOL_RESULT_MISMATCH",
        )),
    }
}

fn valid_provenance(value: &str) -> bool {
    serde_json::from_str::<Value>(value)
        .ok()
        .and_then(|value| value.as_object().cloned())
        .is_some_and(|object| {
            !object.is_empty() && (object.contains_key("kind") || object.contains_key("source"))
        })
}

fn valid_plain_hash(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn provider_type_name(provider_type: ProviderType) -> &'static str {
    match provider_type {
        ProviderType::Cloud => "cloud",
        ProviderType::Ollama => "ollama",
        ProviderType::LlamaCpp => "llama_cpp",
        ProviderType::LocalOpenAiCompatible => "local_openai_compatible",
    }
}

fn agent_state_can_fail(state: WorkflowState) -> bool {
    matches!(
        state,
        WorkflowState::Claimed
            | WorkflowState::AgentRunning
            | WorkflowState::AwaitingTool
            | WorkflowState::Committing
    )
}

fn map_provider_error(error: ModelProviderError) -> AgentJobError {
    if error.retryable() {
        return AgentJobError::retryable(error.code());
    }
    match error.kind() {
        ModelProviderErrorKind::Cancelled => AgentJobError::terminal("AGENT_JOB_CANCELLED"),
        _ => AgentJobError::terminal(error.code()),
    }
}

fn map_store_error(error: WorkflowStoreError) -> AgentJobError {
    match error {
        WorkflowStoreError::Connection
        | WorkflowStoreError::Database(_)
        | WorkflowStoreError::VersionConflict { .. }
        | WorkflowStoreError::StateConflict => {
            AgentJobError::retryable("AGENT_JOB_STORE_RETRYABLE")
        }
        WorkflowStoreError::NotFound => AgentJobError::terminal("AGENT_JOB_NOT_FOUND"),
        WorkflowStoreError::IdempotencyConflict => {
            AgentJobError::terminal("AGENT_JOB_IDEMPOTENCY_CONFLICT")
        }
        WorkflowStoreError::Configuration(_)
        | WorkflowStoreError::Migration
        | WorkflowStoreError::Validation(_)
        | WorkflowStoreError::IntegrityViolation(_) => {
            AgentJobError::terminal("AGENT_JOB_STORE_INTEGRITY_FAILURE")
        }
    }
}

fn phase_id(state: WorkflowState) -> &'static str {
    match state {
        WorkflowState::AgentRunning => "resume_running",
        WorkflowState::AwaitingTool => "resume_awaiting_tool",
        WorkflowState::Committing => "resume_committing",
        _ => "resume",
    }
}

fn retry_delay_ms(attempt: i32) -> i64 {
    let shift = u32::try_from(attempt.clamp(0, 6)).unwrap_or(0);
    1_000_i64.saturating_mul(1_i64 << shift)
}

fn duration_millis_i64(duration: Duration) -> AgentJobResult<i64> {
    i64::try_from(duration.as_millis())
        .map_err(|_| AgentJobError::terminal("AGENT_JOB_CONFIGURATION_INVALID"))
}

fn observed_now(started_unix_ms: i64, started: Instant) -> AgentJobResult<i64> {
    started_unix_ms
        .checked_add(
            i64::try_from(started.elapsed().as_millis())
                .map_err(|_| AgentJobError::terminal("AGENT_JOB_TIME_OVERFLOW"))?,
        )
        .ok_or_else(|| AgentJobError::terminal("AGENT_JOB_TIME_OVERFLOW"))
}

fn sha256_label(value: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(value))
}
