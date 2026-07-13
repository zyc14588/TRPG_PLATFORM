use std::error::Error;
use std::fmt;
pub use trpg_contracts::WireErrorCode;

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
    SharedKernel(WireErrorCode),
}

impl DomainError {
    pub const fn wire_code(&self) -> WireErrorCode {
        match self {
            Self::AuthorityContractImmutable => WireErrorCode::AuthorityContractImmutable,
            Self::AuthorityViolation => WireErrorCode::AuthorityViolation,
            Self::InvalidConfirmedFactSource => WireErrorCode::InvalidConfirmedFactSource,
            Self::MissingCommandMetadata => WireErrorCode::MissingCommandMetadata,
            Self::DuplicateCommand => WireErrorCode::DuplicateCommand,
            Self::ExpectedVersionConflict { .. } => WireErrorCode::ExpectedVersionConflict,
            Self::VisibilityDenied => WireErrorCode::VisibilityDenied,
            Self::PolicyDenied => WireErrorCode::PolicyDenied,
            Self::SharedKernel(code) => *code,
        }
    }

    pub const fn code(&self) -> &'static str {
        self.wire_code().as_str()
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
            other => Self::SharedKernel(other.wire_code()),
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
