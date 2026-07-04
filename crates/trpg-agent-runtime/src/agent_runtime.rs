use trpg_shared_kernel::{
    AuthorityContract, AuthorityMode, CommandEnvelope, EntityId, EventEnvelope, EventStore,
    FormalWritePath, PrincipalScope, TrpgError, Visibility, VisibilityLabel,
};

pub type AgentResult<T> = Result<T, AgentError>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AgentError {
    Core(TrpgError),
    ToolPermissionDenied,
    HumanKpDraftOnly,
    AgentDirectStateWriteForbidden,
    DirectLlmCallForbidden,
    PromptInjectionDetected,
    LocalModelNotCertifiedForAiKp,
    SilentFallbackForbidden,
    UnauthenticatedLocalProviderExposed,
    RagVisibilityScopeViolation,
}

impl AgentError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Core(error) => error.code(),
            Self::ToolPermissionDenied => "ToolPermissionDenied",
            Self::HumanKpDraftOnly => "HUMAN_KP_AI_DRAFT_ONLY",
            Self::AgentDirectStateWriteForbidden => "AGENT_DIRECT_STATE_WRITE_FORBIDDEN",
            Self::DirectLlmCallForbidden => "DIRECT_LLM_CALL_FORBIDDEN",
            Self::PromptInjectionDetected => "PROMPT_INJECTION_DETECTED",
            Self::LocalModelNotCertifiedForAiKp => "LOCAL_MODEL_NOT_CERTIFIED_FOR_AI_KP",
            Self::SilentFallbackForbidden => "SILENT_FALLBACK_FORBIDDEN",
            Self::UnauthenticatedLocalProviderExposed => "UNAUTHENTICATED_LOCAL_PROVIDER_EXPOSED",
            Self::RagVisibilityScopeViolation => "RAG_VISIBILITY_SCOPE_VIOLATION",
        }
    }
}

impl From<TrpgError> for AgentError {
    fn from(error: TrpgError) -> Self {
        match error {
            TrpgError::DirectAgentStateWrite => Self::AgentDirectStateWriteForbidden,
            other => Self::Core(other),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentModule {
    AgentContextAssembler,
    AgentRuntime,
    AiEvaluationRuntime,
    LocalModelCertification,
    MemoryRagRagSnapshot,
    ModelProvider,
    ToolProtocol,
    Adr0009AgentGovernanceAgentGovernance,
    AgentRuntimeToolProtocol,
    AgentEvaluationGoldenScenario,
    WorkingMemoryLongMemoryRag,
    RagSnapshot,
    ModelProviderLocalCloud,
    AiEvaluationGoldenScenario,
    WorkingMemoryRagRagSnapshot,
    MemoryRag,
    MemoryRagImpl,
    ModelProviderLocalCloudImpl,
    RagSnapshotImpl,
    Adr0009AgentGovernance,
}

pub const BATCH_017_PRIMARY_MODULES: &[AgentModule] = &[
    AgentModule::AgentContextAssembler,
    AgentModule::AgentRuntime,
    AgentModule::AiEvaluationRuntime,
    AgentModule::LocalModelCertification,
    AgentModule::MemoryRagRagSnapshot,
    AgentModule::ModelProvider,
    AgentModule::ToolProtocol,
    AgentModule::Adr0009AgentGovernanceAgentGovernance,
    AgentModule::AgentRuntimeToolProtocol,
    AgentModule::AgentEvaluationGoldenScenario,
    AgentModule::WorkingMemoryLongMemoryRag,
    AgentModule::RagSnapshot,
    AgentModule::ModelProviderLocalCloud,
    AgentModule::AiEvaluationGoldenScenario,
    AgentModule::WorkingMemoryRagRagSnapshot,
    AgentModule::MemoryRag,
];

pub const BATCH_017_PROMPT_IDS: &[&str] = &[
    "CODEX-0040-04-AI-AGENT-SYSTEM-0ed30fc5f0",
    "CODEX-0041-04-AI-AGENT-SYSTEM-570f17da9d",
    "CODEX-0042-04-AI-AGENT-SYSTEM-bbc851a5de",
    "CODEX-0043-04-AI-AGENT-SYSTEM-8bea44b53b",
    "CODEX-0044-04-AI-AGENT-SYSTEM-4a4aa2a8df",
    "CODEX-0045-04-AI-AGENT-SYSTEM-e852321d0b",
    "CODEX-0046-04-AI-AGENT-SYSTEM-6468c9be5b",
    "CODEX-0047-04-AI-AGENT-SYSTEM-524e2550a8",
    "CODEX-0441-04-AI-AGENT-SYSTEM-b81eba8b66",
    "CODEX-0442-04-AI-AGENT-SYSTEM-34a7e5c6f0",
    "CODEX-0443-04-AI-AGENT-SYSTEM-bcbd7b78de",
    "CODEX-0444-04-AI-AGENT-SYSTEM-20434579ca",
    "CODEX-0445-04-AI-AGENT-SYSTEM-43507a6209",
    "CODEX-0446-04-AI-AGENT-SYSTEM-bafcf3dfc6",
    "CODEX-0447-04-AI-AGENT-SYSTEM-3497400719",
    "CODEX-0448-04-AI-AGENT-SYSTEM-41ecd49e88",
    "CODEX-0449-04-AI-AGENT-SYSTEM-b319601824",
    "CODEX-0450-04-AI-AGENT-SYSTEM-3e566913fa",
    "CODEX-0451-04-AI-AGENT-SYSTEM-dab850ee74",
    "CODEX-0452-04-AI-AGENT-SYSTEM-4f2dab7f75",
    "CODEX-0453-04-AI-AGENT-SYSTEM-159b37a04c",
    "CODEX-0454-04-AI-AGENT-SYSTEM-014bc53177",
    "CODEX-0455-04-AI-AGENT-SYSTEM-a49d9b14ee",
    "CODEX-0456-04-AI-AGENT-SYSTEM-d68068a022",
    "CODEX-0457-04-AI-AGENT-SYSTEM-487c497469",
];

pub const BATCH_019_PRIMARY_MODULES: &[AgentModule] = &[
    AgentModule::MemoryRagImpl,
    AgentModule::ModelProviderLocalCloudImpl,
    AgentModule::RagSnapshotImpl,
    AgentModule::Adr0009AgentGovernance,
];

pub const BATCH_019_PROMPT_IDS: &[&str] = &[
    "CODEX-0483-04-AI-AGENT-SYSTEM-a577767984",
    "CODEX-0484-04-AI-AGENT-SYSTEM-e96dc3868d",
    "CODEX-0485-04-AI-AGENT-SYSTEM-962b774429",
    "CODEX-0486-04-AI-AGENT-SYSTEM-9ce89f19f8",
    "CODEX-0487-04-AI-AGENT-SYSTEM-dbe6de7e59",
    "CODEX-0488-04-AI-AGENT-SYSTEM-03fc2209c6",
    "CODEX-0489-04-AI-AGENT-SYSTEM-752b9c9430",
    "CODEX-0490-04-AI-AGENT-SYSTEM-475b10a2a4",
    "CODEX-0491-04-AI-AGENT-SYSTEM-a7c5faa922",
    "CODEX-0492-04-AI-AGENT-SYSTEM-f219f76442",
    "CODEX-0493-04-AI-AGENT-SYSTEM-eb040218e6",
    "CODEX-0494-04-AI-AGENT-SYSTEM-e007c89f57",
    "CODEX-0495-04-AI-AGENT-SYSTEM-799fc14dc2",
    "CODEX-0496-04-AI-AGENT-SYSTEM-c0f67c85c7",
    "CODEX-0497-04-AI-AGENT-SYSTEM-044ab5dc87",
    "CODEX-0498-04-AI-AGENT-SYSTEM-13927ff7ed",
    "CODEX-0499-04-AI-AGENT-SYSTEM-04b8aaf7da",
    "CODEX-0500-04-AI-AGENT-SYSTEM-9f239edf80",
    "CODEX-0501-04-AI-AGENT-SYSTEM-687782b527",
    "CODEX-0502-04-AI-AGENT-SYSTEM-1cc19ac6d6",
    "CODEX-0503-04-AI-AGENT-SYSTEM-0e7645f3a5",
    "CODEX-0504-04-AI-AGENT-SYSTEM-2d75990472",
    "CODEX-0505-04-AI-AGENT-SYSTEM-9f37999d40",
    "CODEX-0506-04-AI-AGENT-SYSTEM-e75d4617db",
    "CODEX-0507-04-AI-AGENT-SYSTEM-a1e5d3d499",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentKind {
    AiKeeperOrchestrator,
    KeeperCopilot,
    AtmosphereWriter,
    MemoryCurator,
    SummaryAgent,
    ExportAgent,
    SafetyModerator,
}

impl AgentKind {
    pub fn is_ai(self) -> bool {
        true
    }

    fn may_request_formal_tool(self) -> bool {
        matches!(self, Self::AiKeeperOrchestrator)
    }

    fn is_expression_only(self) -> bool {
        matches!(self, Self::AtmosphereWriter)
    }

    fn is_non_adjudicating(self) -> bool {
        matches!(
            self,
            Self::MemoryCurator | Self::SummaryAgent | Self::ExportAgent | Self::SafetyModerator
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentTool {
    RequestSkillCheck,
    RevealClue,
    ApplySanLoss,
    ChangeScene,
    DraftSanLoss,
    NarrationOnly,
}

impl AgentTool {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RequestSkillCheck => "request_skill_check",
            Self::RevealClue => "reveal_clue",
            Self::ApplySanLoss => "apply_san_loss",
            Self::ChangeScene => "change_scene",
            Self::DraftSanLoss => "draft_san_loss",
            Self::NarrationOnly => "narration_only",
        }
    }

    fn is_adjudication(self) -> bool {
        !matches!(self, Self::NarrationOnly | Self::DraftSanLoss)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolRequest {
    pub requested_by: AgentKind,
    pub tool: AgentTool,
    pub formal_state_change: bool,
    pub visibility: Visibility,
}

impl ToolRequest {
    pub fn formal(requested_by: AgentKind, tool: AgentTool) -> Self {
        Self {
            requested_by,
            tool,
            formal_state_change: true,
            visibility: Visibility::new(VisibilityLabel::Public),
        }
    }

    pub fn draft(requested_by: AgentKind, tool: AgentTool) -> Self {
        Self {
            requested_by,
            tool,
            formal_state_change: false,
            visibility: Visibility::new(VisibilityLabel::KeeperOnly),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolDecision {
    pub tool_executed: bool,
    pub downgraded_to: Option<AgentTool>,
    pub requires_human_confirmation: bool,
    pub draft_only: bool,
    pub error: Option<&'static str>,
}

impl ToolDecision {
    fn allow() -> Self {
        Self {
            tool_executed: true,
            downgraded_to: None,
            requires_human_confirmation: false,
            draft_only: false,
            error: None,
        }
    }

    fn deny(error: AgentError) -> Self {
        Self {
            tool_executed: false,
            downgraded_to: None,
            requires_human_confirmation: false,
            draft_only: false,
            error: Some(error.code()),
        }
    }
}

pub fn evaluate_agent_tool_request(
    authority_mode: &AuthorityMode,
    request: &ToolRequest,
) -> ToolDecision {
    if request.formal_state_change
        && request.tool.is_adjudication()
        && (request.requested_by.is_expression_only() || request.requested_by.is_non_adjudicating())
    {
        return ToolDecision::deny(AgentError::ToolPermissionDenied);
    }

    match authority_mode {
        AuthorityMode::HumanKp if request.requested_by.is_ai() && request.formal_state_change => {
            ToolDecision {
                tool_executed: false,
                downgraded_to: Some(match request.tool {
                    AgentTool::ApplySanLoss => AgentTool::DraftSanLoss,
                    _ => AgentTool::NarrationOnly,
                }),
                requires_human_confirmation: true,
                draft_only: true,
                error: Some(AgentError::HumanKpDraftOnly.code()),
            }
        }
        AuthorityMode::HumanKp => ToolDecision {
            requires_human_confirmation: request.formal_state_change,
            ..ToolDecision::allow()
        },
        AuthorityMode::AiKp
            if request.formal_state_change && !request.requested_by.may_request_formal_tool() =>
        {
            ToolDecision::deny(AgentError::ToolPermissionDenied)
        }
        AuthorityMode::AiKp => ToolDecision::allow(),
    }
}

const PLAYER_VISIBLE_RESTRICTED_TOKENS: &[&str] = &[
    "keeper_truth",
    "secret_operator",
    "npc_true_identity",
    "keeper_only",
    "private_to_player",
    "ai_internal",
    "KeeperOnly",
    "PrivateToPlayer",
    "AiInternal",
];

pub fn redact_player_visible_text(text: &str) -> String {
    PLAYER_VISIBLE_RESTRICTED_TOKENS
        .iter()
        .fold(text.to_owned(), |redacted, token| {
            redacted.replace(token, "[redacted]")
        })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentDecision {
    pub decision_id: EntityId,
    pub tool_request: ToolRequest,
    pub player_visible_text: String,
    pub keeper_notes: Vec<String>,
    pub linked_records: Vec<&'static str>,
    pub audit_fields: Vec<&'static str>,
}

impl AgentDecision {
    pub fn new(
        decision_id: impl Into<String>,
        tool_request: ToolRequest,
        player_visible_text: impl Into<String>,
    ) -> Result<Self, TrpgError> {
        let player_visible_text = player_visible_text.into();
        Ok(Self {
            decision_id: EntityId::new(decision_id)?,
            tool_request,
            player_visible_text: redact_player_visible_text(&player_visible_text),
            keeper_notes: Vec::new(),
            linked_records: vec!["DecisionRecord", "GameEvent", "ToolResult"],
            audit_fields: vec![
                "agent_pack_version",
                "prompt_version",
                "model_provider",
                "context_hash",
                "tool_calls",
                "visibility_labels",
            ],
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AgentEventPayload {
    ToolRequestApproved {
        tool: &'static str,
        decision: ToolDecision,
    },
    DecisionCommitted {
        decision_id: EntityId,
        player_visible_text: String,
        linked_records: Vec<&'static str>,
        audit_fields: Vec<&'static str>,
    },
    DraftDecisionCreated {
        downgraded_to: &'static str,
    },
    AgentContextAssembled {
        visible_fact_count: usize,
    },
}

pub fn validate_agent_command<T>(
    contract: &AuthorityContract,
    command: &CommandEnvelope<T>,
) -> AgentResult<()> {
    if command.write_path == FormalWritePath::DirectAgent {
        return Err(AgentError::AgentDirectStateWriteForbidden);
    }
    contract.validate_command(command).map_err(AgentError::from)
}

fn derived_command<T: Clone>(
    command: &CommandEnvelope<T>,
    suffix: &str,
    expected_version: u64,
) -> AgentResult<CommandEnvelope<T>> {
    let mut derived = command.clone();
    derived.command_id = EntityId::new(format!("{}_{}", command.command_id.as_str(), suffix))?;
    derived.idempotency_key = format!("{}:{}", command.idempotency_key, suffix);
    derived.expected_version = expected_version;
    Ok(derived)
}

pub fn commit_agent_decision(
    store: &mut EventStore<AgentEventPayload>,
    contract: &AuthorityContract,
    command: &CommandEnvelope<AgentDecision>,
    decision: AgentDecision,
) -> AgentResult<Vec<EventEnvelope<AgentEventPayload>>> {
    validate_agent_command(contract, command)?;

    let tool_decision =
        evaluate_agent_tool_request(&command.authority_mode, &decision.tool_request);
    if tool_decision.draft_only {
        let draft_command = derived_command(command, "draft", store.events().len() as u64)?;
        return Ok(vec![store.append(
            &draft_command,
            "DraftDecisionCreated",
            AgentEventPayload::DraftDecisionCreated {
                downgraded_to: tool_decision
                    .downgraded_to
                    .unwrap_or(AgentTool::NarrationOnly)
                    .as_str(),
            },
        )?]);
    }
    if let Some(error) = tool_decision.error {
        return Err(if error == AgentError::ToolPermissionDenied.code() {
            AgentError::ToolPermissionDenied
        } else {
            AgentError::HumanKpDraftOnly
        });
    }

    // ponytail: in-memory EventStore until data/eventing lands for this crate.
    let tool_command = derived_command(command, "tool", store.events().len() as u64)?;
    let tool_event = store.append(
        &tool_command,
        "ToolRequestApproved",
        AgentEventPayload::ToolRequestApproved {
            tool: decision.tool_request.tool.as_str(),
            decision: tool_decision,
        },
    )?;

    let decision_command = derived_command(command, "decision", store.events().len() as u64)?;
    let decision_event = store.append(
        &decision_command,
        "DecisionCommitted",
        AgentEventPayload::DecisionCommitted {
            decision_id: decision.decision_id,
            player_visible_text: redact_player_visible_text(&decision.player_visible_text),
            linked_records: decision.linked_records,
            audit_fields: decision.audit_fields,
        },
    )?;

    Ok(vec![tool_event, decision_event])
}

pub fn replay_agent_events_for_principal(
    store: &EventStore<AgentEventPayload>,
    principal: &PrincipalScope,
) -> Vec<EventEnvelope<AgentEventPayload>> {
    store.replay_visible(principal)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContextFact {
    pub fact_id: EntityId,
    pub text: String,
    pub visibility: Visibility,
}

impl ContextFact {
    pub fn new(
        fact_id: impl Into<String>,
        text: impl Into<String>,
        visibility: Visibility,
    ) -> Result<Self, TrpgError> {
        Ok(Self {
            fact_id: EntityId::new(fact_id)?,
            text: text.into(),
            visibility,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssembledAgentContext {
    pub facts: Vec<ContextFact>,
    pub strictest_visibility: VisibilityLabel,
}

pub fn assemble_context(
    facts: &[ContextFact],
    principal: &PrincipalScope,
) -> AssembledAgentContext {
    let visible: Vec<ContextFact> = facts
        .iter()
        .filter(|fact| fact.visibility.can_view(principal))
        .cloned()
        .collect();
    let strictest_visibility = if visible
        .iter()
        .any(|fact| fact.visibility.label() == &VisibilityLabel::KeeperOnly)
    {
        VisibilityLabel::KeeperOnly
    } else if visible
        .iter()
        .any(|fact| fact.visibility.label() == &VisibilityLabel::PrivateToPlayer)
    {
        VisibilityLabel::PrivateToPlayer
    } else {
        VisibilityLabel::Public
    };

    AssembledAgentContext {
        facts: visible,
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
