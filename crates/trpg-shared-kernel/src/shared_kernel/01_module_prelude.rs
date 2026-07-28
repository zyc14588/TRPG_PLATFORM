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
