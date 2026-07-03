use crate::authority_contract::DomainAuthorityContract;
use crate::ddd::{CommandEnvelope, DomainResult, EntityId};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthorityGuardDecision {
    pub accepted: bool,
    pub contract_id: EntityId,
}

pub fn guard_authority_contract<T>(
    contract: &DomainAuthorityContract,
    command: &CommandEnvelope<T>,
) -> DomainResult<AuthorityGuardDecision> {
    contract.validate_command(command)?;

    Ok(AuthorityGuardDecision {
        accepted: true,
        contract_id: contract.contract_id.clone(),
    })
}
