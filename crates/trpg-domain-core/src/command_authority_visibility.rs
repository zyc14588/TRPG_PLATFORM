use crate::authority_contract::DomainAuthorityContract;
use crate::ddd::{CommandEnvelope, DomainError, DomainResult, PrincipalScope, VisibilityLabel};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandAuthorityVisibility {
    pub accepted: bool,
    pub effective_label: VisibilityLabel,
}

pub fn validate_command_authority_visibility<T>(
    contract: &DomainAuthorityContract,
    command: &CommandEnvelope<T>,
    principal: &PrincipalScope,
) -> DomainResult<CommandAuthorityVisibility> {
    contract.validate_command(command)?;

    if !command.visibility.can_view(principal) {
        return Err(DomainError::VisibilityDenied);
    }

    Ok(CommandAuthorityVisibility {
        accepted: true,
        effective_label: command.visibility.label().clone(),
    })
}
