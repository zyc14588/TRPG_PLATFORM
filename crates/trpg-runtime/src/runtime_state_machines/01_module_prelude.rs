use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use rand_core::{OsRng, RngCore};
use sha2::{Digest, Sha256};
use trpg_contracts::WireErrorCode;
use trpg_identity::{AuthenticationContext, IdentityVerifier, PrincipalKind, ReplayAuthorization};
use trpg_security_governance::formal_commit_audit::{FormalAuthorization, FormalCommitAuthorizer};
use trpg_shared_kernel::{
    Actor, ActorRole, AuthorityContract, AuthorityMode, CanonicalCommitEvent, CanonicalCommitKey,
    CanonicalCommitPort, CanonicalCommitReceipt, CanonicalCommitRequest, CommandEnvelope, EntityId,
    EventEnvelope, EventStore as KernelEventStore, FormalWritePath, KernelResult, ProvenanceKind,
    TrpgError, Visibility, VisibilityLabel,
};

pub type RuntimeResult<T> = Result<T, RuntimeError>;

/// Runtime event storage deliberately exposes replay but not a generic append
/// capability. Formal decisions can therefore only be written by the
/// authority and confirmation gates in this crate.
#[derive(Clone, Debug)]
pub struct EventStore<P> {
    inner: KernelEventStore<P>,
    formal_custody: Option<FormalCommitCustody>,
}

#[derive(Clone)]
struct FormalCommitCustody {
    authorizer: FormalCommitAuthorizer,
    canonical: Arc<dyn CanonicalCommitPort>,
    tool_executor: Arc<dyn RuntimeToolExecutor>,
}

impl std::fmt::Debug for FormalCommitCustody {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FormalCommitCustody")
            .field("authorizer", &self.authorizer)
            .field("canonical", &self.canonical)
            .field("tool_executor", &"[TRUSTED TOOL EXECUTOR]")
            .finish()
    }
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
                tool_executor: Arc::new(RejectingRuntimeToolExecutor),
            }),
        }
    }

    pub fn with_formal_custody_and_executor(
        formal_authorizer: FormalCommitAuthorizer,
        canonical: Arc<dyn CanonicalCommitPort>,
        tool_executor: Arc<dyn RuntimeToolExecutor>,
    ) -> Self {
        Self {
            inner: KernelEventStore::default(),
            formal_custody: Some(FormalCommitCustody {
                authorizer: formal_authorizer,
                canonical,
                tool_executor,
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
    ) -> RuntimeResult<Vec<EventEnvelope<P>>> {
        let mut visible = Vec::new();
        for event in self.inner.events() {
            let allowed = authorization
                .can_view(&event.campaign_id, &event.visibility, now_unix_ms)
                .map_err(|error| match error {
                    trpg_identity::IdentityError::MembershipRequired
                    | trpg_identity::IdentityError::MembershipDenied
                    | trpg_identity::IdentityError::CampaignScopeMismatch => {
                        RuntimeError::Core(TrpgError::AuthorizationDenied)
                    }
                    _ => RuntimeError::Core(TrpgError::AuthenticationRequired),
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
    ) -> KernelResult<EventEnvelope<P>> {
        self.inner.append(command, event_type, payload)
    }

    fn formal_custody(&self) -> KernelResult<&FormalCommitCustody> {
        self.formal_custody
            .as_ref()
            .ok_or(TrpgError::AuditIntegrityViolation)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuntimeError {
    Core(TrpgError),
    AgentToolNotAllowed,
    HumanKpAiDraftOnly,
    AgentDirectStateWriteForbidden,
}

impl RuntimeError {
    pub const fn wire_code(&self) -> WireErrorCode {
        match self {
            Self::Core(error) => error.wire_code(),
            Self::AgentToolNotAllowed => WireErrorCode::AgentToolNotAllowed,
            Self::HumanKpAiDraftOnly => WireErrorCode::HumanKpAiDraftOnly,
            Self::AgentDirectStateWriteForbidden => WireErrorCode::AgentDirectStateWriteForbidden,
        }
    }

    pub fn code(&self) -> &'static str {
        self.wire_code().as_str()
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
    requested_by: RuntimeAgent,
    tool: RuntimeTool,
    visibility: Visibility,
}

impl ToolRequest {
    pub fn formal(requested_by: RuntimeAgent, tool: RuntimeTool) -> Self {
        Self {
            requested_by,
            tool,
            visibility: Visibility::new(VisibilityLabel::Public),
        }
    }

    pub fn draft(requested_by: RuntimeAgent, _tool: RuntimeTool) -> Self {
        Self {
            requested_by,
            tool: RuntimeTool::NarrationOnly,
            visibility: Visibility::new(VisibilityLabel::KeeperOnly),
        }
    }

    pub const fn requested_by(&self) -> RuntimeAgent {
        self.requested_by
    }

    pub const fn tool(&self) -> RuntimeTool {
        self.tool
    }

    pub fn visibility(&self) -> &Visibility {
        &self.visibility
    }

    pub const fn is_formal_state_change(&self) -> bool {
        !matches!(self.tool, RuntimeTool::NarrationOnly)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
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
    if request.is_formal_state_change() && request.requested_by() == RuntimeAgent::AtmosphereWriter
    {
        return ToolGrantDecision::deny(RuntimeError::AgentToolNotAllowed, false, false);
    }

    match authority_mode {
        AuthorityMode::HumanKp if request.requested_by().is_ai() => {
            ToolGrantDecision::deny(RuntimeError::HumanKpAiDraftOnly, true, true)
        }
        AuthorityMode::HumanKp => ToolGrantDecision {
            requires_human_confirmation: request.is_formal_state_change(),
            ..ToolGrantDecision::allow()
        },
        AuthorityMode::AiKp
            if request.is_formal_state_change()
                && request.requested_by() != RuntimeAgent::AiKeeperOrchestrator =>
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeToolExecutionOutput {
    pub execution_id: String,
    pub result_hash: String,
}

pub trait RuntimeToolExecutor: Send + Sync {
    fn execute(&self, decision: &RuntimeDecision) -> RuntimeResult<RuntimeToolExecutionOutput>;
}

#[derive(Debug)]
struct RejectingRuntimeToolExecutor;

impl RuntimeToolExecutor for RejectingRuntimeToolExecutor {
    fn execute(&self, _decision: &RuntimeDecision) -> RuntimeResult<RuntimeToolExecutionOutput> {
        Err(RuntimeError::AgentToolNotAllowed)
    }
}
