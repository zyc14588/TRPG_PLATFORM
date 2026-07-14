use std::collections::HashMap;
use std::error::Error;
use std::fmt;

use trpg_contracts::WireErrorCode;

pub type KernelResult<T> = Result<T, TrpgError>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrpgError {
    InvalidEntityId,
    UnknownVisibilityLabel,
    MissingIdempotencyKey,
    MissingCorrelationId,
    MissingCausationId,
    MissingFactProvenance,
    AuthorityViolation,
    AuthorityContractMutation,
    DirectAgentStateWrite,
    PolicyDenied,
    ExpectedVersionConflict { expected: u64, actual: u64 },
    DuplicateCommand,
    VisibilityDenied,
    InvalidConfiguration(&'static str),
    DependencyViolation(&'static str),
    CrateOwnershipViolation(&'static str),
    WorkspaceViolation(&'static str),
    CodingPolicyViolation(&'static str),
    OpenSourceReferenceViolation(&'static str),
    EventContractUnknown,
    EventContractVersionMismatch,
    AuthenticationRequired,
    AuthorizationDenied,
    CampaignScopeMismatch,
    AuthorityOwnerMismatch,
    AuthorityContractVersionConflict,
    InternalIdentityInvalid,
    PolicyUnavailable,
    PolicyEvidenceUntrusted,
    DecisionConfirmationRequired,
    DecisionDraftChanged,
    DecisionExpired,
    DecisionAlreadyCommitted,
    AuditIntegrityViolation,
}

impl TrpgError {
    pub const fn wire_code(&self) -> WireErrorCode {
        match self {
            Self::InvalidEntityId => WireErrorCode::InvalidEntityId,
            Self::UnknownVisibilityLabel => WireErrorCode::UnknownVisibilityLabel,
            Self::MissingIdempotencyKey => WireErrorCode::MissingIdempotencyKey,
            Self::MissingCorrelationId => WireErrorCode::MissingCorrelationId,
            Self::MissingCausationId => WireErrorCode::MissingCausationId,
            Self::MissingFactProvenance => WireErrorCode::MissingFactProvenance,
            Self::AuthorityViolation => WireErrorCode::AuthorityViolation,
            Self::AuthorityContractMutation => WireErrorCode::AuthorityContractMutation,
            Self::DirectAgentStateWrite => WireErrorCode::DirectAgentStateWrite,
            Self::PolicyDenied => WireErrorCode::PolicyDenied,
            Self::ExpectedVersionConflict { .. } => WireErrorCode::ExpectedVersionConflict,
            Self::DuplicateCommand => WireErrorCode::DuplicateCommand,
            Self::VisibilityDenied => WireErrorCode::VisibilityDenied,
            Self::InvalidConfiguration(_) => WireErrorCode::InvalidConfiguration,
            Self::DependencyViolation(_) => WireErrorCode::DependencyDirectionViolation,
            Self::CrateOwnershipViolation(_) => WireErrorCode::CrateOwnershipViolation,
            Self::WorkspaceViolation(_) => WireErrorCode::WorkspaceContractViolation,
            Self::CodingPolicyViolation(_) => WireErrorCode::RustCodingPolicyViolation,
            Self::OpenSourceReferenceViolation(_) => WireErrorCode::OpenSourceReferenceViolation,
            Self::EventContractUnknown => WireErrorCode::EventContractUnknown,
            Self::EventContractVersionMismatch => WireErrorCode::EventContractVersionMismatch,
            Self::AuthenticationRequired => WireErrorCode::AuthenticationRequired,
            Self::AuthorizationDenied => WireErrorCode::AuthorizationDenied,
            Self::CampaignScopeMismatch => WireErrorCode::CampaignScopeMismatch,
            Self::AuthorityOwnerMismatch => WireErrorCode::AuthorityOwnerMismatch,
            Self::AuthorityContractVersionConflict => {
                WireErrorCode::AuthorityContractVersionConflict
            }
            Self::InternalIdentityInvalid => WireErrorCode::InternalIdentityInvalid,
            Self::PolicyUnavailable => WireErrorCode::PolicyUnavailable,
            Self::PolicyEvidenceUntrusted => WireErrorCode::PolicyEvidenceUntrusted,
            Self::DecisionConfirmationRequired => WireErrorCode::DecisionConfirmationRequired,
            Self::DecisionDraftChanged => WireErrorCode::DecisionDraftChanged,
            Self::DecisionExpired => WireErrorCode::DecisionExpired,
            Self::DecisionAlreadyCommitted => WireErrorCode::DecisionAlreadyCommitted,
            Self::AuditIntegrityViolation => WireErrorCode::AuditIntegrityViolation,
        }
    }

    pub fn code(&self) -> &'static str {
        self.wire_code().as_str()
    }

    pub const fn http_status(&self) -> u16 {
        match self {
            Self::AuthenticationRequired | Self::InternalIdentityInvalid => 401,
            Self::AuthorizationDenied
            | Self::AuthorityViolation
            | Self::AuthorityOwnerMismatch
            | Self::CampaignScopeMismatch
            | Self::PolicyDenied
            | Self::VisibilityDenied
            | Self::DirectAgentStateWrite => 403,
            Self::AuthorityContractMutation
            | Self::AuthorityContractVersionConflict
            | Self::ExpectedVersionConflict { .. }
            | Self::DuplicateCommand
            | Self::DecisionDraftChanged
            | Self::DecisionExpired
            | Self::DecisionAlreadyCommitted => 409,
            Self::PolicyUnavailable => 503,
            Self::AuditIntegrityViolation => 500,
            _ => 400,
        }
    }

    pub fn retryable(&self) -> bool {
        matches!(
            self,
            Self::ExpectedVersionConflict { .. } | Self::DuplicateCommand
        )
    }
}

impl fmt::Display for TrpgError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.code())
    }
}

impl Error for TrpgError {}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct EntityId(String);

impl EntityId {
    pub fn new(value: impl Into<String>) -> KernelResult<Self> {
        let value = value.into();
        let trimmed = value.trim();
        if trimmed.is_empty()
            || !trimmed
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            return Err(TrpgError::InvalidEntityId);
        }

        Ok(Self(trimmed.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for EntityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VisibilityLabel {
    Public,
    PartyVisible,
    KeeperOnly,
    PrivateToPlayer,
    InvestigatorPrivate,
    AiInternal,
    SystemOnly,
    SystemPrivate,
}

impl VisibilityLabel {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::PartyVisible => "party_visible",
            Self::KeeperOnly => "keeper_only",
            Self::PrivateToPlayer => "private_to_player",
            Self::InvestigatorPrivate => "investigator_private",
            Self::AiInternal => "ai_internal",
            Self::SystemOnly => "system_only",
            Self::SystemPrivate => "system_private",
        }
    }
}

impl TryFrom<&str> for VisibilityLabel {
    type Error = TrpgError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "public" => Ok(Self::Public),
            "party_visible" => Ok(Self::PartyVisible),
            "keeper_only" => Ok(Self::KeeperOnly),
            "private_to_player" => Ok(Self::PrivateToPlayer),
            "investigator_private" => Ok(Self::InvestigatorPrivate),
            "ai_internal" => Ok(Self::AiInternal),
            "system_only" => Ok(Self::SystemOnly),
            "system_private" => Ok(Self::SystemPrivate),
            _ => Err(TrpgError::UnknownVisibilityLabel),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Visibility {
    label: VisibilityLabel,
    player_id: Option<EntityId>,
}

impl Visibility {
    pub fn new(label: VisibilityLabel) -> Self {
        Self {
            label,
            player_id: None,
        }
    }

    pub fn private_to_player(player_id: EntityId) -> Self {
        Self {
            label: VisibilityLabel::PrivateToPlayer,
            player_id: Some(player_id),
        }
    }

    pub fn label(&self) -> &VisibilityLabel {
        &self.label
    }

    pub fn can_view(&self, principal: &PrincipalScope) -> bool {
        match (&self.label, principal) {
            (VisibilityLabel::Public, _) => true,
            (VisibilityLabel::PartyVisible, PrincipalScope::PartyMember)
            | (VisibilityLabel::PartyVisible, PrincipalScope::Keeper)
            | (VisibilityLabel::PartyVisible, PrincipalScope::System) => true,
            (VisibilityLabel::KeeperOnly, PrincipalScope::Keeper)
            | (VisibilityLabel::KeeperOnly, PrincipalScope::System) => true,
            (VisibilityLabel::PrivateToPlayer, PrincipalScope::Player(player_id)) => {
                self.player_id.as_ref() == Some(player_id)
            }
            (VisibilityLabel::PrivateToPlayer, PrincipalScope::Keeper)
            | (VisibilityLabel::PrivateToPlayer, PrincipalScope::System) => true,
            (VisibilityLabel::InvestigatorPrivate, PrincipalScope::Player(player_id)) => {
                self.player_id.as_ref() == Some(player_id)
            }
            (VisibilityLabel::InvestigatorPrivate, PrincipalScope::Keeper)
            | (VisibilityLabel::InvestigatorPrivate, PrincipalScope::System) => true,
            (VisibilityLabel::AiInternal, PrincipalScope::System)
            | (VisibilityLabel::SystemOnly, PrincipalScope::System)
            | (VisibilityLabel::SystemPrivate, PrincipalScope::System) => true,
            _ => false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PrincipalScope {
    Public,
    PartyMember,
    Keeper,
    Player(EntityId),
    System,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProvenanceKind {
    HumanKeeperStatement,
    RulesEngineDecision,
    ToolResult,
    AgentProposal,
    ImportedSource,
    SystemFixture,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FactProvenance {
    pub kind: ProvenanceKind,
    pub reference: EntityId,
    pub recorded_by: EntityId,
}

impl FactProvenance {
    pub fn new(
        kind: ProvenanceKind,
        reference: impl Into<String>,
        recorded_by: impl Into<String>,
    ) -> KernelResult<Self> {
        Ok(Self {
            kind,
            reference: EntityId::new(reference)?,
            recorded_by: EntityId::new(recorded_by)?,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuthorityMode {
    HumanKp,
    AiKp,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActorRole {
    ServerOwner,
    CampaignOwner,
    HumanKeeper,
    AiKeeper,
    Investigator,
    Moderator,
    Spectator,
    Workflow,
    RulesEngine,
    System,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkloadRole {
    ApiServer,
    RealtimeServer,
    AgentWorker,
    WorkflowEngine,
    RulesEngine,
    AuditWriter,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentClass {
    AiKeeperOrchestrator,
    KeeperCopilot,
    AtmosphereWriter,
    MemoryCurator,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActorOrigin {
    UserSession {
        session_id: EntityId,
    },
    Workload {
        role: WorkloadRole,
    },
    AgentRun {
        run_id: EntityId,
        class: AgentClass,
        campaign_id: EntityId,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Actor {
    id: EntityId,
    role: ActorRole,
    origin: ActorOrigin,
}

impl Actor {
    pub fn authenticated_user(
        id: impl Into<String>,
        role: ActorRole,
        session_id: impl Into<String>,
    ) -> KernelResult<Self> {
        if !matches!(
            role,
            ActorRole::ServerOwner
                | ActorRole::CampaignOwner
                | ActorRole::HumanKeeper
                | ActorRole::Investigator
                | ActorRole::Moderator
                | ActorRole::Spectator
        ) {
            return Err(TrpgError::InternalIdentityInvalid);
        }
        Ok(Self {
            id: EntityId::new(id)?,
            role,
            origin: ActorOrigin::UserSession {
                session_id: EntityId::new(session_id)?,
            },
        })
    }

    pub fn verified_workload(id: impl Into<String>, role: WorkloadRole) -> KernelResult<Self> {
        let actor_role = match role {
            WorkloadRole::WorkflowEngine => ActorRole::Workflow,
            WorkloadRole::RulesEngine => ActorRole::RulesEngine,
            WorkloadRole::ApiServer
            | WorkloadRole::RealtimeServer
            | WorkloadRole::AgentWorker
            | WorkloadRole::AuditWriter => ActorRole::System,
        };
        Ok(Self {
            id: EntityId::new(id)?,
            role: actor_role,
            origin: ActorOrigin::Workload { role },
        })
    }

    pub fn verified_agent_run(
        agent_id: impl Into<String>,
        run_id: impl Into<String>,
        class: AgentClass,
        campaign_id: impl Into<String>,
    ) -> KernelResult<Self> {
        Ok(Self {
            id: EntityId::new(agent_id)?,
            role: if class == AgentClass::AiKeeperOrchestrator {
                ActorRole::AiKeeper
            } else {
                ActorRole::Investigator
            },
            origin: ActorOrigin::AgentRun {
                run_id: EntityId::new(run_id)?,
                class,
                campaign_id: EntityId::new(campaign_id)?,
            },
        })
    }

    pub fn id(&self) -> &EntityId {
        &self.id
    }

    pub fn role(&self) -> &ActorRole {
        &self.role
    }

    pub fn origin(&self) -> &ActorOrigin {
        &self.origin
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourceRef {
    campaign_id: EntityId,
    resource_type: EntityId,
    resource_id: EntityId,
}

impl ResourceRef {
    pub fn new(
        campaign_id: impl Into<String>,
        resource_type: impl Into<String>,
        resource_id: impl Into<String>,
    ) -> KernelResult<Self> {
        Ok(Self {
            campaign_id: EntityId::new(campaign_id)?,
            resource_type: EntityId::new(resource_type)?,
            resource_id: EntityId::new(resource_id)?,
        })
    }

    pub fn campaign_id(&self) -> &EntityId {
        &self.campaign_id
    }

    pub fn resource_type(&self) -> &EntityId {
        &self.resource_type
    }

    pub fn resource_id(&self) -> &EntityId {
        &self.resource_id
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthorityBinding {
    contract_id: EntityId,
    authority_owner: EntityId,
    contract_version: u64,
}

impl AuthorityBinding {
    pub fn new(
        contract_id: impl Into<String>,
        authority_owner: impl Into<String>,
        contract_version: u64,
    ) -> KernelResult<Self> {
        if contract_version == 0 {
            return Err(TrpgError::AuthorityContractVersionConflict);
        }
        Ok(Self {
            contract_id: EntityId::new(contract_id)?,
            authority_owner: EntityId::new(authority_owner)?,
            contract_version,
        })
    }

    pub fn contract_id(&self) -> &EntityId {
        &self.contract_id
    }

    pub fn authority_owner(&self) -> &EntityId {
        &self.authority_owner
    }

    pub const fn contract_version(&self) -> u64 {
        self.contract_version
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthenticatedCommandContext {
    actor: Actor,
    resource: ResourceRef,
    authority: AuthorityBinding,
    trace_id: EntityId,
    authenticated_at_unix_ms: u64,
    authentication_expires_at_unix_ms: u64,
}

impl AuthenticatedCommandContext {
    pub fn new(
        actor: Actor,
        resource: ResourceRef,
        authority: AuthorityBinding,
        trace_id: impl Into<String>,
        authenticated_at_unix_ms: u64,
        authentication_expires_at_unix_ms: u64,
    ) -> KernelResult<Self> {
        if authenticated_at_unix_ms == 0
            || authentication_expires_at_unix_ms <= authenticated_at_unix_ms
        {
            return Err(TrpgError::AuthenticationRequired);
        }
        if let ActorOrigin::AgentRun { campaign_id, .. } = actor.origin() {
            if campaign_id != resource.campaign_id() {
                return Err(TrpgError::CampaignScopeMismatch);
            }
        }
        Ok(Self {
            actor,
            resource,
            authority,
            trace_id: EntityId::new(trace_id)?,
            authenticated_at_unix_ms,
            authentication_expires_at_unix_ms,
        })
    }

    pub fn actor(&self) -> &Actor {
        &self.actor
    }

    pub fn resource(&self) -> &ResourceRef {
        &self.resource
    }

    pub fn authority(&self) -> &AuthorityBinding {
        &self.authority
    }

    pub fn trace_id(&self) -> &EntityId {
        &self.trace_id
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FormalWritePath {
    WorkflowDecision,
    RulesDecision,
    ToolDecision,
    DirectAgent,
    DirectBusiness,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangePolicy {
    ForkOnly,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthorityVersionSnapshot {
    ruleset_version: EntityId,
    house_rules_version: EntityId,
    scenario_version: EntityId,
    prompt_version: EntityId,
    agent_pack_version: EntityId,
    tool_schema_version: EntityId,
    safety_profile_version: EntityId,
    ai_provider_snapshot: EntityId,
    model_route_snapshot: EntityId,
    character_sheet_template_version: EntityId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthorityVersionSnapshotDraft {
    pub ruleset_version: String,
    pub house_rules_version: String,
    pub scenario_version: String,
    pub prompt_version: String,
    pub agent_pack_version: String,
    pub tool_schema_version: String,
    pub safety_profile_version: String,
    pub ai_provider_snapshot: String,
    pub model_route_snapshot: String,
    pub character_sheet_template_version: String,
}

impl AuthorityVersionSnapshot {
    pub fn from_draft(draft: AuthorityVersionSnapshotDraft) -> KernelResult<Self> {
        Ok(Self {
            ruleset_version: EntityId::new(draft.ruleset_version)?,
            house_rules_version: EntityId::new(draft.house_rules_version)?,
            scenario_version: EntityId::new(draft.scenario_version)?,
            prompt_version: EntityId::new(draft.prompt_version)?,
            agent_pack_version: EntityId::new(draft.agent_pack_version)?,
            tool_schema_version: EntityId::new(draft.tool_schema_version)?,
            safety_profile_version: EntityId::new(draft.safety_profile_version)?,
            ai_provider_snapshot: EntityId::new(draft.ai_provider_snapshot)?,
            model_route_snapshot: EntityId::new(draft.model_route_snapshot)?,
            character_sheet_template_version: EntityId::new(
                draft.character_sheet_template_version,
            )?,
        })
    }

    pub fn ruleset_version(&self) -> &EntityId {
        &self.ruleset_version
    }

    pub fn house_rules_version(&self) -> &EntityId {
        &self.house_rules_version
    }

    pub fn scenario_version(&self) -> &EntityId {
        &self.scenario_version
    }

    pub fn prompt_version(&self) -> &EntityId {
        &self.prompt_version
    }

    pub fn agent_pack_version(&self) -> &EntityId {
        &self.agent_pack_version
    }

    pub fn tool_schema_version(&self) -> &EntityId {
        &self.tool_schema_version
    }

    pub fn safety_profile_version(&self) -> &EntityId {
        &self.safety_profile_version
    }

    pub fn ai_provider_snapshot(&self) -> &EntityId {
        &self.ai_provider_snapshot
    }

    pub fn model_route_snapshot(&self) -> &EntityId {
        &self.model_route_snapshot
    }

    pub fn character_sheet_template_version(&self) -> &EntityId {
        &self.character_sheet_template_version
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthorityContractDraft {
    pub contract_id: String,
    pub campaign_id: String,
    pub mode: AuthorityMode,
    pub authority_owner: String,
    pub version: u64,
    pub snapshot: AuthorityVersionSnapshotDraft,
    pub created_at_unix_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthorityContract {
    contract_id: EntityId,
    campaign_id: EntityId,
    mode: AuthorityMode,
    authority_owner: EntityId,
    version: u64,
    snapshot: AuthorityVersionSnapshot,
    created_at_unix_ms: u64,
    locked: bool,
    change_policy: ChangePolicy,
}

impl AuthorityContract {
    pub fn new_locked(draft: AuthorityContractDraft) -> KernelResult<Self> {
        if draft.version == 0 || draft.created_at_unix_ms == 0 {
            return Err(TrpgError::AuthorityContractMutation);
        }
        Ok(Self {
            contract_id: EntityId::new(draft.contract_id)?,
            campaign_id: EntityId::new(draft.campaign_id)?,
            mode: draft.mode,
            authority_owner: EntityId::new(draft.authority_owner)?,
            version: draft.version,
            snapshot: AuthorityVersionSnapshot::from_draft(draft.snapshot)?,
            created_at_unix_ms: draft.created_at_unix_ms,
            locked: true,
            change_policy: ChangePolicy::ForkOnly,
        })
    }

    pub fn contract_id(&self) -> &EntityId {
        &self.contract_id
    }

    pub fn campaign_id(&self) -> &EntityId {
        &self.campaign_id
    }

    pub fn mode(&self) -> &AuthorityMode {
        &self.mode
    }

    pub fn authority_mode(&self) -> &AuthorityMode {
        &self.mode
    }

    pub fn authority_owner(&self) -> &EntityId {
        &self.authority_owner
    }

    pub const fn version(&self) -> u64 {
        self.version
    }

    pub fn snapshot(&self) -> &AuthorityVersionSnapshot {
        &self.snapshot
    }

    pub const fn created_at_unix_ms(&self) -> u64 {
        self.created_at_unix_ms
    }

    pub const fn is_locked(&self) -> bool {
        self.locked
    }

    pub const fn change_policy(&self) -> ChangePolicy {
        self.change_policy
    }

    pub fn binding(&self) -> KernelResult<AuthorityBinding> {
        AuthorityBinding::new(
            self.contract_id.as_str(),
            self.authority_owner.as_str(),
            self.version,
        )
    }

    pub fn fork_with_draft(&self, draft: AuthorityContractDraft) -> KernelResult<Self> {
        if draft.campaign_id == self.campaign_id.as_str()
            || draft.contract_id == self.contract_id.as_str()
        {
            return Err(TrpgError::AuthorityContractMutation);
        }
        Self::new_locked(draft)
    }

    pub fn fork_for_child(
        &self,
        child_campaign_id: impl Into<String>,
        child_mode: AuthorityMode,
        child_owner: impl Into<String>,
    ) -> KernelResult<Self> {
        let child_campaign_id = child_campaign_id.into();
        self.fork_with_draft(AuthorityContractDraft {
            contract_id: format!("authority_contract_{child_campaign_id}_1"),
            campaign_id: child_campaign_id,
            mode: child_mode,
            authority_owner: child_owner.into(),
            version: 1,
            snapshot: AuthorityVersionSnapshotDraft {
                ruleset_version: self.snapshot.ruleset_version.to_string(),
                house_rules_version: self.snapshot.house_rules_version.to_string(),
                scenario_version: self.snapshot.scenario_version.to_string(),
                prompt_version: self.snapshot.prompt_version.to_string(),
                agent_pack_version: self.snapshot.agent_pack_version.to_string(),
                tool_schema_version: self.snapshot.tool_schema_version.to_string(),
                safety_profile_version: self.snapshot.safety_profile_version.to_string(),
                ai_provider_snapshot: self.snapshot.ai_provider_snapshot.to_string(),
                model_route_snapshot: self.snapshot.model_route_snapshot.to_string(),
                character_sheet_template_version: self
                    .snapshot
                    .character_sheet_template_version
                    .to_string(),
            },
            created_at_unix_ms: self.created_at_unix_ms.saturating_add(1),
        })
    }

    /// Authority is immutable inside a campaign. Existing call sites that try
    /// to change only mode/version are rejected; a legitimate fork must name a
    /// distinct child campaign through `fork_for_child` or `fork_with_draft`.
    pub fn fork(&self, _mode: AuthorityMode, _version: u64) -> KernelResult<Self> {
        Err(TrpgError::AuthorityContractMutation)
    }

    pub fn reject_in_place_authority_change(
        &self,
        attempted_mode: &AuthorityMode,
        attempted_owner: &EntityId,
    ) -> KernelResult<()> {
        if &self.mode != attempted_mode || &self.authority_owner != attempted_owner {
            return Err(TrpgError::AuthorityContractMutation);
        }
        Ok(())
    }

    pub fn validate_command<T>(&self, command: &CommandEnvelope<T>) -> KernelResult<()> {
        if !self.locked || self.change_policy != ChangePolicy::ForkOnly {
            return Err(TrpgError::AuthorityContractMutation);
        }
        if self.mode != command.authority_mode {
            return Err(TrpgError::AuthorityViolation);
        }
        let context = command.authenticated_context();
        if context.resource().campaign_id() != &self.campaign_id {
            return Err(TrpgError::CampaignScopeMismatch);
        }
        if context.authority().contract_id() != &self.contract_id {
            return Err(TrpgError::AuthorityContractMutation);
        }
        if context.authority().authority_owner() != &self.authority_owner {
            return Err(TrpgError::AuthorityOwnerMismatch);
        }
        if context.authority().contract_version() != self.version
            || command.authority_contract_version != self.version
        {
            return Err(TrpgError::AuthorityContractVersionConflict);
        }
        if command.actor.role() == &ActorRole::HumanKeeper
            && command.actor.id() != &self.authority_owner
        {
            return Err(TrpgError::AuthorityOwnerMismatch);
        }
        validate_command_envelope(command)
    }
}

/// Canonical, process-local view of persisted Authority Contracts. A campaign
/// can be registered exactly once; changing mode, owner, version, or contract
/// id requires a distinct child campaign fork.
#[derive(Clone, Debug, Default)]
pub struct AuthorityRegistry {
    contracts_by_campaign: HashMap<EntityId, AuthorityContract>,
}

impl AuthorityRegistry {
    pub fn register(&mut self, contract: AuthorityContract) -> KernelResult<()> {
        match self.contracts_by_campaign.get(contract.campaign_id()) {
            Some(existing) if existing == &contract => Ok(()),
            Some(_) => Err(TrpgError::AuthorityContractMutation),
            None => {
                self.contracts_by_campaign
                    .insert(contract.campaign_id().clone(), contract);
                Ok(())
            }
        }
    }

    pub fn from_contracts(
        contracts: impl IntoIterator<Item = AuthorityContract>,
    ) -> KernelResult<Self> {
        let mut registry = Self::default();
        for contract in contracts {
            registry.register(contract)?;
        }
        Ok(registry)
    }

    pub fn contract_for(&self, campaign_id: &EntityId) -> KernelResult<&AuthorityContract> {
        self.contracts_by_campaign
            .get(campaign_id)
            .ok_or(TrpgError::AuthorityViolation)
    }

    pub fn validate_command<T>(
        &self,
        command: &CommandEnvelope<T>,
    ) -> KernelResult<&AuthorityContract> {
        let campaign_id = command.authenticated_context().resource().campaign_id();
        let contract = self.contract_for(campaign_id)?;
        contract.validate_command(command)?;
        Ok(contract)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandEnvelope<T> {
    pub command_id: EntityId,
    pub idempotency_key: String,
    pub expected_version: u64,
    pub actor: Actor,
    pub authority_mode: AuthorityMode,
    pub authority_contract_version: u64,
    pub visibility: Visibility,
    pub fact_provenance: FactProvenance,
    pub correlation_id: EntityId,
    pub causation_id: EntityId,
    pub write_path: FormalWritePath,
    pub payload: T,
    authenticated_context: AuthenticatedCommandContext,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandMetadata {
    pub command_id: EntityId,
    pub idempotency_key: String,
    pub expected_version: u64,
    pub authority_mode: AuthorityMode,
    pub visibility: Visibility,
    pub fact_provenance: FactProvenance,
    pub correlation_id: EntityId,
    pub causation_id: EntityId,
    pub write_path: FormalWritePath,
    pub authenticated_context: AuthenticatedCommandContext,
}

impl<T> CommandEnvelope<T> {
    pub fn new(payload: T, metadata: CommandMetadata) -> Self {
        let actor = metadata.authenticated_context.actor().clone();
        let authority_contract_version = metadata
            .authenticated_context
            .authority()
            .contract_version();
        Self {
            command_id: metadata.command_id,
            idempotency_key: metadata.idempotency_key,
            expected_version: metadata.expected_version,
            actor,
            authority_mode: metadata.authority_mode,
            authority_contract_version,
            visibility: metadata.visibility,
            fact_provenance: metadata.fact_provenance,
            correlation_id: metadata.correlation_id,
            causation_id: metadata.causation_id,
            write_path: metadata.write_path,
            payload,
            authenticated_context: metadata.authenticated_context,
        }
    }

    pub fn authenticated_context(&self) -> &AuthenticatedCommandContext {
        &self.authenticated_context
    }
}

pub fn validate_command_envelope<T>(command: &CommandEnvelope<T>) -> KernelResult<()> {
    if command.idempotency_key.trim().is_empty() {
        return Err(TrpgError::MissingIdempotencyKey);
    }

    match command.write_path {
        FormalWritePath::DirectAgent => return Err(TrpgError::DirectAgentStateWrite),
        FormalWritePath::DirectBusiness => return Err(TrpgError::PolicyDenied),
        FormalWritePath::WorkflowDecision
        | FormalWritePath::RulesDecision
        | FormalWritePath::ToolDecision => {}
    }

    let context = command.authenticated_context();
    if &command.actor != context.actor() {
        return Err(TrpgError::InternalIdentityInvalid);
    }
    if context.authentication_expires_at_unix_ms <= context.authenticated_at_unix_ms {
        return Err(TrpgError::AuthenticationRequired);
    }

    match (&command.authority_mode, command.actor.role()) {
        (AuthorityMode::HumanKp, ActorRole::HumanKeeper)
        | (AuthorityMode::HumanKp, ActorRole::Workflow)
        | (AuthorityMode::HumanKp, ActorRole::RulesEngine)
        | (AuthorityMode::HumanKp, ActorRole::System) => {}
        (AuthorityMode::AiKp, ActorRole::Workflow)
        | (AuthorityMode::AiKp, ActorRole::RulesEngine)
        | (AuthorityMode::AiKp, ActorRole::System) => {}
        _ => return Err(TrpgError::AuthorityViolation),
    }

    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventEnvelope<P> {
    pub sequence: u64,
    pub event_type: &'static str,
    pub campaign_id: EntityId,
    pub authenticated_actor: Actor,
    pub resource: ResourceRef,
    pub authority_contract_id: EntityId,
    pub authority_owner: EntityId,
    pub command_id: EntityId,
    pub idempotency_key: String,
    pub authority_contract_version: u64,
    pub visibility: Visibility,
    pub fact_provenance: FactProvenance,
    pub correlation_id: EntityId,
    pub causation_id: EntityId,
    pub trace_id: EntityId,
    pub occurred_at_unix_ms: u64,
    pub payload: P,
}

#[derive(Clone, Debug)]
pub struct EventStore<P> {
    events: Vec<EventEnvelope<P>>,
    idempotency_index: HashMap<String, u64>,
}

impl<P> Default for EventStore<P> {
    fn default() -> Self {
        Self {
            events: Vec::new(),
            idempotency_index: HashMap::new(),
        }
    }
}

impl<P: Clone> EventStore<P> {
    pub fn append<T>(
        &mut self,
        command: &CommandEnvelope<T>,
        event_type: &'static str,
        payload: P,
    ) -> KernelResult<EventEnvelope<P>> {
        validate_command_envelope(command)?;

        let actual_version = self.events.len() as u64;
        if command.expected_version != actual_version {
            return Err(TrpgError::ExpectedVersionConflict {
                expected: command.expected_version,
                actual: actual_version,
            });
        }

        if self
            .idempotency_index
            .contains_key(&command.idempotency_key)
        {
            return Err(TrpgError::DuplicateCommand);
        }

        let event = EventEnvelope {
            sequence: actual_version + 1,
            event_type,
            campaign_id: command
                .authenticated_context()
                .resource()
                .campaign_id()
                .clone(),
            authenticated_actor: command.actor.clone(),
            resource: command.authenticated_context().resource().clone(),
            authority_contract_id: command
                .authenticated_context()
                .authority()
                .contract_id()
                .clone(),
            authority_owner: command
                .authenticated_context()
                .authority()
                .authority_owner()
                .clone(),
            command_id: command.command_id.clone(),
            idempotency_key: command.idempotency_key.clone(),
            authority_contract_version: command.authority_contract_version,
            visibility: command.visibility.clone(),
            fact_provenance: command.fact_provenance.clone(),
            correlation_id: command.correlation_id.clone(),
            causation_id: command.causation_id.clone(),
            trace_id: command.authenticated_context().trace_id().clone(),
            occurred_at_unix_ms: unix_time_ms(),
            payload,
        };

        self.idempotency_index
            .insert(command.idempotency_key.clone(), event.sequence);
        self.events.push(event.clone());

        Ok(event)
    }

    pub fn events(&self) -> &[EventEnvelope<P>] {
        &self.events
    }

    pub fn replay_visible(&self, principal: &PrincipalScope) -> Vec<EventEnvelope<P>> {
        self.events
            .iter()
            .filter(|event| event.visibility.can_view(principal))
            .cloned()
            .collect()
    }
}

fn unix_time_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KernelContractSnapshot {
    pub id_format: &'static str,
    pub version_policy: &'static str,
    pub visibility_enum: Vec<&'static str>,
    pub error_codes: Vec<&'static str>,
}

pub fn kernel_contract_snapshot() -> KernelContractSnapshot {
    KernelContractSnapshot {
        id_format: "non_empty_ascii_alnum_underscore_dash",
        version_policy: "expected_version_plus_immutable_authority_contract",
        visibility_enum: vec![
            VisibilityLabel::Public.as_str(),
            VisibilityLabel::PartyVisible.as_str(),
            VisibilityLabel::KeeperOnly.as_str(),
            VisibilityLabel::PrivateToPlayer.as_str(),
            VisibilityLabel::InvestigatorPrivate.as_str(),
            VisibilityLabel::AiInternal.as_str(),
            VisibilityLabel::SystemOnly.as_str(),
            VisibilityLabel::SystemPrivate.as_str(),
        ],
        error_codes: vec![
            TrpgError::InvalidEntityId.code(),
            TrpgError::UnknownVisibilityLabel.code(),
            TrpgError::AuthorityViolation.code(),
            TrpgError::ExpectedVersionConflict {
                expected: 0,
                actual: 1,
            }
            .code(),
        ],
    }
}
