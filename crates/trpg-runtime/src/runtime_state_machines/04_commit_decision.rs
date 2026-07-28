
pub fn commit_decision(
    store: &mut EventStore<RuntimeEventPayload>,
    contract: &AuthorityContract,
    command: &CommandEnvelope<RuntimeDecision>,
    workflow_authentication: &AuthenticationContext,
    decision: RuntimeDecision,
    now_unix_ms: u64,
) -> RuntimeResult<Vec<EventEnvelope<RuntimeEventPayload>>> {
    validate_runtime_command(contract, command)?;
    if command.payload != decision {
        return Err(RuntimeError::Core(TrpgError::DecisionDraftChanged));
    }
    if !decision.tool_request.is_formal_state_change() {
        return Err(RuntimeError::Core(TrpgError::PolicyDenied));
    }
    if command.authority_mode == AuthorityMode::HumanKp {
        return Err(RuntimeError::Core(TrpgError::DecisionConfirmationRequired));
    }
    append_committed_decision_events(
        store,
        command,
        workflow_authentication,
        decision,
        false,
        None,
        now_unix_ms,
    )
}

fn append_committed_decision_events(
    store: &mut EventStore<RuntimeEventPayload>,
    command: &CommandEnvelope<RuntimeDecision>,
    workflow_authentication: &AuthenticationContext,
    decision: RuntimeDecision,
    human_confirmed: bool,
    authorizing_authentication: Option<&AuthenticationContext>,
    now_unix_ms: u64,
) -> RuntimeResult<Vec<EventEnvelope<RuntimeEventPayload>>> {
    let grant = if human_confirmed {
        ToolGrantDecision::allow()
    } else {
        approve_tool_request(&command.authority_mode, &decision.tool_request)?
    };

    // Derived event versions are part of the original request hash. Keeping
    // them anchored to the caller's expected_version lets EventStore return
    // the original events before checking the now-advanced stream version.
    let next_version = command.expected_version;
    let tool_command = derived_command(command, "tool", next_version)?;
    let requested_role = if human_confirmed {
        "human_keeper"
    } else {
        actor_role_name(command.actor.role())
    };
    let (authorization, canonical, tool_executor) = {
        let custody = store.formal_custody()?;
        (
            custody.authorizer.authorize(
                workflow_authentication,
                authorizing_authentication,
                command,
                requested_role,
                now_unix_ms,
            )?,
            Arc::clone(&custody.canonical),
            Arc::clone(&custody.tool_executor),
        )
    };
    // Resolve an exact prior command under canonical custody before repeating
    // a non-idempotent tool (for example server dice). Authorization above is
    // also guaranteed to complete before either lookup or execution.
    let commit_key = CanonicalCommitKey {
        commit_id: format!(
            "{}_{}",
            authorization.contract().campaign_id().as_str(),
            command.command_id.as_str()
        ),
        campaign_id: authorization.contract().campaign_id().to_string(),
        stream_id: command
            .authenticated_context()
            .resource()
            .resource_id()
            .to_string(),
        idempotency_key: command.idempotency_key.clone(),
        expected_version: command.expected_version,
    };
    let execution = match canonical.load_receipt(&commit_key)? {
        Some(receipt) => {
            runtime_execution_from_receipt(&receipt, decision.tool_request.tool().as_str())?
        }
        None => tool_executor.execute(&decision)?,
    };
    let execution_id = EntityId::new(&execution.execution_id)?;
    if !execution.result_hash.starts_with("sha256:")
        || execution.result_hash.len() != 71
        || !execution
            .result_hash
            .bytes()
            .skip(7)
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(RuntimeError::Core(TrpgError::InvalidConfiguration(
            "runtime_tool_execution_result",
        )));
    }
    let execution_command = derived_command(command, "execution", next_version + 1)?;
    let decision_command = derived_command(command, "decision", next_version + 2)?;
    persist_runtime_formal_batch(
        store,
        command,
        &authorization,
        &canonical,
        vec![
            (
                tool_command,
                "ToolRequestApproved",
                RuntimeEventPayload::ToolRequestApproved {
                    tool: decision.tool_request.tool().as_str(),
                    grant: grant.clone(),
                    seal: RuntimeFormalEventSeal::new(),
                },
            ),
            (
                execution_command,
                "ToolExecutionSucceeded",
                RuntimeEventPayload::ToolExecutionSucceeded {
                    tool: decision.tool_request.tool().as_str(),
                    execution_id,
                    result_hash: execution.result_hash,
                    seal: RuntimeFormalEventSeal::new(),
                },
            ),
            (
                decision_command,
                "DecisionCommitted",
                RuntimeEventPayload::DecisionCommitted {
                    decision_id: decision.decision_id,
                    linked_records: decision.linked_records,
                    player_visible_explanation: decision.player_visible_explanation,
                    audit_fields: decision.audit_fields,
                    seal: RuntimeFormalEventSeal::new(),
                },
            ),
        ],
    )
}

fn runtime_execution_from_receipt(
    receipt: &CanonicalCommitReceipt,
    expected_tool: &str,
) -> RuntimeResult<RuntimeToolExecutionOutput> {
    if receipt.events.len() != 3 {
        return Err(RuntimeError::Core(TrpgError::AuditIntegrityViolation));
    }
    let event = receipt
        .events
        .get(1)
        .ok_or(TrpgError::AuditIntegrityViolation)?;
    if event.event_type != "ToolExecutionSucceeded" {
        return Err(RuntimeError::Core(TrpgError::AuditIntegrityViolation));
    }
    let payload: serde_json::Value = serde_json::from_str(&event.payload_json)
        .map_err(|_| TrpgError::AuditIntegrityViolation)?;
    let execution = payload
        .get("ToolExecutionSucceeded")
        .and_then(serde_json::Value::as_object)
        .ok_or(TrpgError::AuditIntegrityViolation)?;
    let tool = execution
        .get("tool")
        .and_then(serde_json::Value::as_str)
        .ok_or(TrpgError::AuditIntegrityViolation)?;
    let execution_id = execution
        .get("execution_id")
        .and_then(serde_json::Value::as_str)
        .ok_or(TrpgError::AuditIntegrityViolation)?;
    let result_hash = execution
        .get("result_hash")
        .and_then(serde_json::Value::as_str)
        .ok_or(TrpgError::AuditIntegrityViolation)?;
    if tool != expected_tool {
        return Err(RuntimeError::Core(TrpgError::AuditIntegrityViolation));
    }
    Ok(RuntimeToolExecutionOutput {
        execution_id: execution_id.to_owned(),
        result_hash: result_hash.to_owned(),
    })
}

fn persist_runtime_formal_batch(
    store: &mut EventStore<RuntimeEventPayload>,
    command: &CommandEnvelope<RuntimeDecision>,
    authorization: &FormalAuthorization,
    canonical: &Arc<dyn CanonicalCommitPort>,
    events: Vec<(
        CommandEnvelope<RuntimeDecision>,
        &'static str,
        RuntimeEventPayload,
    )>,
) -> RuntimeResult<Vec<EventEnvelope<RuntimeEventPayload>>> {
    let contract = authorization.contract();
    let request = CanonicalCommitRequest {
        commit_id: format!(
            "{}_{}",
            contract.campaign_id().as_str(),
            command.command_id.as_str()
        ),
        campaign_id: contract.campaign_id().to_string(),
        idempotency_key: command.idempotency_key.clone(),
        expected_version: command.expected_version,
        command_id: command.command_id.to_string(),
        authenticated_actor_id: command.actor.id().to_string(),
        authenticated_actor_role: command.actor.canonical_role_name().to_owned(),
        authenticated_actor_origin: command.actor.canonical_origin_wire(),
        authority_mode: authority_mode_name(&command.authority_mode).to_owned(),
        authority_contract_version: contract.version(),
        authority_contract_id: contract.contract_id().to_string(),
        authority_owner: contract.authority_owner().to_string(),
        visibility_label: command.visibility.label().as_str().to_owned(),
        visibility_subject: command
            .visibility
            .subject_id()
            .map(ToString::to_string)
            .unwrap_or_else(|| "not_applicable".to_owned()),
        data_subject_id: "not_applicable".to_owned(),
        provenance_kind: provenance_kind_name(&command.fact_provenance.kind).to_owned(),
        provenance_reference: command.fact_provenance.reference.to_string(),
        provenance_recorded_by: command.fact_provenance.recorded_by.to_string(),
        correlation_id: command.correlation_id.to_string(),
        causation_id: command.causation_id.to_string(),
        trace_id: command.authenticated_context().trace_id().to_string(),
        events: events
            .iter()
            .map(|(_, event_type, payload)| {
                Ok(CanonicalCommitEvent {
                    event_type: (*event_type).to_owned(),
                    payload_json: serde_json::to_string(payload)
                        .map_err(|_| TrpgError::AuditIntegrityViolation)?,
                })
            })
            .collect::<KernelResult<Vec<_>>>()?,
        audit: authorization.canonical_audit().clone(),
    };
    let receipt = canonical.commit(&request)?;
    canonical.verify_receipt(&request, &receipt)?;
    let expected_first = command
        .expected_version
        .checked_add(1)
        .ok_or(TrpgError::AuditIntegrityViolation)?;
    let expected_last = command
        .expected_version
        .checked_add(events.len() as u64)
        .ok_or(TrpgError::AuditIntegrityViolation)?;
    if receipt.first_stream_version != expected_first
        || receipt.last_stream_version != expected_last
        || receipt.events.len() != events.len()
    {
        return Err(RuntimeError::Core(TrpgError::AuditIntegrityViolation));
    }
    // Validate the complete durable receipt before publishing any part of the
    // formal batch into the process-local read model. A faulty adapter must
    // not make event one visible when event two is malformed.
    let mut previous_sequence = 0;
    for (index, ((_, event_type, payload), durable)) in
        events.iter().zip(receipt.events.iter()).enumerate()
    {
        let expected_payload =
            serde_json::to_value(payload).map_err(|_| TrpgError::AuditIntegrityViolation)?;
        let durable_payload: serde_json::Value = serde_json::from_str(&durable.payload_json)
            .map_err(|_| TrpgError::AuditIntegrityViolation)?;
        let expected_version = expected_first
            .checked_add(index as u64)
            .ok_or(TrpgError::AuditIntegrityViolation)?;
        if durable.sequence == 0
            || durable.sequence <= previous_sequence
            || durable.occurred_at_unix_ms == 0
            || durable.stream_version != expected_version
            || durable.event_type != *event_type
            || durable_payload != expected_payload
            || durable.command_id != request.command_id
            || durable.idempotency_key != format!("{}:{index:04}", request.idempotency_key)
        {
            return Err(RuntimeError::Core(TrpgError::AuditIntegrityViolation));
        }
        previous_sequence = durable.sequence;
    }

    let mut candidate = store.inner.clone();
    let mut appended = Vec::with_capacity(events.len());
    for ((event_command, event_type, payload), durable) in
        events.into_iter().zip(receipt.events.iter())
    {
        appended.push(candidate.record_canonical(&event_command, event_type, payload, durable)?);
    }
    store.inner = candidate;
    Ok(appended)
}

fn authority_mode_name(mode: &AuthorityMode) -> &'static str {
    match mode {
        AuthorityMode::HumanKp => "human_kp",
        AuthorityMode::AiKp => "ai_kp",
    }
}

fn provenance_kind_name(kind: &ProvenanceKind) -> &'static str {
    match kind {
        ProvenanceKind::UserStatement => "user_statement",
        ProvenanceKind::HumanKeeperStatement => "human_keeper_statement",
        ProvenanceKind::RulesEngineDecision => "rules_engine_decision",
        ProvenanceKind::ToolResult => "tool_result",
        ProvenanceKind::AgentProposal => "agent_proposal",
        ProvenanceKind::ImportedSource => "imported_source",
        ProvenanceKind::SystemFixture => "system_fixture",
    }
}

fn actor_role_name(role: &ActorRole) -> &'static str {
    match role {
        ActorRole::ServerOwner => "server_owner",
        ActorRole::CampaignOwner => "campaign_owner",
        ActorRole::HumanKeeper => "human_keeper",
        ActorRole::AiKeeper => "ai_keeper",
        ActorRole::Investigator => "investigator",
        ActorRole::Moderator => "moderator",
        ActorRole::Spectator => "spectator",
        ActorRole::Workflow => "workflow",
        ActorRole::RulesEngine => "rules_engine",
        ActorRole::System => "system",
    }
}

pub fn replay_visible_runtime_events(
    store: &EventStore<RuntimeEventPayload>,
    authorization: &ReplayAuthorization,
    now_unix_ms: u64,
) -> RuntimeResult<Vec<EventEnvelope<RuntimeEventPayload>>> {
    store.replay_visible(authorization, now_unix_ms)
}
