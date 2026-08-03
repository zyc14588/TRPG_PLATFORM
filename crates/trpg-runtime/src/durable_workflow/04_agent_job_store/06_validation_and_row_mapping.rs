fn validate_agent_job_enqueue(
    draft: &AgentJobEnqueueDraft,
) -> Result<(), WorkflowStoreError> {
    for (value, reason) in [
        (&draft.job_id, "job_id_required"),
        (&draft.campaign_id, "campaign_id_required"),
        (&draft.actor_id, "actor_id_required"),
        (&draft.agent_kind, "agent_kind_required"),
        (&draft.authority_contract_id, "authority_contract_id_required"),
        (&draft.authority_mode, "authority_mode_required"),
        (&draft.rag_snapshot_id, "rag_snapshot_id_required"),
        (&draft.provider_id, "provider_id_required"),
        (&draft.provider_type, "provider_type_required"),
        (&draft.model_id, "model_id_required"),
        (
            &draft.route_authorization_event_id,
            "route_authorization_event_id_required",
        ),
        (&draft.prompt_template_id, "prompt_template_id_required"),
        (
            &draft.prompt_template_version,
            "prompt_template_version_required",
        ),
        (&draft.tool_schema_version, "tool_schema_version_required"),
        (&draft.idempotency_key, "idempotency_key_required"),
    ] {
        validate_identifier(value, reason)?;
    }
    if draft.authority_contract_version <= 0
        || draft.input_event_sequence <= 0
        || draft.input_stream_version < 0
        || draft.deadline_unix_ms <= 0
        || !matches!(
            draft.authority_mode.as_str(),
            "AI_KP" | "HUMAN_KP"
        )
        || !matches!(
            draft.provider_type.as_str(),
            "cloud" | "ollama" | "llama_cpp"
        )
        || !matches!(
            draft.agent_kind.as_str(),
            "ai_keeper_orchestrator" | "keeper_copilot"
        )
        || !valid_labelled_sha256(&draft.model_artifact_sha256)
    {
        return Err(WorkflowStoreError::Validation(
            "invalid_agent_job_binding",
        ));
    }
    Ok(())
}

fn validate_visibility_scope(scope: &Value) -> Result<(), WorkflowStoreError> {
    let Some(scope) = scope.as_object() else {
        return Err(WorkflowStoreError::Validation(
            "invalid_visibility_scope",
        ));
    };
    let allowed_labels = scope
        .get("allowed_labels")
        .and_then(Value::as_array)
        .filter(|labels| !labels.is_empty() && labels.len() <= 11)
        .ok_or(WorkflowStoreError::Validation(
            "invalid_visibility_scope",
        ))?;
    let output_label = scope
        .get("output_label")
        .and_then(Value::as_str)
        .filter(|label| !label.trim().is_empty())
        .ok_or(WorkflowStoreError::Validation(
            "invalid_visibility_scope",
        ))?;
    if scope.len() != 3
        || !["allowed_labels", "output_label", "subject_id"]
            .iter()
            .all(|field| scope.contains_key(*field))
        || allowed_labels.iter()
        .any(|label| label.as_str().is_none_or(|label| label.trim().is_empty()))
        || !allowed_labels
            .iter()
            .any(|label| label.as_str() == Some(output_label))
        || !scope.contains_key("subject_id")
        || scope
            .get("subject_id")
            .is_some_and(|subject| !subject.is_null() && subject.as_str().is_none())
        || scope
            .get("subject_id")
            .and_then(Value::as_str)
            .is_some_and(|subject| subject.trim().is_empty())
    {
        return Err(WorkflowStoreError::Validation(
            "invalid_visibility_scope",
        ));
    }
    Ok(())
}

fn validate_agent_job_transition(
    draft: &AgentJobTransitionDraft,
) -> Result<(), WorkflowStoreError> {
    for (value, reason) in [
        (&draft.job_id, "job_id_required"),
        (&draft.claim_owner, "claim_owner_required"),
        (&draft.claim_token, "claim_token_required"),
        (&draft.idempotency_key, "idempotency_key_required"),
        (&draft.correlation_id, "correlation_id_required"),
        (&draft.causation_id, "causation_id_required"),
    ] {
        validate_identifier(value, reason)?;
    }
    if draft.expected_version < 0
        || draft.now_unix_ms < 0
        || !draft.from_state.can_transition_to(draft.to_state)
    {
        return Err(WorkflowStoreError::Validation(
            "invalid_agent_job_transition",
        ));
    }
    validate_timestamp(draft.next_attempt_at_unix_ms)
}

fn validate_agent_job_evidence(
    draft: &AgentJobEvidenceDraft,
) -> Result<(), WorkflowStoreError> {
    if draft.attempt <= 0
        || draft.input_tokens < 0
        || draft.output_tokens < 0
        || draft.latency_ms < 0
        || draft.tool_call_count < 0
        || draft.retention_until_unix_ms <= 0
        || !matches!(
            draft.phase.as_str(),
            "authority"
                | "context"
                | "provider"
                | "tool"
                | "canonical_commit"
                | "completed"
                | "failed"
        )
        || [
            &draft.prompt_template_hash,
            &draft.tool_schema_hash,
            &draft.retrieval_hash,
            &draft.input_hash,
            &draft.output_hash,
        ]
        .iter()
        .any(|hash| !valid_labelled_sha256(hash))
    {
        return Err(WorkflowStoreError::Validation(
            "invalid_agent_job_evidence",
        ));
    }
    Ok(())
}

fn valid_plain_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn valid_labelled_sha256(value: &str) -> bool {
    value
        .strip_prefix("sha256:")
        .is_some_and(valid_plain_sha256)
}

fn labelled_sha256(value: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(value))
}

fn agent_job_binding_matches(
    existing: &DurableAgentJob,
    draft: &AgentJobEnqueueDraft,
    visibility_scope_json: &str,
) -> bool {
    existing.job_id == draft.job_id
        && existing.campaign_id == draft.campaign_id
        && existing.actor_id == draft.actor_id
        && existing.agent_kind == draft.agent_kind
        && existing.authority_contract_id == draft.authority_contract_id
        && existing.authority_mode == draft.authority_mode
        && existing.authority_contract_version == draft.authority_contract_version
        && existing.input_event_sequence == draft.input_event_sequence
        && existing.input_stream_version == draft.input_stream_version
        && json_values_equal(
            &existing.visibility_scope_json,
            visibility_scope_json,
        )
        && existing.rag_snapshot_id == draft.rag_snapshot_id
        && existing.provider_id == draft.provider_id
        && existing.provider_type == draft.provider_type
        && existing.model_id == draft.model_id
        && existing.model_artifact_sha256 == draft.model_artifact_sha256
        && existing.route_authorization_event_id == draft.route_authorization_event_id
        && existing.prompt_template_id == draft.prompt_template_id
        && existing.prompt_template_version == draft.prompt_template_version
        && existing.tool_schema_version == draft.tool_schema_version
        && existing.idempotency_key == draft.idempotency_key
        && existing.deadline_unix_ms == draft.deadline_unix_ms
}

fn json_values_equal(left: &str, right: &str) -> bool {
    serde_json::from_str::<Value>(left)
        .ok()
        .zip(serde_json::from_str::<Value>(right).ok())
        .is_some_and(|(left, right)| left == right)
}

fn agent_job_select_sql() -> &'static str {
    r#"
    SELECT job.job_id, job.campaign_id, job.actor_id, job.agent_kind,
           job.authority_contract_id, job.authority_mode,
           job.authority_contract_version, job.input_event_sequence,
           source.stream_id AS input_stream_id,
           job.input_stream_version, job.visibility_scope::text
               AS visibility_scope_json,
           job.rag_snapshot_id, job.provider_id, job.provider_type,
           job.model_id, job.model_artifact_sha256,
           job.route_authorization_event_id, job.prompt_template_id,
           job.prompt_template_version, job.tool_schema_version,
           job.idempotency_key,
           (extract(epoch FROM job.deadline_at) * 1000)::bigint
               AS deadline_unix_ms,
           workflow.state, job.resume_state, workflow.version,
           workflow.lease_owner AS claim_owner, workflow.claim_token,
           CASE WHEN workflow.lease_expires_at IS NULL THEN NULL
                ELSE (extract(epoch FROM workflow.lease_expires_at) * 1000)::bigint
                END AS lease_expires_at_unix_ms,
           CASE WHEN workflow.heartbeat_at IS NULL THEN NULL
                ELSE (extract(epoch FROM workflow.heartbeat_at) * 1000)::bigint
                END AS heartbeat_at_unix_ms,
           workflow.attempt,
           CASE WHEN workflow.next_attempt_at IS NULL THEN NULL
                ELSE (extract(epoch FROM workflow.next_attempt_at) * 1000)::bigint
                END AS next_attempt_at_unix_ms,
           job.decision_json::text AS decision_json,
           job.tool_result_json::text AS tool_result_json,
           job.linked_event_sequences,
           CASE WHEN job.cancellation_requested_at IS NULL THEN NULL
                ELSE (extract(epoch FROM job.cancellation_requested_at) * 1000)::bigint
                END AS cancellation_requested_at_unix_ms,
           job.error_code
      FROM agent_jobs AS job
      JOIN workflow_instances AS workflow ON workflow.workflow_id = job.job_id
      JOIN event_store AS source ON source.sequence = job.input_event_sequence
     WHERE job.job_id = $1
    "#
}

async fn load_agent_job_in_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    job_id: &str,
) -> Result<Option<DurableAgentJob>, WorkflowStoreError> {
    let row = sqlx::query(agent_job_select_sql())
        .bind(job_id)
        .fetch_optional(&mut **transaction)
        .await
        .map_err(|_| WorkflowStoreError::Database("load_agent_job_transaction"))?;
    row.as_ref().map(agent_job_from_row).transpose()
}

fn agent_job_from_row(
    row: &sqlx::postgres::PgRow,
) -> Result<DurableAgentJob, WorkflowStoreError> {
    let resume_state = row
        .get::<Option<String>, _>("resume_state")
        .as_deref()
        .map(WorkflowState::parse)
        .transpose()?;
    Ok(DurableAgentJob {
        job_id: row.get("job_id"),
        campaign_id: row.get("campaign_id"),
        actor_id: row.get("actor_id"),
        agent_kind: row.get("agent_kind"),
        authority_contract_id: row.get("authority_contract_id"),
        authority_mode: row.get("authority_mode"),
        authority_contract_version: row.get("authority_contract_version"),
        input_event_sequence: row.get("input_event_sequence"),
        input_stream_id: row.get("input_stream_id"),
        input_stream_version: row.get("input_stream_version"),
        visibility_scope_json: row.get("visibility_scope_json"),
        rag_snapshot_id: row.get("rag_snapshot_id"),
        provider_id: row.get("provider_id"),
        provider_type: row.get("provider_type"),
        model_id: row.get("model_id"),
        model_artifact_sha256: row.get("model_artifact_sha256"),
        route_authorization_event_id: row.get("route_authorization_event_id"),
        prompt_template_id: row.get("prompt_template_id"),
        prompt_template_version: row.get("prompt_template_version"),
        tool_schema_version: row.get("tool_schema_version"),
        idempotency_key: row.get("idempotency_key"),
        deadline_unix_ms: row.get("deadline_unix_ms"),
        state: WorkflowState::parse(row.get::<String, _>("state").as_str())?,
        resume_state,
        version: row.get("version"),
        claim_owner: row.get("claim_owner"),
        claim_token: row.get("claim_token"),
        lease_expires_at_unix_ms: row.get("lease_expires_at_unix_ms"),
        heartbeat_at_unix_ms: row.get("heartbeat_at_unix_ms"),
        attempt: row.get("attempt"),
        next_attempt_at_unix_ms: row.get("next_attempt_at_unix_ms"),
        decision_json: row.get("decision_json"),
        tool_result_json: row.get("tool_result_json"),
        linked_event_sequences: row.get("linked_event_sequences"),
        cancellation_requested_at_unix_ms: row.get("cancellation_requested_at_unix_ms"),
        error_code: row.get("error_code"),
    })
}
