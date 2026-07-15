use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use rand_core::{OsRng, RngCore};
use sha2::{Digest, Sha256};
use trpg_contracts::WireErrorCode;
use trpg_identity::{AuthenticationContext, IdentityVerifier, PrincipalKind, ReplayAuthorization};
use trpg_security_governance::formal_commit_audit::{FormalAuthorization, FormalCommitAuthorizer};
use trpg_shared_kernel::{
    Actor, ActorRole, AuthorityContract, AuthorityMode, CanonicalCommitEvent, CanonicalCommitPort,
    CanonicalCommitRequest, CommandEnvelope, EntityId, EventEnvelope,
    EventStore as KernelEventStore, FormalWritePath, KernelResult, ProvenanceKind, TrpgError,
    Visibility, VisibilityLabel,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
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
    governed: Option<GovernedPendingBinding>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct GovernedPendingBinding {
    confirmation_id: [u8; 32],
    campaign_id: EntityId,
    authority_contract_id: EntityId,
    authority_contract_version: u64,
    authority_owner: EntityId,
    draft_hash: String,
    expires_at_unix_ms: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ConfirmedPendingDecision {
    pending: PendingDecision,
    confirmed_by: Actor,
    confirmation_authentication: AuthenticationContext,
    confirmed_at_unix_ms: u64,
    committed: bool,
}

impl ConfirmedPendingDecision {
    pub fn status(&self) -> PendingDecisionStatus {
        self.pending.status
    }

    pub fn confirmed_by(&self) -> &Actor {
        &self.confirmed_by
    }

    pub const fn confirmed_at_unix_ms(&self) -> u64 {
        self.confirmed_at_unix_ms
    }

    pub const fn is_committed(&self) -> bool {
        self.committed
    }
}

#[derive(Clone)]
pub struct HumanConfirmationGate {
    identity_verifier: IdentityVerifier,
    confirmation_state: Arc<Mutex<HumanConfirmationState>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ConfirmationLifecycle {
    Awaiting,
    Confirmed,
    Committed,
}

#[derive(Debug)]
struct HumanConfirmationState {
    instance_nonce: [u8; 32],
    next_sequence: u64,
    lifecycle_by_id: HashMap<[u8; 32], ConfirmationLifecycle>,
}

impl std::fmt::Debug for HumanConfirmationGate {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("HumanConfirmationGate")
            .field("identity_verifier", &self.identity_verifier)
            .field("confirmation_state", &"[REDACTED]")
            .finish()
    }
}

impl HumanConfirmationGate {
    pub fn new(identity_verifier: IdentityVerifier) -> RuntimeResult<Self> {
        let mut instance_nonce = [0_u8; 32];
        OsRng.try_fill_bytes(&mut instance_nonce).map_err(|_| {
            RuntimeError::Core(TrpgError::InvalidConfiguration(
                "confirmation_nonce_unavailable",
            ))
        })?;
        Ok(Self {
            identity_verifier,
            confirmation_state: Arc::new(Mutex::new(HumanConfirmationState {
                instance_nonce,
                next_sequence: 0,
                lifecycle_by_id: HashMap::new(),
            })),
        })
    }

    pub fn create_pending(
        &self,
        campaign_id: &EntityId,
        decision: RuntimeDecision,
        created_at_unix_ms: u64,
        expires_at_unix_ms: u64,
    ) -> RuntimeResult<PendingDecision> {
        let contract = canonical_contract(&self.identity_verifier, campaign_id)?;
        let draft_hash = decision_hash(&decision);
        let mut state = self.confirmation_state.lock().map_err(|_| {
            RuntimeError::Core(TrpgError::InvalidConfiguration(
                "confirmation_state_unavailable",
            ))
        })?;
        state.next_sequence = state
            .next_sequence
            .checked_add(1)
            .ok_or(RuntimeError::Core(TrpgError::InvalidConfiguration(
                "confirmation_sequence_exhausted",
            )))?;
        let confirmation_id = confirmation_id(
            &state.instance_nonce,
            state.next_sequence,
            &contract,
            &draft_hash,
            created_at_unix_ms,
            expires_at_unix_ms,
        );
        let pending = create_governed_pending_decision(
            &contract,
            decision,
            created_at_unix_ms,
            expires_at_unix_ms,
            confirmation_id,
        )?;
        state
            .lifecycle_by_id
            .insert(confirmation_id, ConfirmationLifecycle::Awaiting);
        Ok(pending)
    }

    pub fn confirm(
        &self,
        pending: &PendingDecision,
        authentication: &AuthenticationContext,
        submitted_decision: &RuntimeDecision,
        now_unix_ms: u64,
    ) -> RuntimeResult<ConfirmedPendingDecision> {
        let binding = pending
            .governed
            .as_ref()
            .ok_or(RuntimeError::Core(TrpgError::DecisionConfirmationRequired))?;
        let campaign_id = binding.campaign_id.clone();
        let confirmation_id = binding.confirmation_id;
        let contract = canonical_contract(&self.identity_verifier, &campaign_id)?;
        let confirmed = confirm_pending_decision(
            pending,
            &contract,
            &self.identity_verifier,
            authentication,
            submitted_decision,
            now_unix_ms,
        )?;
        let mut state = self.confirmation_state.lock().map_err(|_| {
            RuntimeError::Core(TrpgError::InvalidConfiguration(
                "confirmation_state_unavailable",
            ))
        })?;
        let lifecycle = state
            .lifecycle_by_id
            .get_mut(&confirmation_id)
            .ok_or(RuntimeError::Core(TrpgError::DecisionConfirmationRequired))?;
        if lifecycle != &ConfirmationLifecycle::Awaiting {
            return Err(RuntimeError::Core(TrpgError::DecisionAlreadyCommitted));
        }
        *lifecycle = ConfirmationLifecycle::Confirmed;
        Ok(confirmed)
    }

    pub fn commit(
        &self,
        store: &mut EventStore<RuntimeEventPayload>,
        command: &CommandEnvelope<RuntimeDecision>,
        workflow_authentication: &AuthenticationContext,
        confirmed: &mut ConfirmedPendingDecision,
        submitted_decision: RuntimeDecision,
        now_unix_ms: u64,
    ) -> RuntimeResult<Vec<EventEnvelope<RuntimeEventPayload>>> {
        let contract = canonical_contract(
            &self.identity_verifier,
            command.authenticated_context().resource().campaign_id(),
        )?;
        contract.validate_command(command)?;
        self.identity_verifier
            .verify_actor(
                workflow_authentication,
                &command.actor,
                command.authenticated_context().resource().campaign_id(),
                now_unix_ms,
            )
            .map_err(|_| RuntimeError::Core(TrpgError::InternalIdentityInvalid))?;
        self.identity_verifier
            .verify(&confirmed.confirmation_authentication, now_unix_ms)
            .map_err(|_| RuntimeError::Core(TrpgError::InternalIdentityInvalid))?;
        let confirmation_id = confirmed
            .pending
            .governed
            .as_ref()
            .ok_or(RuntimeError::Core(TrpgError::DecisionConfirmationRequired))?
            .confirmation_id;
        let mut state = self.confirmation_state.lock().map_err(|_| {
            RuntimeError::Core(TrpgError::InvalidConfiguration(
                "confirmation_state_unavailable",
            ))
        })?;
        let lifecycle = state
            .lifecycle_by_id
            .get_mut(&confirmation_id)
            .ok_or(RuntimeError::Core(TrpgError::DecisionConfirmationRequired))?;
        match lifecycle {
            ConfirmationLifecycle::Awaiting => {
                return Err(RuntimeError::Core(TrpgError::DecisionConfirmationRequired));
            }
            ConfirmationLifecycle::Committed => {
                return Err(RuntimeError::Core(TrpgError::DecisionAlreadyCommitted));
            }
            ConfirmationLifecycle::Confirmed => {}
        }
        let events = commit_confirmed_decision(
            store,
            &contract,
            command,
            workflow_authentication,
            confirmed,
            submitted_decision,
            now_unix_ms,
        )?;
        *lifecycle = ConfirmationLifecycle::Committed;
        Ok(events)
    }
}

fn canonical_contract(
    identity_verifier: &IdentityVerifier,
    campaign_id: &EntityId,
) -> RuntimeResult<AuthorityContract> {
    identity_verifier
        .authority_contract(campaign_id)
        .map_err(|_| RuntimeError::Core(TrpgError::AuthorityViolation))
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
        governed: None,
    }
}

fn create_governed_pending_decision(
    contract: &AuthorityContract,
    decision: RuntimeDecision,
    created_at_unix_ms: u64,
    expires_at_unix_ms: u64,
    confirmation_id: [u8; 32],
) -> RuntimeResult<PendingDecision> {
    if created_at_unix_ms == 0 || expires_at_unix_ms <= created_at_unix_ms {
        return Err(RuntimeError::Core(TrpgError::DecisionExpired));
    }
    let mut pending = create_pending_decision(contract.mode(), decision);
    if contract.mode() == &AuthorityMode::HumanKp
        && pending.decision.tool_request.is_formal_state_change()
    {
        pending.status = PendingDecisionStatus::AwaitingHumanConfirmation;
    }
    pending.governed = Some(GovernedPendingBinding {
        confirmation_id,
        campaign_id: contract.campaign_id().clone(),
        authority_contract_id: contract.contract_id().clone(),
        authority_contract_version: contract.version(),
        authority_owner: contract.authority_owner().clone(),
        draft_hash: decision_hash(&pending.decision),
        expires_at_unix_ms,
    });
    Ok(pending)
}

fn confirmation_id(
    instance_nonce: &[u8; 32],
    sequence: u64,
    contract: &AuthorityContract,
    draft_hash: &str,
    created_at_unix_ms: u64,
    expires_at_unix_ms: u64,
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(instance_nonce);
    hasher.update(sequence.to_be_bytes());
    hasher.update(contract.campaign_id().as_str().as_bytes());
    hasher.update(contract.contract_id().as_str().as_bytes());
    hasher.update(contract.version().to_be_bytes());
    hasher.update(draft_hash.as_bytes());
    hasher.update(created_at_unix_ms.to_be_bytes());
    hasher.update(expires_at_unix_ms.to_be_bytes());
    hasher.finalize().into()
}

fn confirm_pending_decision(
    pending: &PendingDecision,
    contract: &AuthorityContract,
    identity_verifier: &IdentityVerifier,
    authentication: &AuthenticationContext,
    submitted_decision: &RuntimeDecision,
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
    if decision_hash(submitted_decision) != binding.draft_hash {
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
    if confirmed.committed || confirmed.pending.status == PendingDecisionStatus::Committed {
        return Err(RuntimeError::Core(TrpgError::DecisionAlreadyCommitted));
    }
    let binding = validate_pending_binding(&confirmed.pending, contract, now_unix_ms)?;
    let submitted_hash = decision_hash(&submitted_decision);
    if submitted_hash != binding.draft_hash || decision_hash(&command.payload) != binding.draft_hash
    {
        return Err(RuntimeError::Core(TrpgError::DecisionDraftChanged));
    }
    if confirmed.confirmed_by.id() != contract.authority_owner()
        || confirmed.confirmed_by.role() != &ActorRole::HumanKeeper
    {
        return Err(RuntimeError::Core(TrpgError::AuthorityOwnerMismatch));
    }
    validate_runtime_command(contract, command)?;
    ensure_expected_version(store, command)?;
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

fn decision_hash(decision: &RuntimeDecision) -> String {
    let mut hasher = Sha256::new();
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
    format!("sha256:{:x}", hasher.finalize())
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
    match label {
        VisibilityLabel::Public => "public",
        VisibilityLabel::PartyVisible => "party_visible",
        VisibilityLabel::KeeperOnly => "keeper_only",
        VisibilityLabel::PrivateToPlayer => "private_to_player",
        VisibilityLabel::InvestigatorPrivate => "investigator_private",
        VisibilityLabel::AiInternal => "ai_internal",
        VisibilityLabel::SystemOnly => "system_only",
        VisibilityLabel::SystemPrivate => "system_private",
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub enum RuntimeEventPayload {
    ToolRequestApproved {
        tool: &'static str,
        grant: ToolGrantDecision,
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
    let actual = store.inner.current_stream_version();
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

pub fn commit_decision(
    store: &mut EventStore<RuntimeEventPayload>,
    contract: &AuthorityContract,
    command: &CommandEnvelope<RuntimeDecision>,
    workflow_authentication: &AuthenticationContext,
    decision: RuntimeDecision,
    now_unix_ms: u64,
) -> RuntimeResult<Vec<EventEnvelope<RuntimeEventPayload>>> {
    validate_runtime_command(contract, command)?;
    ensure_expected_version(store, command)?;
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

    let next_version = if store.has_canonical_custody() && store.events().is_empty() {
        command.expected_version
    } else {
        store.inner.current_stream_version()
    };
    let tool_command = derived_command(command, "tool", next_version)?;
    let decision_command = derived_command(command, "decision", next_version + 1)?;
    if store.events().iter().any(|event| {
        event.idempotency_key == tool_command.idempotency_key
            || event.idempotency_key == decision_command.idempotency_key
    }) {
        return Err(RuntimeError::Core(TrpgError::DuplicateCommand));
    }
    let requested_role = if human_confirmed {
        "human_keeper"
    } else {
        actor_role_name(command.actor.role())
    };
    let (authorization, canonical) = {
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
        )
    };
    persist_runtime_formal_batch(
        store,
        command,
        &authorization,
        &canonical,
        [
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

fn persist_runtime_formal_batch(
    store: &mut EventStore<RuntimeEventPayload>,
    command: &CommandEnvelope<RuntimeDecision>,
    authorization: &FormalAuthorization,
    canonical: &Arc<dyn CanonicalCommitPort>,
    events: [(
        CommandEnvelope<RuntimeDecision>,
        &'static str,
        RuntimeEventPayload,
    ); 2],
) -> RuntimeResult<Vec<EventEnvelope<RuntimeEventPayload>>> {
    let mut candidate = if store.events().is_empty() && command.expected_version > 0 {
        KernelEventStore::with_stream_base_version(command.expected_version)
    } else {
        store.inner.clone()
    };
    let mut appended = Vec::with_capacity(events.len());
    for (event_command, event_type, payload) in events {
        appended.push(candidate.append(&event_command, event_type, payload)?);
    }
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
        authority_mode: authority_mode_name(&command.authority_mode).to_owned(),
        authority_contract_version: contract.version(),
        authority_contract_id: contract.contract_id().to_string(),
        authority_owner: contract.authority_owner().to_string(),
        visibility_label: command.visibility.label().as_str().to_owned(),
        visibility_subject: command
            .visibility
            .player_id()
            .map(ToString::to_string)
            .unwrap_or_else(|| "not_applicable".to_owned()),
        provenance_kind: provenance_kind_name(&command.fact_provenance.kind).to_owned(),
        provenance_reference: command.fact_provenance.reference.to_string(),
        provenance_recorded_by: command.fact_provenance.recorded_by.to_string(),
        correlation_id: command.correlation_id.to_string(),
        causation_id: command.causation_id.to_string(),
        trace_id: command.authenticated_context().trace_id().to_string(),
        events: appended
            .iter()
            .map(|event| {
                Ok(CanonicalCommitEvent {
                    event_type: event.event_type.to_owned(),
                    payload_json: serde_json::to_string(&event.payload)
                        .map_err(|_| TrpgError::AuditIntegrityViolation)?,
                })
            })
            .collect::<KernelResult<Vec<_>>>()?,
        audit: authorization.canonical_audit().clone(),
    };
    let receipt = canonical.commit(&request)?;
    let expected_first = command
        .expected_version
        .checked_add(1)
        .ok_or(TrpgError::AuditIntegrityViolation)?;
    let expected_last = command
        .expected_version
        .checked_add(appended.len() as u64)
        .ok_or(TrpgError::AuditIntegrityViolation)?;
    if receipt.first_stream_version != expected_first
        || receipt.last_stream_version != expected_last
    {
        return Err(RuntimeError::Core(TrpgError::AuditIntegrityViolation));
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

#[cfg(test)]
mod security_regression_tests {
    use super::*;

    #[test]
    fn internal_generic_append_rejects_formal_decision_payloads() {
        let contract = trpg_test_support::authority_contract_with_owner(
            "campaign_human",
            AuthorityMode::HumanKp,
            "keeper_owner",
            1,
        )
        .unwrap();
        let decision = RuntimeDecision::new(
            "decision_forged",
            "forged",
            ToolRequest::formal(RuntimeAgent::HumanKeeper, RuntimeTool::CommitDecision),
        )
        .unwrap();
        let command = trpg_test_support::governed_command_for_contract(
            &contract,
            decision,
            ActorRole::Workflow,
        );
        let mut store = EventStore::default();
        let forged = RuntimeEventPayload::DecisionCommitted {
            decision_id: EntityId::new("forged_decision").unwrap(),
            linked_records: vec!["DecisionRecord"],
            player_visible_explanation: "forged".to_owned(),
            audit_fields: vec!["context_hash"],
            seal: RuntimeFormalEventSeal::new(),
        };

        assert_eq!(
            append_runtime_event(&mut store, &contract, &command, "DecisionCommitted", forged,)
                .unwrap_err(),
            RuntimeError::Core(TrpgError::PolicyDenied)
        );
        assert!(store.events().is_empty());
    }
}
