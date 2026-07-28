
fn persist_agent_formal_batch(
    store: &mut EventStore<AgentEventPayload>,
    command: &CommandEnvelope<AgentDecision>,
    authorization: &FormalAuthorization,
    canonical: &Arc<dyn CanonicalCommitPort>,
    events: Vec<(
        CommandEnvelope<AgentDecision>,
        &'static str,
        AgentEventPayload,
    )>,
) -> AgentResult<Vec<EventEnvelope<AgentEventPayload>>> {
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
            .collect::<Result<Vec<_>, TrpgError>>()?,
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
        return Err(AgentError::Core(TrpgError::AuditIntegrityViolation));
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
            return Err(AgentError::Core(TrpgError::AuditIntegrityViolation));
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

pub fn replay_agent_events_for_principal(
    store: &EventStore<AgentEventPayload>,
    authorization: &ReplayAuthorization,
    now_unix_ms: u64,
) -> AgentResult<Vec<EventEnvelope<AgentEventPayload>>> {
    store.replay_visible(authorization, now_unix_ms)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContextFact {
    pub fact_id: EntityId,
    pub text: String,
    pub visibility: Visibility,
    pub fact_provenance: FactProvenance,
}

impl ContextFact {
    pub fn new(
        fact_id: impl Into<String>,
        text: impl Into<String>,
        visibility: Visibility,
        fact_provenance: FactProvenance,
    ) -> Result<Self, TrpgError> {
        Ok(Self {
            fact_id: EntityId::new(fact_id)?,
            text: text.into(),
            visibility,
            fact_provenance,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssembledAgentContext {
    pub facts: Vec<ContextFact>,
    pub derived_visibility: Visibility,
    pub strictest_visibility: VisibilityLabel,
}

pub fn assemble_context(
    facts: &[ContextFact],
    processor: &PrincipalScope,
    target_audience: &PrincipalScope,
) -> AssembledAgentContext {
    assemble_context_for_audience(facts, processor, target_audience)
}

/// Assembles context for a declared target audience. A System/Keeper worker
/// may process more sources than the target can receive, but those sources are
/// omitted before context construction and cannot influence generated text.
pub fn assemble_context_for_audience(
    facts: &[ContextFact],
    processor: &PrincipalScope,
    target_audience: &PrincipalScope,
) -> AssembledAgentContext {
    let visible: Vec<ContextFact> = facts
        .iter()
        .filter(|fact| {
            let sources = [fact.visibility.clone()];
            evaluate_derived_visibility(DerivationRequest {
                sources: &sources,
                processor,
                target_audience,
                target: SecurityDerivedObject::AgentContext,
            })
            .outcome
                == SecurityRedactionOutcome::Visible
        })
        .cloned()
        .collect();
    let derived_visibility = visible
        .iter()
        .map(|fact| fact.visibility.clone())
        .reduce(|current, candidate| current.intersection(&candidate))
        .unwrap_or_else(|| Visibility::new(VisibilityLabel::Public));
    let strictest_visibility = derived_visibility.label().clone();

    AssembledAgentContext {
        facts: visible,
        derived_visibility,
        strictest_visibility,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PromptInjectionReport {
    pub detected: bool,
    pub keeper_truth_leaked: bool,
    pub audit_flag: Option<&'static str>,
    pub player_visible_text: String,
}

pub fn evaluate_prompt_injection(input: &str, generated_text: &str) -> PromptInjectionReport {
    let detected = input.contains("忽略以上规则")
        || input.contains("keeper_truth")
        || input.to_ascii_lowercase().contains("ignore previous");
    let player_visible_text = redact_player_visible_text(generated_text);

    PromptInjectionReport {
        detected,
        keeper_truth_leaked: false,
        audit_flag: detected.then_some("prompt_injection_detected"),
        player_visible_text,
    }
}
