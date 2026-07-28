
fn confirm_pending_decision(
    pending: &PendingDecision,
    contract: &AuthorityContract,
    identity_verifier: &IdentityVerifier,
    authentication: &AuthenticationContext,
    submitted_command: &CommandEnvelope<RuntimeDecision>,
    now_unix_ms: u64,
) -> RuntimeResult<ConfirmedPendingDecision> {
    if pending.status != PendingDecisionStatus::AwaitingHumanConfirmation {
        return Err(RuntimeError::Core(TrpgError::DecisionConfirmationRequired));
    }
    let binding = validate_pending_binding(pending, contract, now_unix_ms)?;
    identity_verifier
        .verify(authentication, now_unix_ms)
        .map_err(|_| RuntimeError::Core(TrpgError::InternalIdentityInvalid))?;
    let PrincipalKind::UserSession { session_id, .. } = authentication.kind() else {
        return Err(RuntimeError::Core(TrpgError::InternalIdentityInvalid));
    };
    if authentication.subject_id() != &binding.authority_owner {
        return Err(RuntimeError::Core(TrpgError::AuthorityOwnerMismatch));
    }
    let confirmer = Actor::authenticated_user(
        authentication.subject_id().as_str(),
        ActorRole::HumanKeeper,
        session_id.as_str(),
    )?;
    if canonical_commit_draft_hash(submitted_command) != binding.draft_hash
        || submitted_command.payload != pending.decision
    {
        return Err(RuntimeError::Core(TrpgError::DecisionDraftChanged));
    }
    let mut confirmed_pending = pending.clone();
    confirmed_pending.status = PendingDecisionStatus::ReadyToCommit;
    Ok(ConfirmedPendingDecision {
        pending: confirmed_pending,
        confirmed_by: confirmer,
        confirmation_authentication: authentication.clone(),
        confirmed_at_unix_ms: now_unix_ms,
        committed: false,
    })
}

fn commit_confirmed_decision(
    store: &mut EventStore<RuntimeEventPayload>,
    contract: &AuthorityContract,
    command: &CommandEnvelope<RuntimeDecision>,
    workflow_authentication: &AuthenticationContext,
    confirmed: &mut ConfirmedPendingDecision,
    submitted_decision: RuntimeDecision,
    now_unix_ms: u64,
) -> RuntimeResult<Vec<EventEnvelope<RuntimeEventPayload>>> {
    let binding = validate_pending_binding(&confirmed.pending, contract, now_unix_ms)?;
    if canonical_commit_draft_hash(command) != binding.draft_hash
        || command.payload != submitted_decision
    {
        return Err(RuntimeError::Core(TrpgError::DecisionDraftChanged));
    }
    if confirmed.confirmed_by.id() != contract.authority_owner()
        || confirmed.confirmed_by.role() != &ActorRole::HumanKeeper
    {
        return Err(RuntimeError::Core(TrpgError::AuthorityOwnerMismatch));
    }
    validate_runtime_command(contract, command)?;
    let events = append_committed_decision_events(
        store,
        command,
        workflow_authentication,
        submitted_decision,
        true,
        Some(&confirmed.confirmation_authentication),
        now_unix_ms,
    )?;
    confirmed.pending.status = PendingDecisionStatus::Committed;
    confirmed.committed = true;
    Ok(events)
}

fn validate_pending_binding<'a>(
    pending: &'a PendingDecision,
    contract: &AuthorityContract,
    now_unix_ms: u64,
) -> RuntimeResult<&'a GovernedPendingBinding> {
    let binding = pending
        .governed
        .as_ref()
        .ok_or(RuntimeError::Core(TrpgError::DecisionConfirmationRequired))?;
    if now_unix_ms > binding.expires_at_unix_ms {
        return Err(RuntimeError::Core(TrpgError::DecisionExpired));
    }
    if &binding.campaign_id != contract.campaign_id() {
        return Err(RuntimeError::Core(TrpgError::CampaignScopeMismatch));
    }
    if &binding.authority_contract_id != contract.contract_id() {
        return Err(RuntimeError::Core(TrpgError::AuthorityContractMutation));
    }
    if binding.authority_contract_version != contract.version() {
        return Err(RuntimeError::Core(
            TrpgError::AuthorityContractVersionConflict,
        ));
    }
    if &binding.authority_owner != contract.authority_owner() {
        return Err(RuntimeError::Core(TrpgError::AuthorityOwnerMismatch));
    }
    Ok(binding)
}

fn canonical_commit_draft_hash(command: &CommandEnvelope<RuntimeDecision>) -> String {
    let mut hasher = Sha256::new();
    let context = command.authenticated_context();
    let actor_origin = serde_json::to_string(&command.actor.canonical_origin_wire())
        .expect("canonical actor origin is serializable");
    for value in [
        "trpg-confirmed-canonical-commit-draft-v1",
        command.command_id.as_str(),
        command.idempotency_key.as_str(),
        &command.expected_version.to_string(),
        command.actor.id().as_str(),
        actor_role_name(command.actor.role()),
        actor_origin.as_str(),
        authority_mode_name(&command.authority_mode),
        &command.authority_contract_version.to_string(),
        visibility_name(command.visibility.label()),
        command
            .visibility
            .subject_id()
            .map(EntityId::as_str)
            .unwrap_or("not_applicable"),
        provenance_kind_name(&command.fact_provenance.kind),
        command.fact_provenance.reference.as_str(),
        command.fact_provenance.recorded_by.as_str(),
        command.correlation_id.as_str(),
        command.causation_id.as_str(),
        formal_write_path_name(&command.write_path),
        context.resource().campaign_id().as_str(),
        context.resource().resource_type().as_str(),
        context.resource().resource_id().as_str(),
        context.authority().contract_id().as_str(),
        context.authority().authority_owner().as_str(),
        &context.authority().contract_version().to_string(),
        context.trace_id().as_str(),
    ] {
        hasher.update(value.len().to_be_bytes());
        hasher.update(value.as_bytes());
    }
    hash_runtime_decision(&mut hasher, &command.payload);
    format!("sha256:{:x}", hasher.finalize())
}

fn hash_runtime_decision(hasher: &mut Sha256, decision: &RuntimeDecision) {
    for value in [
        decision.decision_id.as_str(),
        decision.decision_summary.as_str(),
        decision.tool_request.requested_by().as_str(),
        decision.tool_request.tool().as_str(),
        if decision.tool_request.is_formal_state_change() {
            "formal"
        } else {
            "draft"
        },
        visibility_name(decision.tool_request.visibility().label()),
        decision.player_visible_explanation.as_str(),
    ] {
        hasher.update(value.len().to_be_bytes());
        hasher.update(value.as_bytes());
    }
    for value in decision
        .linked_records
        .iter()
        .chain(decision.audit_fields.iter())
    {
        hasher.update(value.len().to_be_bytes());
        hasher.update(value.as_bytes());
    }
}

fn formal_write_path_name(path: &FormalWritePath) -> &'static str {
    match path {
        FormalWritePath::WorkflowDecision => "workflow_decision",
        FormalWritePath::RulesDecision => "rules_decision",
        FormalWritePath::ToolDecision => "tool_decision",
        FormalWritePath::DirectAgent => "direct_agent",
        FormalWritePath::DirectBusiness => "direct_business",
    }
}

impl RuntimeAgent {
    fn as_str(self) -> &'static str {
        match self {
            Self::AiKeeperOrchestrator => "ai_keeper_orchestrator",
            Self::KeeperCopilot => "keeper_copilot",
            Self::AtmosphereWriter => "atmosphere_writer",
            Self::MemoryCurator => "memory_curator",
            Self::WorkflowEngine => "workflow_engine",
            Self::HumanKeeper => "human_keeper",
        }
    }
}

fn visibility_name(label: &VisibilityLabel) -> &'static str {
    label.as_str()
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub enum RuntimeEventPayload {
    ToolRequestApproved {
        tool: &'static str,
        grant: ToolGrantDecision,
        seal: RuntimeFormalEventSeal,
    },
    ToolExecutionSucceeded {
        tool: &'static str,
        execution_id: EntityId,
        result_hash: String,
        seal: RuntimeFormalEventSeal,
    },
    DecisionCommitted {
        decision_id: EntityId,
        linked_records: Vec<&'static str>,
        player_visible_explanation: String,
        audit_fields: Vec<&'static str>,
        seal: RuntimeFormalEventSeal,
    },
    PendingDecisionCreated {
        decision_id: EntityId,
        status: PendingDecisionStatus,
    },
    SessionStarted {
        session_id: EntityId,
    },
    WorkflowAdvanced {
        workflow_id: EntityId,
    },
    SagaCompensated {
        saga_id: EntityId,
    },
    ScheduledTaskDue {
        task_id: EntityId,
    },
    RealtimeDeltaPublished {
        delta_id: EntityId,
    },
}

/// Opaque constructor token carried by formal runtime events. External crates
/// may inspect a payload but cannot mint a formal event payload themselves.
#[doc(hidden)]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct RuntimeFormalEventSeal {
    _private: (),
}

impl RuntimeFormalEventSeal {
    fn new() -> Self {
        Self { _private: () }
    }
}

pub fn validate_runtime_command<T>(
    contract: &AuthorityContract,
    command: &CommandEnvelope<T>,
) -> RuntimeResult<()> {
    if command.write_path == FormalWritePath::DirectAgent {
        return Err(RuntimeError::AgentDirectStateWriteForbidden);
    }

    contract
        .validate_command(command)
        .map_err(RuntimeError::from)
}

fn ensure_expected_version<T>(
    store: &EventStore<RuntimeEventPayload>,
    command: &CommandEnvelope<T>,
) -> RuntimeResult<()> {
    let resource = command.authenticated_context().resource();
    let actual = store
        .inner
        .current_stream_version(resource.campaign_id(), resource.resource_id());
    if store.has_canonical_custody() && store.events().is_empty() {
        return Ok(());
    }
    if command.expected_version != actual {
        return Err(RuntimeError::Core(TrpgError::ExpectedVersionConflict {
            expected: command.expected_version,
            actual,
        }));
    }
    Ok(())
}

fn derived_command<T: Clone>(
    command: &CommandEnvelope<T>,
    suffix: &str,
    expected_version: u64,
) -> RuntimeResult<CommandEnvelope<T>> {
    let mut derived = command.clone();
    derived.command_id = EntityId::new(format!("{}_{}", command.command_id.as_str(), suffix))?;
    derived.idempotency_key = format!("{}:{}", command.idempotency_key, suffix);
    derived.expected_version = expected_version;
    Ok(derived)
}

pub(crate) fn append_runtime_event<T: Clone>(
    store: &mut EventStore<RuntimeEventPayload>,
    contract: &AuthorityContract,
    command: &CommandEnvelope<T>,
    event_type: &'static str,
    payload: RuntimeEventPayload,
) -> RuntimeResult<EventEnvelope<RuntimeEventPayload>> {
    let expected_event_type = match &payload {
        RuntimeEventPayload::ToolRequestApproved { .. }
        | RuntimeEventPayload::ToolExecutionSucceeded { .. }
        | RuntimeEventPayload::DecisionCommitted { .. } => {
            return Err(RuntimeError::Core(TrpgError::PolicyDenied));
        }
        RuntimeEventPayload::PendingDecisionCreated {
            status: PendingDecisionStatus::ReadyToCommit | PendingDecisionStatus::Committed,
            ..
        } => {
            return Err(RuntimeError::Core(TrpgError::PolicyDenied));
        }
        RuntimeEventPayload::PendingDecisionCreated { .. } => "PendingDecisionCreated",
        RuntimeEventPayload::SessionStarted { .. } => "SessionStarted",
        RuntimeEventPayload::WorkflowAdvanced { .. } => "WorkflowAdvanced",
        RuntimeEventPayload::SagaCompensated { .. } => "SagaCompensated",
        RuntimeEventPayload::ScheduledTaskDue { .. } => "ScheduledTaskDue",
        RuntimeEventPayload::RealtimeDeltaPublished { .. } => "RealtimeDeltaPublished",
    };
    if event_type != expected_event_type {
        return Err(RuntimeError::Core(TrpgError::EventContractUnknown));
    }
    validate_runtime_command(contract, command)?;
    ensure_expected_version(store, command)?;
    store
        .append(command, event_type, payload)
        .map_err(RuntimeError::from)
}
