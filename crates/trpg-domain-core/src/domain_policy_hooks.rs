use crate::authority_contract::DomainAuthorityContract;
use crate::ddd::{CommandEnvelope, DomainError, DomainResult, PrincipalScope, Visibility};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PolicyDecision {
    Allow,
    Deny(DomainError),
}

impl PolicyDecision {
    pub fn require_allow(self) -> DomainResult<()> {
        match self {
            Self::Allow => Ok(()),
            Self::Deny(error) => Err(error),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolicyContext {
    pub principal: PrincipalScope,
    pub target_visibility: Visibility,
}

pub fn deny_by_default() -> PolicyDecision {
    PolicyDecision::Deny(DomainError::PolicyDenied)
}

pub fn allow_governed_command<T>(
    contract: &DomainAuthorityContract,
    command: &CommandEnvelope<T>,
    context: &PolicyContext,
) -> DomainResult<PolicyDecision> {
    contract.validate_command(command)?;

    if !context.target_visibility.can_view(&context.principal) {
        return Ok(PolicyDecision::Deny(DomainError::VisibilityDenied));
    }

    Ok(PolicyDecision::Allow)
}
