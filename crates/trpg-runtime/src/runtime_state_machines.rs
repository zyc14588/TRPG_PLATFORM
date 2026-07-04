use trpg_shared_kernel::{
    AuthorityContract, AuthorityMode, CommandEnvelope, EntityId, EventEnvelope, EventStore,
    FormalWritePath, KernelResult, PrincipalScope, TrpgError, Visibility, VisibilityLabel,
};

pub type RuntimeResult<T> = Result<T, RuntimeError>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuntimeError {
    Core(TrpgError),
    AgentToolNotAllowed,
    HumanKpAiDraftOnly,
    AgentDirectStateWriteForbidden,
}

impl RuntimeError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Core(error) => error.code(),
            Self::AgentToolNotAllowed => "AGENT_TOOL_NOT_ALLOWED",
            Self::HumanKpAiDraftOnly => "HUMAN_KP_AI_DRAFT_ONLY",
            Self::AgentDirectStateWriteForbidden => "AGENT_DIRECT_STATE_WRITE_FORBIDDEN",
        }
    }
}

impl From<TrpgError> for RuntimeError {
    fn from(error: TrpgError) -> Self {
        match error {
            TrpgError::DirectAgentStateWrite => Self::AgentDirectStateWriteForbidden,
            other => Self::Core(other),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeModule {
    CapabilityToolGrant,
    PendingDecision,
    RealtimeRuntimeBinding,
    RuntimeStateMachines,
    SagaTransaction,
    SchedulerService,
    SessionRuntime,
    WorkflowEngine,
    Adr0007InternalWorkflowVsTemporal,
    CapabilityLayerToolGrant,
    RuntimeWorkflowEngine,
    CapabilityLayer,
    RealtimeRoomSync,
    RuntimePendingDecision,
}

pub const BATCH_012_PRIMARY_MODULES: &[RuntimeModule] = &[
    RuntimeModule::CapabilityToolGrant,
    RuntimeModule::PendingDecision,
    RuntimeModule::RealtimeRuntimeBinding,
    RuntimeModule::RuntimeStateMachines,
    RuntimeModule::SagaTransaction,
    RuntimeModule::SchedulerService,
    RuntimeModule::SessionRuntime,
    RuntimeModule::WorkflowEngine,
    RuntimeModule::Adr0007InternalWorkflowVsTemporal,
    RuntimeModule::CapabilityLayerToolGrant,
    RuntimeModule::RuntimeWorkflowEngine,
    RuntimeModule::CapabilityLayer,
    RuntimeModule::RealtimeRoomSync,
    RuntimeModule::RuntimePendingDecision,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeAgent {
    AiKeeperOrchestrator,
    KeeperCopilot,
    AtmosphereWriter,
    MemoryCurator,
    WorkflowEngine,
    HumanKeeper,
}

impl RuntimeAgent {
    pub fn is_ai(self) -> bool {
        !matches!(self, Self::HumanKeeper | Self::WorkflowEngine)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeTool {
    RequestSkillCheck,
    CommitDecision,
    ChangeScene,
    ApplyDamage,
    NarrationOnly,
}

impl RuntimeTool {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RequestSkillCheck => "request_skill_check",
            Self::CommitDecision => "commit_decision",
            Self::ChangeScene => "change_scene",
            Self::ApplyDamage => "apply_damage",
            Self::NarrationOnly => "narration_only",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolRequest {
    pub requested_by: RuntimeAgent,
    pub tool: RuntimeTool,
    pub formal_state_change: bool,
    pub visibility: Visibility,
}

impl ToolRequest {
    pub fn formal(requested_by: RuntimeAgent, tool: RuntimeTool) -> Self {
        Self {
            requested_by,
            tool,
            formal_state_change: true,
            visibility: Visibility::new(VisibilityLabel::Public),
        }
    }

    pub fn draft(requested_by: RuntimeAgent, tool: RuntimeTool) -> Self {
        Self {
            requested_by,
            tool,
            formal_state_change: false,
            visibility: Visibility::new(VisibilityLabel::KeeperOnly),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolGrantDecision {
    pub allowed: bool,
    pub requires_human_confirmation: bool,
    pub draft_only: bool,
    pub error_code: Option<&'static str>,
}

impl ToolGrantDecision {
    pub fn allow() -> Self {
        Self {
            allowed: true,
            requires_human_confirmation: false,
            draft_only: false,
            error_code: None,
        }
    }

    pub fn deny(error: RuntimeError, requires_human_confirmation: bool, draft_only: bool) -> Self {
        Self {
            allowed: false,
            requires_human_confirmation,
            draft_only,
            error_code: Some(error.code()),
        }
    }
}

pub fn evaluate_tool_grant(
    authority_mode: &AuthorityMode,
    request: &ToolRequest,
) -> ToolGrantDecision {
    if request.formal_state_change && request.requested_by == RuntimeAgent::AtmosphereWriter {
        return ToolGrantDecision::deny(RuntimeError::AgentToolNotAllowed, false, false);
    }

    match authority_mode {
        AuthorityMode::HumanKp if request.requested_by.is_ai() => {
            ToolGrantDecision::deny(RuntimeError::HumanKpAiDraftOnly, true, true)
        }
        AuthorityMode::HumanKp => ToolGrantDecision {
            requires_human_confirmation: request.formal_state_change,
            ..ToolGrantDecision::allow()
        },
        AuthorityMode::AiKp
            if request.formal_state_change
                && request.requested_by != RuntimeAgent::AiKeeperOrchestrator =>
        {
            ToolGrantDecision::deny(RuntimeError::AgentToolNotAllowed, false, false)
        }
        AuthorityMode::AiKp => ToolGrantDecision::allow(),
    }
}

pub fn approve_tool_request(
    authority_mode: &AuthorityMode,
    request: &ToolRequest,
) -> RuntimeResult<ToolGrantDecision> {
    let grant = evaluate_tool_grant(authority_mode, request);
    if grant.allowed {
        Ok(grant)
    } else if grant.error_code == Some(RuntimeError::HumanKpAiDraftOnly.code()) {
        Err(RuntimeError::HumanKpAiDraftOnly)
    } else {
        Err(RuntimeError::AgentToolNotAllowed)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeDecision {
    pub decision_id: EntityId,
    pub decision_summary: String,
    pub tool_request: ToolRequest,
    pub linked_records: Vec<&'static str>,
    pub player_visible_explanation: String,
    pub audit_fields: Vec<&'static str>,
}

impl RuntimeDecision {
    pub fn new(
        decision_id: impl Into<String>,
        decision_summary: impl Into<String>,
        tool_request: ToolRequest,
    ) -> KernelResult<Self> {
        Ok(Self {
            decision_id: EntityId::new(decision_id)?,
            decision_summary: decision_summary.into(),
            tool_request,
            linked_records: vec!["DecisionRecord", "DiceRoll", "GameEvent"],
            player_visible_explanation: "Ruling resolved through the runtime decision pipeline."
                .to_owned(),
            audit_fields: vec![
                "agent_pack_version",
                "prompt_version",
                "model_provider",
                "context_hash",
                "tool_calls",
                "decision_summary",
            ],
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PendingDecisionStatus {
    DraftOnly,
    AwaitingHumanConfirmation,
    ReadyToCommit,
    Committed,
    Rejected,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingDecision {
    pub decision: RuntimeDecision,
    pub status: PendingDecisionStatus,
    pub grant: ToolGrantDecision,
}

pub fn create_pending_decision(
    authority_mode: &AuthorityMode,
    decision: RuntimeDecision,
) -> PendingDecision {
    let grant = evaluate_tool_grant(authority_mode, &decision.tool_request);
    let status = if grant.draft_only {
        PendingDecisionStatus::DraftOnly
    } else if grant.requires_human_confirmation {
        PendingDecisionStatus::AwaitingHumanConfirmation
    } else {
        PendingDecisionStatus::ReadyToCommit
    };

    PendingDecision {
        decision,
        status,
        grant,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuntimeEventPayload {
    ToolRequestApproved {
        tool: &'static str,
        grant: ToolGrantDecision,
    },
    DecisionCommitted {
        decision_id: EntityId,
        linked_records: Vec<&'static str>,
        player_visible_explanation: String,
        audit_fields: Vec<&'static str>,
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
    let actual = store.events().len() as u64;
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

pub fn append_runtime_event<T: Clone>(
    store: &mut EventStore<RuntimeEventPayload>,
    contract: &AuthorityContract,
    command: &CommandEnvelope<T>,
    event_type: &'static str,
    payload: RuntimeEventPayload,
) -> RuntimeResult<EventEnvelope<RuntimeEventPayload>> {
    validate_runtime_command(contract, command)?;
    ensure_expected_version(store, command)?;
    store
        .append(command, event_type, payload)
        .map_err(RuntimeError::from)
}

pub fn commit_decision(
    store: &mut EventStore<RuntimeEventPayload>,
    contract: &AuthorityContract,
    command: &CommandEnvelope<RuntimeDecision>,
    decision: RuntimeDecision,
) -> RuntimeResult<Vec<EventEnvelope<RuntimeEventPayload>>> {
    validate_runtime_command(contract, command)?;
    ensure_expected_version(store, command)?;
    let grant = approve_tool_request(&command.authority_mode, &decision.tool_request)?;

    // ponytail: in-memory event store until the data/eventing crate exists.
    let tool_command = derived_command(command, "tool", store.events().len() as u64)?;
    let tool_event = store.append(
        &tool_command,
        "ToolRequestApproved",
        RuntimeEventPayload::ToolRequestApproved {
            tool: decision.tool_request.tool.as_str(),
            grant: grant.clone(),
        },
    )?;

    let decision_command = derived_command(command, "decision", store.events().len() as u64)?;
    let decision_event = store.append(
        &decision_command,
        "DecisionCommitted",
        RuntimeEventPayload::DecisionCommitted {
            decision_id: decision.decision_id,
            linked_records: decision.linked_records,
            player_visible_explanation: decision.player_visible_explanation,
            audit_fields: decision.audit_fields,
        },
    )?;

    Ok(vec![tool_event, decision_event])
}

pub fn replay_visible_runtime_events(
    store: &EventStore<RuntimeEventPayload>,
    principal: &PrincipalScope,
) -> Vec<EventEnvelope<RuntimeEventPayload>> {
    store.replay_visible(principal)
}
