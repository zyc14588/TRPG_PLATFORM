use std::sync::Arc;

use sha2::{Digest, Sha256};
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
