use trpg_contracts::WireErrorCode;
use trpg_identity::{
    AgentClass as IdentityAgentClass, AuthenticationContext, IdentityVerifier, PrincipalKind,
};
use trpg_security_governance::formal_commit_audit::FormalCommitAudit;
use trpg_shared_kernel::{
    AuthorityContract, AuthorityMode, CommandEnvelope, EntityId, EventEnvelope,
    EventStore as KernelEventStore, FormalWritePath, PrincipalScope, TrpgError, Visibility,
    VisibilityLabel,
};

pub type AgentResult<T> = Result<T, AgentError>;

/// Agent event storage exposes replay but keeps append authority inside the
/// authenticated decision committer.
#[derive(Clone, Debug)]
pub struct EventStore<P> {
    inner: KernelEventStore<P>,
    formal_audit: Option<FormalCommitAudit>,
}

impl<P> Default for EventStore<P> {
    fn default() -> Self {
        Self {
            inner: KernelEventStore::default(),
            formal_audit: None,
        }
    }
}

impl<P: Clone> EventStore<P> {
    pub fn with_formal_audit(formal_audit: FormalCommitAudit) -> Self {
        Self {
            inner: KernelEventStore::default(),
            formal_audit: Some(formal_audit),
        }
    }

    pub fn events(&self) -> &[EventEnvelope<P>] {
        self.inner.events()
    }

    pub fn replay_visible(&self, principal: &PrincipalScope) -> Vec<EventEnvelope<P>> {
        self.inner.replay_visible(principal)
    }

    fn append<T>(
        &mut self,
        command: &CommandEnvelope<T>,
        event_type: &'static str,
        payload: P,
    ) -> Result<EventEnvelope<P>, TrpgError> {
        self.inner.append(command, event_type, payload)
    }

    fn formal_audit(&self) -> Result<&FormalCommitAudit, TrpgError> {
        self.formal_audit
            .as_ref()
            .ok_or(TrpgError::AuditIntegrityViolation)
    }
}

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
    pub const fn wire_code(&self) -> WireErrorCode {
        match self {
            Self::Core(error) => error.wire_code(),
            Self::ToolPermissionDenied => WireErrorCode::ToolPermissionDenied,
            Self::HumanKpDraftOnly => WireErrorCode::HumanKpAiDraftOnly,
            Self::AgentDirectStateWriteForbidden => WireErrorCode::AgentDirectStateWriteForbidden,
            Self::DirectLlmCallForbidden => WireErrorCode::DirectLlmCallForbidden,
            Self::PromptInjectionDetected => WireErrorCode::PromptInjectionDetected,
            Self::LocalModelNotCertifiedForAiKp => WireErrorCode::LocalModelNotCertifiedForAiKp,
            Self::SilentFallbackForbidden => WireErrorCode::SilentFallbackForbidden,
            Self::UnauthenticatedLocalProviderExposed => {
                WireErrorCode::UnauthenticatedLocalProviderExposed
            }
            Self::RagVisibilityScopeViolation => WireErrorCode::RagVisibilityScopeViolation,
        }
    }

    pub fn code(&self) -> &'static str {
        self.wire_code().as_str()
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
    requested_by: AgentKind,
    tool: AgentTool,
    visibility: Visibility,
}

impl ToolRequest {
    pub fn formal(requested_by: AgentKind, tool: AgentTool) -> Self {
        Self {
            requested_by,
            tool,
            visibility: Visibility::new(VisibilityLabel::Public),
        }
    }

    pub fn draft(requested_by: AgentKind, tool: AgentTool) -> Self {
        Self {
            requested_by,
            tool: match tool {
                AgentTool::ApplySanLoss => AgentTool::DraftSanLoss,
                AgentTool::DraftSanLoss | AgentTool::NarrationOnly => tool,
                AgentTool::RequestSkillCheck | AgentTool::RevealClue | AgentTool::ChangeScene => {
                    AgentTool::NarrationOnly
                }
            },
            visibility: Visibility::new(VisibilityLabel::KeeperOnly),
        }
    }

    pub const fn requested_by(&self) -> AgentKind {
        self.requested_by
    }

    pub const fn tool(&self) -> AgentTool {
        self.tool
    }

    pub fn visibility(&self) -> &Visibility {
        &self.visibility
    }

    pub fn is_formal_state_change(&self) -> bool {
        self.tool.is_adjudication()
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
    if request.is_formal_state_change()
        && (request.requested_by().is_expression_only()
            || request.requested_by().is_non_adjudicating())
    {
        return ToolDecision::deny(AgentError::ToolPermissionDenied);
    }

    match authority_mode {
        AuthorityMode::HumanKp
            if request.requested_by().is_ai() && request.is_formal_state_change() =>
        {
            ToolDecision {
                tool_executed: false,
                downgraded_to: Some(match request.tool() {
                    AgentTool::ApplySanLoss => AgentTool::DraftSanLoss,
                    _ => AgentTool::NarrationOnly,
                }),
                requires_human_confirmation: true,
                draft_only: true,
                error: Some(AgentError::HumanKpDraftOnly.code()),
            }
        }
        AuthorityMode::HumanKp => ToolDecision {
            requires_human_confirmation: request.is_formal_state_change(),
            ..ToolDecision::allow()
        },
        AuthorityMode::AiKp
            if request.is_formal_state_change()
                && !request.requested_by().may_request_formal_tool() =>
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
    authentication: AuthenticationContext,
}

impl AgentDecision {
    pub fn new(
        decision_id: impl Into<String>,
        tool_request: ToolRequest,
        player_visible_text: impl Into<String>,
        authentication: &AuthenticationContext,
    ) -> AgentResult<Self> {
        validate_requester_identity(&tool_request, authentication)?;
        let player_visible_text = player_visible_text.into();
        Ok(Self {
            decision_id: EntityId::new(decision_id).map_err(AgentError::from)?,
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
            authentication: authentication.clone(),
        })
    }
}

fn validate_requester_identity(
    request: &ToolRequest,
    authentication: &AuthenticationContext,
) -> AgentResult<()> {
    let PrincipalKind::AgentRun { class, .. } = authentication.kind() else {
        return Err(AgentError::Core(TrpgError::InternalIdentityInvalid));
    };
    let expected = match class {
        IdentityAgentClass::AiKeeperOrchestrator => AgentKind::AiKeeperOrchestrator,
        IdentityAgentClass::KeeperCopilot => AgentKind::KeeperCopilot,
        IdentityAgentClass::AtmosphereWriter => AgentKind::AtmosphereWriter,
        IdentityAgentClass::MemoryCurator => AgentKind::MemoryCurator,
    };
    if request.requested_by() != expected {
        return Err(AgentError::Core(TrpgError::InternalIdentityInvalid));
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AgentEventPayload {
    ToolRequestApproved {
        tool: &'static str,
        decision: ToolDecision,
        seal: AgentFormalEventSeal,
    },
    DecisionCommitted {
        decision_id: EntityId,
        player_visible_text: String,
        linked_records: Vec<&'static str>,
        audit_fields: Vec<&'static str>,
        seal: AgentFormalEventSeal,
    },
    DraftDecisionCreated {
        downgraded_to: &'static str,
    },
    AgentContextAssembled {
        visible_fact_count: usize,
    },
}

/// Opaque constructor token carried by formal agent events.
#[doc(hidden)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentFormalEventSeal {
    _private: (),
}

impl AgentFormalEventSeal {
    fn new() -> Self {
        Self { _private: () }
    }
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

#[derive(Clone, Debug)]
pub struct AgentDecisionCommitter {
    identity_verifier: IdentityVerifier,
}

impl AgentDecisionCommitter {
    pub fn new(identity_verifier: IdentityVerifier) -> AgentResult<Self> {
        Ok(Self { identity_verifier })
    }

    pub fn commit(
        &self,
        store: &mut EventStore<AgentEventPayload>,
        command: &CommandEnvelope<AgentDecision>,
        workflow_authentication: &AuthenticationContext,
        decision: AgentDecision,
        now_unix_ms: u64,
    ) -> AgentResult<Vec<EventEnvelope<AgentEventPayload>>> {
        let contract = self
            .identity_verifier
            .authority_contract(command.authenticated_context().resource().campaign_id())
            .map_err(|_| AgentError::Core(TrpgError::AuthorityViolation))?;
        contract
            .validate_command(command)
            .map_err(AgentError::from)?;
        self.identity_verifier
            .verify_actor(
                workflow_authentication,
                &command.actor,
                command.authenticated_context().resource().campaign_id(),
                now_unix_ms,
            )
            .map_err(|_| AgentError::Core(TrpgError::InternalIdentityInvalid))?;
        if command.write_path == FormalWritePath::DirectAgent {
            return Err(AgentError::AgentDirectStateWriteForbidden);
        }
        self.identity_verifier
            .verify(&decision.authentication, now_unix_ms)
            .map_err(|_| AgentError::Core(TrpgError::InternalIdentityInvalid))?;
        if command.payload != decision {
            return Err(AgentError::Core(TrpgError::DecisionDraftChanged));
        }
        decision
            .authentication
            .require_campaign(contract.campaign_id())
            .map_err(|error| match error {
                trpg_identity::IdentityError::CampaignScopeMismatch => {
                    AgentError::Core(TrpgError::CampaignScopeMismatch)
                }
                _ => AgentError::Core(TrpgError::InternalIdentityInvalid),
            })?;
        validate_requester_identity(&decision.tool_request, &decision.authentication)?;

        if !decision.tool_request.is_formal_state_change() {
            let draft_command = derived_command(command, "draft", store.events().len() as u64)?;
            return Ok(vec![store.append(
                &draft_command,
                "DraftDecisionCreated",
                AgentEventPayload::DraftDecisionCreated {
                    downgraded_to: decision.tool_request.tool().as_str(),
                },
            )?]);
        }

        if contract.mode() == &AuthorityMode::AiKp
            && decision.authentication.subject_id() != contract.authority_owner()
        {
            return Err(AgentError::Core(TrpgError::AuthorityOwnerMismatch));
        }

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

        // Identity, authority, and tool checks must complete before the event-store capability is used.
        let next_version = store.events().len() as u64;
        let tool_command = derived_command(command, "tool", next_version)?;
        let decision_command = derived_command(command, "decision", next_version + 1)?;
        if store.events().iter().any(|event| {
            event.idempotency_key == tool_command.idempotency_key
                || event.idempotency_key == decision_command.idempotency_key
        }) {
            return Err(AgentError::Core(TrpgError::DuplicateCommand));
        }
        let requested_role = match decision.authentication.kind() {
            PrincipalKind::AgentRun { class, .. } => match class {
                IdentityAgentClass::AiKeeperOrchestrator => "ai_keeper_orchestrator",
                IdentityAgentClass::KeeperCopilot => "keeper_copilot",
                IdentityAgentClass::AtmosphereWriter => "atmosphere_writer",
                IdentityAgentClass::MemoryCurator => "memory_curator",
            },
            _ => return Err(AgentError::Core(TrpgError::InternalIdentityInvalid)),
        };
        store.formal_audit()?.record_authorized_commit(
            &decision.authentication,
            command,
            &contract,
            "authorize_agent_formal_commit",
            requested_role,
        )?;
        let tool_event = store.append(
            &tool_command,
            "ToolRequestApproved",
            AgentEventPayload::ToolRequestApproved {
                tool: decision.tool_request.tool().as_str(),
                decision: tool_decision,
                seal: AgentFormalEventSeal::new(),
            },
        )?;

        let decision_event = store.append(
            &decision_command,
            "DecisionCommitted",
            AgentEventPayload::DecisionCommitted {
                decision_id: decision.decision_id,
                player_visible_text: redact_player_visible_text(&decision.player_visible_text),
                linked_records: decision.linked_records,
                audit_fields: decision.audit_fields,
                seal: AgentFormalEventSeal::new(),
            },
        )?;

        Ok(vec![tool_event, decision_event])
    }
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
