use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use trpg_contracts::{CanonicalEvent, EventDescriptor, WireErrorCode};

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
        }
    }

    pub fn code(&self) -> &'static str {
        self.wire_code().as_str()
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
    HumanKeeper,
    AiKeeper,
    Investigator,
    Workflow,
    RulesEngine,
    System,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Actor {
    pub id: EntityId,
    pub role: ActorRole,
}

impl Actor {
    pub fn new(id: impl Into<String>, role: ActorRole) -> KernelResult<Self> {
        Ok(Self {
            id: EntityId::new(id)?,
            role,
        })
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthorityContract {
    campaign_id: EntityId,
    mode: AuthorityMode,
    version: u64,
}

impl AuthorityContract {
    pub fn new(
        campaign_id: impl Into<String>,
        mode: AuthorityMode,
        version: u64,
    ) -> KernelResult<Self> {
        if version == 0 {
            return Err(TrpgError::AuthorityContractMutation);
        }

        Ok(Self {
            campaign_id: EntityId::new(campaign_id)?,
            mode,
            version,
        })
    }

    pub fn campaign_id(&self) -> &EntityId {
        &self.campaign_id
    }

    pub fn mode(&self) -> &AuthorityMode {
        &self.mode
    }

    pub fn version(&self) -> u64 {
        self.version
    }

    pub fn fork(&self, mode: AuthorityMode, version: u64) -> KernelResult<Self> {
        if version <= self.version {
            return Err(TrpgError::AuthorityContractMutation);
        }

        Ok(Self {
            campaign_id: self.campaign_id.clone(),
            mode,
            version,
        })
    }

    pub fn validate_command<T>(&self, command: &CommandEnvelope<T>) -> KernelResult<()> {
        if self.version != command.authority_contract_version || self.mode != command.authority_mode
        {
            return Err(TrpgError::AuthorityContractMutation);
        }

        validate_command_envelope(command)
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

    match (&command.authority_mode, &command.actor.role) {
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
    pub event_descriptor: Option<EventDescriptor>,
    pub command_id: EntityId,
    pub idempotency_key: String,
    pub authority_contract_version: u64,
    pub visibility: Visibility,
    pub fact_provenance: FactProvenance,
    pub correlation_id: EntityId,
    pub causation_id: EntityId,
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
        let descriptor = CanonicalEvent::lookup(event_type)
            .ok()
            .map(CanonicalEvent::descriptor);
        self.append_with_descriptor(command, event_type, descriptor, payload)
    }

    pub fn append_canonical<T>(
        &mut self,
        command: &CommandEnvelope<T>,
        event: CanonicalEvent,
        payload: P,
    ) -> KernelResult<EventEnvelope<P>> {
        self.append_with_descriptor(command, event.name(), Some(event.descriptor()), payload)
    }

    fn append_with_descriptor<T>(
        &mut self,
        command: &CommandEnvelope<T>,
        event_type: &'static str,
        event_descriptor: Option<EventDescriptor>,
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
            event_descriptor,
            command_id: command.command_id.clone(),
            idempotency_key: command.idempotency_key.clone(),
            authority_contract_version: command.authority_contract_version,
            visibility: command.visibility.clone(),
            fact_provenance: command.fact_provenance.clone(),
            correlation_id: command.correlation_id.clone(),
            causation_id: command.causation_id.clone(),
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
