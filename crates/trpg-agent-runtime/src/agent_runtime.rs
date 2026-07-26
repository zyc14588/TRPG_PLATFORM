use std::sync::Arc;

use trpg_contracts::WireErrorCode;
use trpg_identity::{
    AgentClass as IdentityAgentClass, AuthenticationContext, IdentityVerifier, PrincipalKind,
    ReplayAuthorization,
};
use trpg_security_governance::formal_commit_audit::{FormalAuthorization, FormalCommitAuthorizer};
use trpg_security_governance::{
    evaluate_derived_visibility, DerivationRequest, DerivedObject as SecurityDerivedObject,
    RedactionOutcome as SecurityRedactionOutcome,
};
use trpg_shared_kernel::{
    AuthorityContract, AuthorityMode, CanonicalCommitEvent, CanonicalCommitKey,
    CanonicalCommitPort, CanonicalCommitReceipt, CanonicalCommitRequest, CommandEnvelope, EntityId,
    EventEnvelope, EventStore as KernelEventStore, FactProvenance, FormalWritePath, PrincipalScope,
    ProvenanceKind, TrpgError, Visibility, VisibilityLabel,
};

pub type AgentResult<T> = Result<T, AgentError>;

/// Agent event storage exposes replay but keeps append authority inside the
/// authenticated decision committer.
#[derive(Clone, Debug)]
pub struct EventStore<P> {
    inner: KernelEventStore<P>,
    formal_custody: Option<FormalCommitCustody>,
}

#[derive(Clone, Debug)]
struct FormalCommitCustody {
    authorizer: FormalCommitAuthorizer,
    canonical: Arc<dyn CanonicalCommitPort>,
}

impl<P> Default for EventStore<P> {
    fn default() -> Self {
        Self {
            inner: KernelEventStore::default(),
            formal_custody: None,
        }
    }
}

impl<P: Clone + PartialEq + serde::Serialize> EventStore<P> {
    pub fn with_formal_custody(
        formal_authorizer: FormalCommitAuthorizer,
        canonical: Arc<dyn CanonicalCommitPort>,
    ) -> Self {
        Self {
            inner: KernelEventStore::default(),
            formal_custody: Some(FormalCommitCustody {
                authorizer: formal_authorizer,
                canonical,
            }),
        }
    }

    pub const fn has_canonical_custody(&self) -> bool {
        self.formal_custody.is_some()
    }

    pub fn events(&self) -> &[EventEnvelope<P>] {
        self.inner.events()
    }

    pub fn replay_visible(
        &self,
        authorization: &ReplayAuthorization,
        now_unix_ms: u64,
    ) -> AgentResult<Vec<EventEnvelope<P>>> {
        let mut visible = Vec::new();
        for event in self.inner.events() {
            let allowed = authorization
                .can_view(&event.campaign_id, &event.visibility, now_unix_ms)
                .map_err(|error| match error {
                    trpg_identity::IdentityError::MembershipRequired
                    | trpg_identity::IdentityError::MembershipDenied
                    | trpg_identity::IdentityError::CampaignScopeMismatch => {
                        AgentError::Core(TrpgError::AuthorizationDenied)
                    }
                    _ => AgentError::Core(TrpgError::AuthenticationRequired),
                })?;
            if allowed {
                visible.push(event.clone());
            }
        }
        Ok(visible)
    }

    fn append<T>(
        &mut self,
        command: &CommandEnvelope<T>,
        event_type: &'static str,
        payload: P,
    ) -> Result<EventEnvelope<P>, TrpgError> {
        self.inner.append(command, event_type, payload)
    }

    fn formal_custody(&self) -> Result<&FormalCommitCustody, TrpgError> {
        self.formal_custody
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
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

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct ToolDecision {
    pub tool_authorized: bool,
    pub tool_executed: bool,
    pub downgraded_to: Option<AgentTool>,
    pub requires_human_confirmation: bool,
    pub draft_only: bool,
    pub error: Option<&'static str>,
}

impl ToolDecision {
    fn allow() -> Self {
        Self {
            tool_authorized: true,
            tool_executed: false,
            downgraded_to: None,
            requires_human_confirmation: false,
            draft_only: false,
            error: None,
        }
    }

    fn deny(error: AgentError) -> Self {
        Self {
            tool_authorized: false,
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
                tool_authorized: false,
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

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub enum AgentEventPayload {
    ToolRequestApproved {
        tool: &'static str,
        decision: ToolDecision,
        seal: AgentFormalEventSeal,
    },
    ToolExecutionSucceeded {
        tool: &'static str,
        execution_id: EntityId,
        result_hash: String,
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
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentToolExecutionOutput {
    pub execution_id: String,
    pub result_hash: String,
}

pub trait AgentToolExecutor: Send + Sync {
    fn execute(&self, decision: &AgentDecision) -> AgentResult<AgentToolExecutionOutput>;
}

#[derive(Debug)]
struct RejectingAgentToolExecutor;

impl AgentToolExecutor for RejectingAgentToolExecutor {
    fn execute(&self, _decision: &AgentDecision) -> AgentResult<AgentToolExecutionOutput> {
        Err(AgentError::ToolPermissionDenied)
    }
}

#[derive(Clone)]
pub struct AgentDecisionCommitter {
    identity_verifier: IdentityVerifier,
    tool_executor: Arc<dyn AgentToolExecutor>,
}

impl std::fmt::Debug for AgentDecisionCommitter {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AgentDecisionCommitter")
            .field("identity_verifier", &self.identity_verifier)
            .field("tool_executor", &"[TRUSTED TOOL EXECUTOR]")
            .finish()
    }
}

impl AgentDecisionCommitter {
    pub fn new(identity_verifier: IdentityVerifier) -> AgentResult<Self> {
        Ok(Self {
            identity_verifier,
            tool_executor: Arc::new(RejectingAgentToolExecutor),
        })
    }

    pub fn with_tool_executor(
        identity_verifier: IdentityVerifier,
        tool_executor: Arc<dyn AgentToolExecutor>,
    ) -> AgentResult<Self> {
        Ok(Self {
            identity_verifier,
            tool_executor,
        })
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
            let resource = command.authenticated_context().resource();
            let draft_version = store
                .inner
                .current_stream_version(resource.campaign_id(), resource.resource_id());
            let draft_command = derived_command(command, "draft", draft_version)?;
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
            let resource = command.authenticated_context().resource();
            let draft_version = store
                .inner
                .current_stream_version(resource.campaign_id(), resource.resource_id());
            let draft_command = derived_command(command, "draft", draft_version)?;
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

        // Preserve the original derived request hashes so an exact network
        // retry resolves through EventStore's scoped idempotency index before
        // optimistic concurrency is evaluated.
        let next_version = command.expected_version;
        let tool_command = derived_command(command, "tool", next_version)?;
        let execution_command = derived_command(command, "execution", next_version + 1)?;
        let decision_command = derived_command(command, "decision", next_version + 2)?;
        let requested_role = match decision.authentication.kind() {
            PrincipalKind::AgentRun { class, .. } => match class {
                IdentityAgentClass::AiKeeperOrchestrator => "ai_keeper_orchestrator",
                IdentityAgentClass::KeeperCopilot => "keeper_copilot",
                IdentityAgentClass::AtmosphereWriter => "atmosphere_writer",
                IdentityAgentClass::MemoryCurator => "memory_curator",
            },
            _ => return Err(AgentError::Core(TrpgError::InternalIdentityInvalid)),
        };
        let (authorization, canonical) = {
            let custody = store.formal_custody()?;
            (
                custody.authorizer.authorize(
                    workflow_authentication,
                    Some(&decision.authentication),
                    command,
                    requested_role,
                    now_unix_ms,
                )?,
                Arc::clone(&custody.canonical),
            )
        };
        // Formal OpenFGA/OPA authorization is deliberately complete before
        // any tool side effect. A durable receipt lookup also precedes tool
        // execution so an exact or cold retry reuses the canonical result.
        let commit_key = CanonicalCommitKey {
            commit_id: format!(
                "{}_{}",
                contract.campaign_id().as_str(),
                command.command_id.as_str()
            ),
            campaign_id: contract.campaign_id().to_string(),
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
                agent_execution_from_receipt(&receipt, decision.tool_request.tool().as_str())?
            }
            None => self.tool_executor.execute(&decision)?,
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
            return Err(AgentError::Core(TrpgError::InvalidConfiguration(
                "agent_tool_execution_result",
            )));
        }
        persist_agent_formal_batch(
            store,
            command,
            &authorization,
            &canonical,
            vec![
                (
                    tool_command,
                    "ToolRequestApproved",
                    AgentEventPayload::ToolRequestApproved {
                        tool: decision.tool_request.tool().as_str(),
                        decision: tool_decision,
                        seal: AgentFormalEventSeal::new(),
                    },
                ),
                (
                    execution_command,
                    "ToolExecutionSucceeded",
                    AgentEventPayload::ToolExecutionSucceeded {
                        tool: decision.tool_request.tool().as_str(),
                        execution_id,
                        result_hash: execution.result_hash,
                        seal: AgentFormalEventSeal::new(),
                    },
                ),
                (
                    decision_command,
                    "DecisionCommitted",
                    AgentEventPayload::DecisionCommitted {
                        decision_id: decision.decision_id,
                        player_visible_text: redact_player_visible_text(
                            &decision.player_visible_text,
                        ),
                        linked_records: decision.linked_records,
                        audit_fields: decision.audit_fields,
                        seal: AgentFormalEventSeal::new(),
                    },
                ),
            ],
        )
    }
}

fn agent_execution_from_receipt(
    receipt: &CanonicalCommitReceipt,
    expected_tool: &str,
) -> AgentResult<AgentToolExecutionOutput> {
    if receipt.events.len() != 3 {
        return Err(AgentError::Core(TrpgError::AuditIntegrityViolation));
    }
    let event = receipt
        .events
        .get(1)
        .ok_or(TrpgError::AuditIntegrityViolation)?;
    if event.event_type != "ToolExecutionSucceeded" {
        return Err(AgentError::Core(TrpgError::AuditIntegrityViolation));
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
        return Err(AgentError::Core(TrpgError::AuditIntegrityViolation));
    }
    Ok(AgentToolExecutionOutput {
        execution_id: execution_id.to_owned(),
        result_hash: result_hash.to_owned(),
    })
}

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
