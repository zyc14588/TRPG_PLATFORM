use std::collections::HashMap;
use std::error::Error;
use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest, Sha256};

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

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VisibilityKind {
    Public,
    PartyVisible,
    PrivateToPlayer,
    PrivateToGroup,
    KeeperOnly,
    InvestigatorPrivate,
    AiInternal,
    SystemOnly,
    SpectatorVisible,
    SpectatorHidden,
    SystemPrivate,
}

impl VisibilityKind {
    pub const fn requires_subject(self) -> bool {
        matches!(
            self,
            Self::PrivateToPlayer | Self::PrivateToGroup | Self::InvestigatorPrivate
        )
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::PartyVisible => "party_visible",
            Self::PrivateToPlayer => "private_to_player",
            Self::PrivateToGroup => "private_to_group",
            Self::KeeperOnly => "keeper_only",
            Self::InvestigatorPrivate => "investigator_private",
            Self::AiInternal => "ai_internal",
            Self::SystemOnly => "system_only",
            Self::SpectatorVisible => "spectator_visible",
            Self::SpectatorHidden => "spectator_hidden",
            Self::SystemPrivate => "system_private",
        }
    }

    pub const fn restriction_rank(self) -> u8 {
        match self {
            Self::Public => 0,
            Self::SpectatorVisible => 1,
            Self::PartyVisible | Self::SpectatorHidden => 2,
            Self::PrivateToPlayer | Self::PrivateToGroup | Self::InvestigatorPrivate => 3,
            Self::KeeperOnly => 4,
            Self::AiInternal => 5,
            Self::SystemOnly | Self::SystemPrivate => 6,
        }
    }
}

impl TryFrom<&str> for VisibilityKind {
    type Error = TrpgError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "public" => Ok(Self::Public),
            "party_visible" => Ok(Self::PartyVisible),
            "private_to_player" => Ok(Self::PrivateToPlayer),
            "private_to_group" => Ok(Self::PrivateToGroup),
            "keeper_only" => Ok(Self::KeeperOnly),
            "investigator_private" => Ok(Self::InvestigatorPrivate),
            "ai_internal" => Ok(Self::AiInternal),
            "system_only" => Ok(Self::SystemOnly),
            "spectator_visible" => Ok(Self::SpectatorVisible),
            "spectator_hidden" => Ok(Self::SpectatorHidden),
            "system_private" => Ok(Self::SystemPrivate),
            _ => Err(TrpgError::UnknownVisibilityLabel),
        }
    }
}

/// A visibility label is a complete audience classification. Targeted enum
/// variants require an `EntityId`, so a targetless private label is not a
/// representable Rust value.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum VisibilityLabel {
    Public,
    PartyVisible,
    PrivateToPlayer(EntityId),
    PrivateToGroup(EntityId),
    KeeperOnly,
    InvestigatorPrivate(EntityId),
    AiInternal,
    SystemOnly,
    SpectatorVisible,
    SpectatorHidden,
    SystemPrivate,
}

impl VisibilityLabel {
    fn targeted(kind: VisibilityKind, subject_id: EntityId) -> Self {
        match kind {
            VisibilityKind::PrivateToPlayer => Self::PrivateToPlayer(subject_id),
            VisibilityKind::PrivateToGroup => Self::PrivateToGroup(subject_id),
            VisibilityKind::InvestigatorPrivate => Self::InvestigatorPrivate(subject_id),
            _ => unreachable!("targeted visibility kind"),
        }
    }

    pub const fn kind(&self) -> VisibilityKind {
        match self {
            Self::Public => VisibilityKind::Public,
            Self::PartyVisible => VisibilityKind::PartyVisible,
            Self::PrivateToPlayer(_) => VisibilityKind::PrivateToPlayer,
            Self::PrivateToGroup(_) => VisibilityKind::PrivateToGroup,
            Self::KeeperOnly => VisibilityKind::KeeperOnly,
            Self::InvestigatorPrivate(_) => VisibilityKind::InvestigatorPrivate,
            Self::AiInternal => VisibilityKind::AiInternal,
            Self::SystemOnly => VisibilityKind::SystemOnly,
            Self::SpectatorVisible => VisibilityKind::SpectatorVisible,
            Self::SpectatorHidden => VisibilityKind::SpectatorHidden,
            Self::SystemPrivate => VisibilityKind::SystemPrivate,
        }
    }

    pub fn subject_id(&self) -> Option<&EntityId> {
        match self {
            Self::PrivateToPlayer(subject_id)
            | Self::PrivateToGroup(subject_id)
            | Self::InvestigatorPrivate(subject_id) => Some(subject_id),
            _ => None,
        }
    }

    pub const fn is_private_to_player(&self) -> bool {
        matches!(
            self,
            Self::PrivateToPlayer(_) | Self::InvestigatorPrivate(_)
        )
    }

    pub const fn is_private_to_group(&self) -> bool {
        matches!(self, Self::PrivateToGroup(_))
    }

    pub const fn is_restricted(&self) -> bool {
        matches!(
            self,
            Self::PrivateToPlayer(_)
                | Self::PrivateToGroup(_)
                | Self::KeeperOnly
                | Self::InvestigatorPrivate(_)
                | Self::AiInternal
                | Self::SystemOnly
                | Self::SpectatorHidden
                | Self::SystemPrivate
        )
    }

    pub fn as_str(&self) -> &'static str {
        self.kind().as_str()
    }

    /// Total ordering used only after audience semantics have been resolved.
    /// Targeted labels at rank 3 still require their `Visibility` subject to
    /// determine whether two scopes are comparable.
    pub const fn restriction_rank(&self) -> u8 {
        self.kind().restriction_rank()
    }

    /// Conservative label-only merge for schema and policy surfaces that do
    /// not carry a targeted subject. Incomparable private label kinds collapse
    /// to Keeper-only instead of depending on input order.
    pub fn conservative_merge(&self, other: &Self) -> Self {
        intersect_visibility_labels(self, other)
    }
}

impl TryFrom<&str> for VisibilityLabel {
    type Error = TrpgError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let kind = VisibilityKind::try_from(value)?;
        match kind {
            VisibilityKind::Public => Ok(Self::Public),
            VisibilityKind::PartyVisible => Ok(Self::PartyVisible),
            VisibilityKind::KeeperOnly => Ok(Self::KeeperOnly),
            VisibilityKind::AiInternal => Ok(Self::AiInternal),
            VisibilityKind::SystemOnly => Ok(Self::SystemOnly),
            VisibilityKind::SpectatorVisible => Ok(Self::SpectatorVisible),
            VisibilityKind::SpectatorHidden => Ok(Self::SpectatorHidden),
            VisibilityKind::SystemPrivate => Ok(Self::SystemPrivate),
            VisibilityKind::PrivateToPlayer
            | VisibilityKind::PrivateToGroup
            | VisibilityKind::InvestigatorPrivate => Err(TrpgError::VisibilityDenied),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Visibility {
    label: VisibilityLabel,
}

impl Visibility {
    pub fn new(label: VisibilityLabel) -> Self {
        Self { label }
    }

    pub fn private_to_player(player_id: EntityId) -> Self {
        Self {
            label: VisibilityLabel::PrivateToPlayer(player_id),
        }
    }

    pub fn private_to_group(group_id: EntityId) -> Self {
        Self {
            label: VisibilityLabel::PrivateToGroup(group_id),
        }
    }

    pub fn investigator_private(player_id: EntityId) -> Self {
        Self {
            label: VisibilityLabel::InvestigatorPrivate(player_id),
        }
    }

    pub fn try_from_parts(label: &str, subject_id: Option<&str>) -> KernelResult<Self> {
        let kind = VisibilityKind::try_from(label)?;
        match (kind.requires_subject(), subject_id) {
            (true, Some(subject)) => {
                let subject = EntityId::new(subject)?;
                Ok(match kind {
                    VisibilityKind::PrivateToPlayer => Self::private_to_player(subject),
                    VisibilityKind::PrivateToGroup => Self::private_to_group(subject),
                    VisibilityKind::InvestigatorPrivate => Self::investigator_private(subject),
                    _ => unreachable!("subject-bearing visibility kind"),
                })
            }
            (false, None) => Ok(Self::new(VisibilityLabel::try_from(label)?)),
            _ => Err(TrpgError::VisibilityDenied),
        }
    }

    pub fn label(&self) -> &VisibilityLabel {
        &self.label
    }

    pub fn player_id(&self) -> Option<&EntityId> {
        self.label
            .is_private_to_player()
            .then_some(self.label.subject_id())
            .flatten()
    }

    pub fn group_id(&self) -> Option<&EntityId> {
        self.label
            .is_private_to_group()
            .then_some(self.label.subject_id())
            .flatten()
    }

    /// The audience identity carried by targeted visibility labels.
    /// Callers that persist or hash visibility metadata must use this method
    /// rather than assuming that every target is a player.
    pub fn subject_id(&self) -> Option<&EntityId> {
        self.label.subject_id()
    }

    pub fn is_well_formed(&self) -> bool {
        true
    }

    pub fn can_view(&self, principal: &PrincipalScope) -> bool {
        match self.label.kind() {
            VisibilityKind::Public => true,
            VisibilityKind::PartyVisible | VisibilityKind::SpectatorHidden => {
                principal.is_party_audience()
            }
            VisibilityKind::KeeperOnly => principal.is_keeper() || principal.is_system(),
            VisibilityKind::PrivateToPlayer | VisibilityKind::InvestigatorPrivate => {
                principal.is_keeper()
                    || principal.is_system()
                    || self
                        .subject_id()
                        .is_some_and(|subject| principal.matches_player(subject))
            }
            VisibilityKind::PrivateToGroup => {
                principal.is_keeper()
                    || principal.is_system()
                    || self
                        .subject_id()
                        .is_some_and(|subject| principal.matches_group(subject))
            }
            VisibilityKind::SpectatorVisible => {
                principal.is_spectator() || principal.is_party_audience()
            }
            VisibilityKind::AiInternal => principal.is_ai_runtime() || principal.is_system(),
            VisibilityKind::SystemOnly | VisibilityKind::SystemPrivate => principal.is_system(),
        }
    }

    /// Computes the audience intersection of two source values. The result is
    /// never broader than either source. Incomparable private targets collapse
    /// to keeper/system or system-only rather than depending on input order.
    pub fn intersection(&self, other: &Self) -> Self {
        Self {
            label: intersect_visibility_labels(&self.label, &other.label),
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct VisibilityWireValue {
    label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    subject_id: Option<String>,
}

impl Serialize for Visibility {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        VisibilityWireValue {
            label: self.label.as_str().to_owned(),
            subject_id: self.subject_id().map(ToString::to_string),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Visibility {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = VisibilityWireValue::deserialize(deserializer)?;
        Self::try_from_parts(&value.label, value.subject_id.as_deref())
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PrincipalCapability {
    PartyMember,
    Keeper,
    Spectator,
    AiRuntime,
    System,
}

/// Lossless authenticated principal claims. Unlike the compatibility enum
/// variants below, this value can simultaneously represent one user/player,
/// multiple groups and characters, spectator status, and trusted workload
/// capabilities.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrincipalClaims {
    user_id: EntityId,
    player_id: Option<EntityId>,
    group_ids: Vec<EntityId>,
    character_ids: Vec<EntityId>,
    capabilities: Vec<PrincipalCapability>,
}

impl PrincipalClaims {
    pub fn new(user_id: impl Into<String>) -> KernelResult<Self> {
        Ok(Self {
            user_id: EntityId::new(user_id)?,
            player_id: None,
            group_ids: Vec::new(),
            character_ids: Vec::new(),
            capabilities: Vec::new(),
        })
    }

    pub fn with_player(mut self, player_id: impl Into<String>) -> KernelResult<Self> {
        self.player_id = Some(EntityId::new(player_id)?);
        Ok(self)
    }

    pub fn with_group(mut self, group_id: impl Into<String>) -> KernelResult<Self> {
        push_unique(&mut self.group_ids, EntityId::new(group_id)?);
        Ok(self)
    }

    pub fn with_character(mut self, character_id: impl Into<String>) -> KernelResult<Self> {
        push_unique(&mut self.character_ids, EntityId::new(character_id)?);
        Ok(self)
    }

    pub fn with_capability(mut self, capability: PrincipalCapability) -> Self {
        if !self.capabilities.contains(&capability) {
            self.capabilities.push(capability);
        }
        self
    }

    pub fn user_id(&self) -> &EntityId {
        &self.user_id
    }

    pub fn player_id(&self) -> Option<&EntityId> {
        self.player_id.as_ref()
    }

    pub fn group_ids(&self) -> &[EntityId] {
        &self.group_ids
    }

    pub fn character_ids(&self) -> &[EntityId] {
        &self.character_ids
    }

    pub fn has_capability(&self, capability: PrincipalCapability) -> bool {
        self.capabilities.contains(&capability)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PrincipalScope {
    Public,
    PartyMember,
    Keeper,
    Player(EntityId),
    GroupMember(EntityId),
    Spectator,
    System,
    Claims(PrincipalClaims),
}

impl PrincipalScope {
    fn is_system(&self) -> bool {
        matches!(self, Self::System)
            || matches!(self, Self::Claims(claims) if claims.has_capability(PrincipalCapability::System))
    }

    fn is_keeper(&self) -> bool {
        matches!(self, Self::Keeper)
            || matches!(self, Self::Claims(claims) if claims.has_capability(PrincipalCapability::Keeper))
    }

    fn is_ai_runtime(&self) -> bool {
        matches!(self, Self::Claims(claims) if claims.has_capability(PrincipalCapability::AiRuntime))
    }

    fn is_spectator(&self) -> bool {
        matches!(self, Self::Spectator)
            || matches!(self, Self::Claims(claims) if claims.has_capability(PrincipalCapability::Spectator))
    }

    fn is_party_audience(&self) -> bool {
        matches!(
            self,
            Self::PartyMember
                | Self::Player(_)
                | Self::GroupMember(_)
                | Self::Keeper
                | Self::System
        ) || matches!(self, Self::Claims(claims) if claims.player_id.is_some()
            || !claims.group_ids.is_empty()
            || claims.has_capability(PrincipalCapability::PartyMember)
            || claims.has_capability(PrincipalCapability::Keeper)
            || claims.has_capability(PrincipalCapability::System))
    }

    fn matches_player(&self, subject: &EntityId) -> bool {
        matches!(self, Self::Player(player_id) if player_id == subject)
            || matches!(self, Self::Claims(claims) if claims.player_id.as_ref() == Some(subject))
    }

    fn matches_group(&self, subject: &EntityId) -> bool {
        matches!(self, Self::GroupMember(group_id) if group_id == subject)
            || matches!(self, Self::Claims(claims) if claims.group_ids.contains(subject))
    }
}

fn push_unique(values: &mut Vec<EntityId>, value: EntityId) {
    if !values.contains(&value) {
        values.push(value);
    }
}

fn intersect_visibility_labels(left: &VisibilityLabel, right: &VisibilityLabel) -> VisibilityLabel {
    use VisibilityKind::*;

    if left == right {
        return left.clone();
    }
    match (left.kind(), right.kind()) {
        (Public, _) => right.clone(),
        (_, Public) => left.clone(),
        (SystemPrivate, _) | (_, SystemPrivate) => VisibilityLabel::SystemPrivate,
        (SystemOnly, _) | (_, SystemOnly) => VisibilityLabel::SystemOnly,
        (AiInternal, AiInternal) => VisibilityLabel::AiInternal,
        (AiInternal, _) | (_, AiInternal) => VisibilityLabel::SystemOnly,
        (KeeperOnly, _) | (_, KeeperOnly) => VisibilityLabel::KeeperOnly,
        (PrivateToPlayer | InvestigatorPrivate, PrivateToPlayer | InvestigatorPrivate)
            if left.subject_id() == right.subject_id() =>
        {
            VisibilityLabel::targeted(
                if matches!(left.kind(), InvestigatorPrivate)
                    || matches!(right.kind(), InvestigatorPrivate)
                {
                    InvestigatorPrivate
                } else {
                    PrivateToPlayer
                },
                left.subject_id()
                    .expect("targeted label has a subject")
                    .clone(),
            )
        }
        (PrivateToGroup, PrivateToGroup) if left.subject_id() == right.subject_id() => left.clone(),
        (
            PrivateToPlayer | PrivateToGroup | InvestigatorPrivate,
            PrivateToPlayer | PrivateToGroup | InvestigatorPrivate,
        ) => VisibilityLabel::KeeperOnly,
        (PrivateToPlayer | PrivateToGroup | InvestigatorPrivate, _)
        | (_, PrivateToPlayer | PrivateToGroup | InvestigatorPrivate) => {
            if left.kind().requires_subject() {
                left.clone()
            } else {
                right.clone()
            }
        }
        (SpectatorVisible, _) => right.clone(),
        (_, SpectatorVisible) => left.clone(),
        (PartyVisible, SpectatorHidden) | (SpectatorHidden, PartyVisible) => {
            VisibilityLabel::SpectatorHidden
        }
        (PartyVisible, PartyVisible) => VisibilityLabel::PartyVisible,
        (SpectatorHidden, SpectatorHidden) => VisibilityLabel::SpectatorHidden,
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum ProvenanceKind {
    UserStatement,
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

    /// Canonical actor role derived from the authenticated principal. It must
    /// not be replaced with the role of a separate policy approver.
    pub fn canonical_role_name(&self) -> &'static str {
        actor_role_integrity_name(&self.role)
    }

    /// Lossless canonical origin of the authenticated principal.
    pub fn canonical_origin_wire(&self) -> EventActorOriginWire {
        event_actor_origin_wire(&self.origin)
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
    authority_mode: AuthorityMode,
    contract_version: u64,
}

impl AuthorityBinding {
    pub fn new(
        contract_id: impl Into<String>,
        authority_owner: impl Into<String>,
        authority_mode: AuthorityMode,
        contract_version: u64,
    ) -> KernelResult<Self> {
        if contract_version == 0 {
            return Err(TrpgError::AuthorityContractVersionConflict);
        }
        Ok(Self {
            contract_id: EntityId::new(contract_id)?,
            authority_owner: EntityId::new(authority_owner)?,
            authority_mode,
            contract_version,
        })
    }

    pub fn contract_id(&self) -> &EntityId {
        &self.contract_id
    }

    pub fn authority_owner(&self) -> &EntityId {
        &self.authority_owner
    }

    pub fn authority_mode(&self) -> &AuthorityMode {
        &self.authority_mode
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

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
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
            self.mode.clone(),
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
        if context.authority().authority_mode() != &self.mode {
            return Err(TrpgError::AuthorityViolation);
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

/// Persistence-neutral request used by runtime and agent layers to hand a
/// fully authorized formal event batch to the canonical Event Store adapter.
/// The port lives in the shared kernel so production composition roots can
/// inject a durable adapter without reversing crate dependency direction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanonicalCommitEvent {
    pub event_type: String,
    pub payload_json: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanonicalPolicyAudit {
    pub actor_id: String,
    pub actor_origin: String,
    pub authentication_reference: String,
    pub resource_type: String,
    pub resource_id: String,
    pub action: String,
    pub requested_role: String,
    pub openfga_decision_id: String,
    pub openfga_policy_revision: String,
    pub opa_decision_id: String,
    pub opa_policy_revision: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanonicalCommitRequest {
    pub commit_id: String,
    pub campaign_id: String,
    pub idempotency_key: String,
    pub expected_version: u64,
    pub command_id: String,
    pub authenticated_actor_id: String,
    pub authenticated_actor_role: String,
    pub authenticated_actor_origin: EventActorOriginWire,
    pub authority_mode: String,
    pub authority_contract_version: u64,
    pub authority_contract_id: String,
    pub authority_owner: String,
    pub visibility_label: String,
    pub visibility_subject: String,
    /// Independent personal-data owner for crypto-erasure and data-subject
    /// workflows. This is deliberately not derived from the visibility
    /// audience: public, party, keeper, and group-visible records can still
    /// contain one person's data.
    pub data_subject_id: String,
    pub provenance_kind: String,
    pub provenance_reference: String,
    pub provenance_recorded_by: String,
    pub correlation_id: String,
    pub causation_id: String,
    pub trace_id: String,
    pub events: Vec<CanonicalCommitEvent>,
    pub audit: CanonicalPolicyAudit,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanonicalCommittedEvent {
    pub sequence: u64,
    pub stream_version: u64,
    pub event_type: String,
    pub payload_json: String,
    pub command_id: String,
    pub idempotency_key: String,
    pub occurred_at_unix_ms: u64,
    /// Store-generated HMAC for this exact canonical event. Consumers that
    /// bind a secondary workflow to an event must use this value rather than
    /// synthesizing a process-local digest.
    pub event_integrity_hash: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanonicalCommitReceipt {
    pub first_stream_version: u64,
    pub last_stream_version: u64,
    /// Exact durable identities; consumers must not invent local replacements.
    pub events: Vec<CanonicalCommittedEvent>,
}

pub trait CanonicalCommitPort: fmt::Debug + Send + Sync {
    /// Atomically validates the campaign stream version and idempotency key,
    /// persists the complete formal batch, and returns its durable range.
    fn commit(&self, request: &CanonicalCommitRequest) -> KernelResult<CanonicalCommitReceipt>;

    /// Revalidates an exact receipt against the port's trusted canonical
    /// custody. Durable adapters must prove the keyed primary/audit chains and
    /// external witness binding; callers must never accept a hash-shaped
    /// string as equivalent evidence.
    fn verify_receipt(
        &self,
        request: &CanonicalCommitRequest,
        receipt: &CanonicalCommitReceipt,
    ) -> KernelResult<()>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventEnvelope<P> {
    pub sequence: u64,
    pub stream_id: EntityId,
    pub stream_version: u64,
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
    recorded_payload: P,
    integrity_hash: [u8; 32],
}

pub const EVENT_ENVELOPE_WIRE_SCHEMA_VERSION: u16 = 2;

/// Stable, versioned representation used at persistence and transport
/// boundaries. Domain-only private fields stay inside `EventEnvelope`, while
/// every authoritative classification and provenance field is explicit here.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventEnvelopeWire<P> {
    pub schema_version: u16,
    pub event_schema_version: u32,
    pub sequence: u64,
    pub stream_id: String,
    pub stream_version: u64,
    pub event_type: String,
    pub campaign_id: String,
    pub authenticated_actor_id: String,
    pub authenticated_actor_role: String,
    pub authenticated_actor_origin: EventActorOriginWire,
    pub resource_campaign_id: String,
    pub resource_type: String,
    pub resource_id: String,
    pub authority_contract_id: String,
    pub authority_owner: String,
    pub command_id: String,
    pub idempotency_key: String,
    pub authority_contract_version: u64,
    pub visibility_label: String,
    pub visibility_subject: Option<String>,
    pub provenance_kind: String,
    pub provenance_reference: String,
    pub provenance_recorded_by: String,
    pub correlation_id: String,
    pub causation_id: String,
    pub trace_id: String,
    pub occurred_at_unix_ms: u64,
    pub payload: P,
    pub request_hash_source: String,
    pub integrity_status: String,
    /// Historical imports can predate the HMAC domain. Absence is explicit
    /// and must be interpreted together with the persisted integrity status;
    /// callers must never synthesize a hash for those records.
    pub integrity_hash: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum EventActorOriginWire {
    UserSession {
        session_id: String,
    },
    Workload {
        role: String,
    },
    AgentRun {
        run_id: String,
        class: String,
        campaign_id: String,
    },
}

impl<P: Serialize> EventEnvelopeWire<P> {
    pub fn to_canonical_json(&self) -> KernelResult<String> {
        let value = serde_json::to_value(self).map_err(|_| TrpgError::AuditIntegrityViolation)?;
        serde_json::to_string(&value).map_err(|_| TrpgError::AuditIntegrityViolation)
    }
}

impl<P: PartialEq + Serialize> EventEnvelope<P> {
    pub fn verify_recorded_integrity(&self) -> KernelResult<()> {
        if self.payload != self.recorded_payload
            || self.integrity_hash != event_integrity_hash(self)?
        {
            return Err(TrpgError::PolicyEvidenceUntrusted);
        }
        Ok(())
    }
}

impl<P: Clone + PartialEq + Serialize> EventEnvelope<P> {
    pub fn to_canonical_wire(&self) -> EventEnvelopeWire<P> {
        EventEnvelopeWire {
            schema_version: EVENT_ENVELOPE_WIRE_SCHEMA_VERSION,
            event_schema_version: 1,
            sequence: self.sequence,
            stream_id: self.stream_id.to_string(),
            stream_version: self.stream_version,
            event_type: self.event_type.to_owned(),
            campaign_id: self.campaign_id.to_string(),
            authenticated_actor_id: self.authenticated_actor.id().to_string(),
            authenticated_actor_role: actor_role_integrity_name(self.authenticated_actor.role())
                .to_owned(),
            authenticated_actor_origin: event_actor_origin_wire(self.authenticated_actor.origin()),
            resource_campaign_id: self.resource.campaign_id().to_string(),
            resource_type: self.resource.resource_type().to_string(),
            resource_id: self.resource.resource_id().to_string(),
            authority_contract_id: self.authority_contract_id.to_string(),
            authority_owner: self.authority_owner.to_string(),
            command_id: self.command_id.to_string(),
            idempotency_key: self.idempotency_key.clone(),
            authority_contract_version: self.authority_contract_version,
            visibility_label: self.visibility.label().as_str().to_owned(),
            visibility_subject: self.visibility.subject_id().map(ToString::to_string),
            provenance_kind: provenance_kind_integrity_name(&self.fact_provenance.kind).to_owned(),
            provenance_reference: self.fact_provenance.reference.to_string(),
            provenance_recorded_by: self.fact_provenance.recorded_by.to_string(),
            correlation_id: self.correlation_id.to_string(),
            causation_id: self.causation_id.to_string(),
            trace_id: self.trace_id.to_string(),
            occurred_at_unix_ms: self.occurred_at_unix_ms,
            payload: self.payload.clone(),
            request_hash_source: "shared_kernel_append".to_owned(),
            integrity_status: "verified_sha256".to_owned(),
            integrity_hash: Some(format!("sha256:{}", hex_lower(&self.integrity_hash))),
        }
    }

    pub fn to_canonical_json(&self) -> KernelResult<String> {
        self.to_canonical_wire().to_canonical_json()
    }
}

#[derive(Clone, Debug)]
pub struct EventStore<P> {
    stream_base_versions: HashMap<(EntityId, EntityId), u64>,
    events: Vec<EventEnvelope<P>>,
    idempotency_index: HashMap<(EntityId, EntityId, String), IdempotencyRecord>,
}

#[derive(Clone, Debug)]
struct IdempotencyRecord {
    request_hash: [u8; 32],
    event_index: usize,
}

impl<P> Default for EventStore<P> {
    fn default() -> Self {
        Self {
            stream_base_versions: HashMap::new(),
            events: Vec::new(),
            idempotency_index: HashMap::new(),
        }
    }
}

impl<P: Clone + PartialEq + Serialize> EventStore<P> {
    /// Performs every deterministic append guard without mutating the store.
    /// Callers that must persist an idempotent side effect before the event can
    /// use this while holding their exclusive `&mut EventStore` borrow; the
    /// subsequent append cannot encounter a stale version introduced by a
    /// concurrent in-process writer.
    pub fn validate_append<T>(
        &self,
        command: &CommandEnvelope<T>,
        event_type: &'static str,
        payload: &P,
    ) -> KernelResult<()> {
        validate_command_envelope(command)?;

        let campaign_id = command
            .authenticated_context()
            .resource()
            .campaign_id()
            .clone();
        let stream_id = command
            .authenticated_context()
            .resource()
            .resource_id()
            .clone();
        let idempotency_scope = (
            campaign_id.clone(),
            stream_id.clone(),
            command.idempotency_key.clone(),
        );
        let request_hash = append_request_hash(command, event_type, payload)?;
        if let Some(existing) = self.idempotency_index.get(&idempotency_scope) {
            return if existing.request_hash == request_hash {
                Ok(())
            } else {
                Err(TrpgError::DuplicateCommand)
            };
        }

        let actual_version = self.current_stream_version(&campaign_id, &stream_id);
        if command.expected_version != actual_version {
            return Err(TrpgError::ExpectedVersionConflict {
                expected: command.expected_version,
                actual: actual_version,
            });
        }
        Ok(())
    }

    pub fn append<T>(
        &mut self,
        command: &CommandEnvelope<T>,
        event_type: &'static str,
        payload: P,
    ) -> KernelResult<EventEnvelope<P>> {
        self.validate_append(command, event_type, &payload)?;

        let campaign_id = command
            .authenticated_context()
            .resource()
            .campaign_id()
            .clone();
        let stream_id = command
            .authenticated_context()
            .resource()
            .resource_id()
            .clone();
        let idempotency_scope = (
            campaign_id.clone(),
            stream_id.clone(),
            command.idempotency_key.clone(),
        );
        let request_hash = append_request_hash(command, event_type, &payload)?;
        if let Some(existing) = self.idempotency_index.get(&idempotency_scope) {
            if existing.request_hash == request_hash {
                return Ok(self.events[existing.event_index].clone());
            }
            return Err(TrpgError::DuplicateCommand);
        }

        let actual_version = self.current_stream_version(&campaign_id, &stream_id);

        let mut event = EventEnvelope {
            sequence: self
                .events
                .iter()
                .map(|event| event.sequence)
                .max()
                .unwrap_or(0)
                .checked_add(1)
                .ok_or(TrpgError::AuditIntegrityViolation)?,
            stream_id,
            stream_version: actual_version + 1,
            event_type,
            campaign_id,
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
            payload: payload.clone(),
            recorded_payload: payload,
            integrity_hash: [0_u8; 32],
        };
        event.integrity_hash = event_integrity_hash(&event)?;

        self.idempotency_index.insert(
            idempotency_scope,
            IdempotencyRecord {
                request_hash,
                event_index: self.events.len(),
            },
        );
        self.events.push(event.clone());

        Ok(event)
    }

    /// Materializes a canonical event using only the identity returned by the
    /// durable adapter. This closes cold-restart sequence/timestamp forgery.
    pub fn record_canonical<T>(
        &mut self,
        command: &CommandEnvelope<T>,
        event_type: &'static str,
        payload: P,
        durable: &CanonicalCommittedEvent,
    ) -> KernelResult<EventEnvelope<P>> {
        validate_command_envelope(command)?;
        if durable.sequence == 0
            || durable.occurred_at_unix_ms == 0
            || durable.stream_version
                != command
                    .expected_version
                    .checked_add(1)
                    .ok_or(TrpgError::AuditIntegrityViolation)?
            || durable.event_type != event_type
            || durable.command_id.trim().is_empty()
            || durable.idempotency_key.trim().is_empty()
            || !is_canonical_hmac(&durable.event_integrity_hash)
        {
            return Err(TrpgError::AuditIntegrityViolation);
        }
        let local_payload =
            serde_json::to_value(&payload).map_err(|_| TrpgError::AuditIntegrityViolation)?;
        let durable_payload: serde_json::Value = serde_json::from_str(&durable.payload_json)
            .map_err(|_| TrpgError::AuditIntegrityViolation)?;
        if local_payload != durable_payload {
            return Err(TrpgError::AuditIntegrityViolation);
        }

        let campaign_id = command
            .authenticated_context()
            .resource()
            .campaign_id()
            .clone();
        let stream_id = command
            .authenticated_context()
            .resource()
            .resource_id()
            .clone();
        let idempotency_scope = (
            campaign_id.clone(),
            stream_id.clone(),
            command.idempotency_key.clone(),
        );
        let request_hash = append_request_hash(command, event_type, &payload)?;
        if let Some(existing) = self.idempotency_index.get(&idempotency_scope) {
            let event = &self.events[existing.event_index];
            if existing.request_hash == request_hash
                && event.sequence == durable.sequence
                && event.stream_version == durable.stream_version
                && event.command_id.as_str() == durable.command_id
                && event.idempotency_key == durable.idempotency_key
                && event.occurred_at_unix_ms == durable.occurred_at_unix_ms
            {
                return Ok(event.clone());
            }
            return Err(TrpgError::DuplicateCommand);
        }
        if self.events.iter().any(|event| {
            event.sequence == durable.sequence
                || (event.campaign_id == campaign_id
                    && event.stream_id == stream_id
                    && event.stream_version == durable.stream_version)
        }) {
            return Err(TrpgError::AuditIntegrityViolation);
        }

        let mut event = EventEnvelope {
            sequence: durable.sequence,
            stream_id: stream_id.clone(),
            stream_version: durable.stream_version,
            event_type,
            campaign_id: campaign_id.clone(),
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
            command_id: EntityId::new(durable.command_id.clone())?,
            idempotency_key: durable.idempotency_key.clone(),
            authority_contract_version: command.authority_contract_version,
            visibility: command.visibility.clone(),
            fact_provenance: command.fact_provenance.clone(),
            correlation_id: command.correlation_id.clone(),
            causation_id: command.causation_id.clone(),
            trace_id: command.authenticated_context().trace_id().clone(),
            occurred_at_unix_ms: durable.occurred_at_unix_ms,
            payload: payload.clone(),
            recorded_payload: payload,
            integrity_hash: [0_u8; 32],
        };
        event.integrity_hash = event_integrity_hash(&event)?;
        self.idempotency_index.insert(
            idempotency_scope,
            IdempotencyRecord {
                request_hash,
                event_index: self.events.len(),
            },
        );
        self.events.push(event.clone());
        self.stream_base_versions
            .entry((campaign_id, stream_id))
            .and_modify(|version| *version = (*version).max(durable.stream_version))
            .or_insert(durable.stream_version);
        Ok(event)
    }
}

fn is_canonical_hmac(value: &str) -> bool {
    const PREFIX: &str = "hmac-sha256:";
    value.len() == PREFIX.len() + 64
        && value.starts_with(PREFIX)
        && value[PREFIX.len()..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

impl<P> EventStore<P> {
    pub fn with_stream_base_version_for(
        campaign_id: EntityId,
        stream_id: EntityId,
        stream_base_version: u64,
    ) -> Self {
        let mut store = Self::default();
        store
            .stream_base_versions
            .insert((campaign_id, stream_id), stream_base_version);
        store
    }

    pub fn events(&self) -> &[EventEnvelope<P>] {
        &self.events
    }

    pub fn current_stream_version(&self, campaign_id: &EntityId, stream_id: &EntityId) -> u64 {
        let seeded = self
            .stream_base_versions
            .get(&(campaign_id.clone(), stream_id.clone()))
            .copied()
            .unwrap_or(0);
        self.events
            .iter()
            .filter(|event| &event.campaign_id == campaign_id && &event.stream_id == stream_id)
            .map(|event| event.stream_version)
            .max()
            .unwrap_or(seeded)
            .max(seeded)
    }
}

fn append_request_hash<T, P: Clone + Serialize>(
    command: &CommandEnvelope<T>,
    event_type: &'static str,
    payload: &P,
) -> KernelResult<[u8; 32]> {
    let mut proposed = EventEnvelope {
        sequence: 0,
        stream_id: command
            .authenticated_context()
            .resource()
            .resource_id()
            .clone(),
        stream_version: command.expected_version.saturating_add(1),
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
        occurred_at_unix_ms: 0,
        payload: payload.clone(),
        recorded_payload: payload.clone(),
        integrity_hash: [0_u8; 32],
    };
    proposed.integrity_hash = event_integrity_hash(&proposed)?;
    Ok(proposed.integrity_hash)
}

impl<P: Clone> EventStore<P> {
    /// Compatibility replay for single-campaign in-memory stores. A store
    /// containing more than one campaign fails closed; callers that own an
    /// authenticated campaign scope must use `replay_visible_in_campaign`.
    pub fn replay_visible(&self, principal: &PrincipalScope) -> Vec<EventEnvelope<P>> {
        let Some(campaign_id) = self.events.first().map(|event| &event.campaign_id) else {
            return Vec::new();
        };
        if self
            .events
            .iter()
            .any(|event| &event.campaign_id != campaign_id)
        {
            return Vec::new();
        }
        self.replay_visible_in_campaign(campaign_id, principal)
    }

    pub fn replay_visible_in_campaign(
        &self,
        campaign_id: &EntityId,
        principal: &PrincipalScope,
    ) -> Vec<EventEnvelope<P>> {
        self.events
            .iter()
            .filter(|event| {
                &event.campaign_id == campaign_id && event.visibility.can_view(principal)
            })
            .cloned()
            .collect()
    }
}

fn event_integrity_hash<P: Serialize>(event: &EventEnvelope<P>) -> KernelResult<[u8; 32]> {
    let mut digest = Sha256::new();
    hash_integrity_field(&mut digest, 1, b"trpg-event-integrity-v4");
    hash_integrity_field(&mut digest, 2, &event.sequence.to_be_bytes());
    hash_integrity_field(&mut digest, 3, event.event_type.as_bytes());
    hash_integrity_field(&mut digest, 4, event.campaign_id.as_str().as_bytes());
    hash_integrity_field(
        &mut digest,
        5,
        event.authenticated_actor.id().as_str().as_bytes(),
    );
    hash_integrity_field(
        &mut digest,
        6,
        actor_role_integrity_name(event.authenticated_actor.role()).as_bytes(),
    );
    hash_actor_origin(&mut digest, event.authenticated_actor.origin());
    hash_integrity_field(
        &mut digest,
        11,
        event.resource.campaign_id().as_str().as_bytes(),
    );
    hash_integrity_field(
        &mut digest,
        12,
        event.resource.resource_type().as_str().as_bytes(),
    );
    hash_integrity_field(
        &mut digest,
        13,
        event.resource.resource_id().as_str().as_bytes(),
    );
    hash_integrity_field(
        &mut digest,
        14,
        event.authority_contract_id.as_str().as_bytes(),
    );
    hash_integrity_field(&mut digest, 15, event.authority_owner.as_str().as_bytes());
    hash_integrity_field(&mut digest, 16, event.command_id.as_str().as_bytes());
    hash_integrity_field(&mut digest, 17, event.idempotency_key.as_bytes());
    hash_integrity_field(
        &mut digest,
        18,
        &event.authority_contract_version.to_be_bytes(),
    );
    hash_integrity_field(
        &mut digest,
        19,
        event.visibility.label().as_str().as_bytes(),
    );
    hash_integrity_field(
        &mut digest,
        20,
        event
            .visibility
            .subject_id()
            .map(EntityId::as_str)
            .unwrap_or_default()
            .as_bytes(),
    );
    hash_integrity_field(
        &mut digest,
        21,
        provenance_kind_integrity_name(&event.fact_provenance.kind).as_bytes(),
    );
    hash_integrity_field(
        &mut digest,
        22,
        event.fact_provenance.reference.as_str().as_bytes(),
    );
    hash_integrity_field(
        &mut digest,
        23,
        event.fact_provenance.recorded_by.as_str().as_bytes(),
    );
    hash_integrity_field(&mut digest, 24, event.correlation_id.as_str().as_bytes());
    hash_integrity_field(&mut digest, 25, event.causation_id.as_str().as_bytes());
    hash_integrity_field(&mut digest, 26, event.trace_id.as_str().as_bytes());
    hash_integrity_field(&mut digest, 27, &event.occurred_at_unix_ms.to_be_bytes());
    hash_integrity_field(&mut digest, 28, &canonical_json_bytes(&event.payload)?);
    hash_integrity_field(&mut digest, 29, event.stream_id.as_str().as_bytes());
    hash_integrity_field(&mut digest, 30, &event.stream_version.to_be_bytes());
    Ok(digest.finalize().into())
}

fn canonical_json_bytes<T: Serialize>(value: &T) -> KernelResult<Vec<u8>> {
    let mut value = serde_json::to_value(value).map_err(|_| TrpgError::PolicyEvidenceUntrusted)?;
    canonicalize_json_value(&mut value);
    serde_json::to_vec(&value).map_err(|_| TrpgError::PolicyEvidenceUntrusted)
}

fn canonicalize_json_value(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Array(values) => {
            for value in values {
                canonicalize_json_value(value);
            }
        }
        serde_json::Value::Object(fields) => {
            for value in fields.values_mut() {
                canonicalize_json_value(value);
            }
            fields.sort_keys();
        }
        _ => {}
    }
}

fn hash_integrity_field(digest: &mut Sha256, tag: u16, bytes: &[u8]) {
    digest.update(tag.to_be_bytes());
    digest.update((bytes.len() as u64).to_be_bytes());
    digest.update(bytes);
}

fn hash_actor_origin(digest: &mut Sha256, origin: &ActorOrigin) {
    match origin {
        ActorOrigin::UserSession { session_id } => {
            hash_integrity_field(digest, 7, b"user_session");
            hash_integrity_field(digest, 8, session_id.as_str().as_bytes());
        }
        ActorOrigin::Workload { role } => {
            hash_integrity_field(digest, 7, b"workload");
            hash_integrity_field(digest, 8, workload_role_integrity_name(*role).as_bytes());
        }
        ActorOrigin::AgentRun {
            run_id,
            class,
            campaign_id,
        } => {
            hash_integrity_field(digest, 7, b"agent_run");
            hash_integrity_field(digest, 8, run_id.as_str().as_bytes());
            hash_integrity_field(digest, 9, agent_class_integrity_name(*class).as_bytes());
            hash_integrity_field(digest, 10, campaign_id.as_str().as_bytes());
        }
    }
}

fn event_actor_origin_wire(origin: &ActorOrigin) -> EventActorOriginWire {
    match origin {
        ActorOrigin::UserSession { session_id } => EventActorOriginWire::UserSession {
            session_id: session_id.to_string(),
        },
        ActorOrigin::Workload { role } => EventActorOriginWire::Workload {
            role: workload_role_integrity_name(*role).to_owned(),
        },
        ActorOrigin::AgentRun {
            run_id,
            class,
            campaign_id,
        } => EventActorOriginWire::AgentRun {
            run_id: run_id.to_string(),
            class: agent_class_integrity_name(*class).to_owned(),
            campaign_id: campaign_id.to_string(),
        },
    }
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

fn actor_role_integrity_name(role: &ActorRole) -> &'static str {
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

fn workload_role_integrity_name(role: WorkloadRole) -> &'static str {
    match role {
        WorkloadRole::ApiServer => "api_server",
        WorkloadRole::RealtimeServer => "realtime_server",
        WorkloadRole::AgentWorker => "agent_worker",
        WorkloadRole::WorkflowEngine => "workflow_engine",
        WorkloadRole::RulesEngine => "rules_engine",
        WorkloadRole::AuditWriter => "audit_writer",
    }
}

fn agent_class_integrity_name(class: AgentClass) -> &'static str {
    match class {
        AgentClass::AiKeeperOrchestrator => "ai_keeper_orchestrator",
        AgentClass::KeeperCopilot => "keeper_copilot",
        AgentClass::AtmosphereWriter => "atmosphere_writer",
        AgentClass::MemoryCurator => "memory_curator",
    }
}

fn provenance_kind_integrity_name(kind: &ProvenanceKind) -> &'static str {
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
            VisibilityKind::Public.as_str(),
            VisibilityKind::PartyVisible.as_str(),
            VisibilityKind::PrivateToPlayer.as_str(),
            VisibilityKind::PrivateToGroup.as_str(),
            VisibilityKind::KeeperOnly.as_str(),
            VisibilityKind::InvestigatorPrivate.as_str(),
            VisibilityKind::AiInternal.as_str(),
            VisibilityKind::SystemOnly.as_str(),
            VisibilityKind::SpectatorVisible.as_str(),
            VisibilityKind::SpectatorHidden.as_str(),
            VisibilityKind::SystemPrivate.as_str(),
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
