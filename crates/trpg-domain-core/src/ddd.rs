use std::error::Error;
use std::fmt;

pub use trpg_shared_kernel::shared_kernel::{
    Actor, ActorRole, AuthorityContract as KernelAuthorityContract, AuthorityMode, CommandEnvelope,
    EntityId, EventEnvelope, EventStore, FactProvenance, FormalWritePath, PrincipalScope,
    ProvenanceKind, TrpgError, Visibility, VisibilityLabel,
};

pub type DomainResult<T> = Result<T, DomainError>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DomainError {
    AuthorityContractImmutable,
    AuthorityViolation,
    InvalidConfirmedFactSource,
    MissingCommandMetadata,
    DuplicateCommand,
    ExpectedVersionConflict { expected: u64, actual: u64 },
    VisibilityDenied,
    PolicyDenied,
    SharedKernel(&'static str),
}

impl DomainError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::AuthorityContractImmutable => "AUTHORITY_CONTRACT_IMMUTABLE",
            Self::AuthorityViolation => "AUTHORITY_VIOLATION",
            Self::InvalidConfirmedFactSource => "INVALID_CONFIRMED_FACT_SOURCE",
            Self::MissingCommandMetadata => "MISSING_COMMAND_METADATA",
            Self::DuplicateCommand => "DUPLICATE_COMMAND",
            Self::ExpectedVersionConflict { .. } => "EXPECTED_VERSION_CONFLICT",
            Self::VisibilityDenied => "VISIBILITY_DENIED",
            Self::PolicyDenied => "POLICY_DENIED",
            Self::SharedKernel(code) => code,
        }
    }
}

impl fmt::Display for DomainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

impl Error for DomainError {}

impl From<TrpgError> for DomainError {
    fn from(error: TrpgError) -> Self {
        match error {
            TrpgError::AuthorityContractMutation => Self::AuthorityContractImmutable,
            TrpgError::AuthorityViolation => Self::AuthorityViolation,
            TrpgError::DirectAgentStateWrite | TrpgError::PolicyDenied => Self::PolicyDenied,
            TrpgError::DuplicateCommand => Self::DuplicateCommand,
            TrpgError::ExpectedVersionConflict { expected, actual } => {
                Self::ExpectedVersionConflict { expected, actual }
            }
            TrpgError::VisibilityDenied => Self::VisibilityDenied,
            other => Self::SharedKernel(other.code()),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FactSource {
    GameEvent,
    DecisionRecord,
    DiceRoll,
    CharacterSheetVersion,
    ClueRevealEvent,
    AgentDraft,
    NpcClaim,
    PlayerInference,
}

impl FactSource {
    pub fn can_be_confirmed(self) -> bool {
        matches!(
            self,
            Self::GameEvent
                | Self::DecisionRecord
                | Self::DiceRoll
                | Self::CharacterSheetVersion
                | Self::ClueRevealEvent
        )
    }
}

pub fn require_confirmable_fact_source(source: FactSource) -> DomainResult<()> {
    if source.can_be_confirmed() {
        Ok(())
    } else {
        Err(DomainError::InvalidConfirmedFactSource)
    }
}
