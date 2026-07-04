use crate::authority_contract::{ChangePolicy, DomainAuthorityContract};
use crate::ddd::{DomainError, DomainResult};

pub const ADR_0003_AUTHORITY_CONTRACT: &str = "ADR-0003 authority contract immutable";

pub const ADR_0003_INVARIANTS: &[&str] = &[
    "authority_contract_locked",
    "change_policy_fork_only",
    "human_kp_ai_kp_mutually_exclusive",
    "formal_state_changes_use_event_store",
];

pub fn validate_adr_0003_contract(contract: &DomainAuthorityContract) -> DomainResult<()> {
    if contract.is_locked() && contract.change_policy() == ChangePolicy::ForkOnly {
        Ok(())
    } else {
        Err(DomainError::AuthorityContractImmutable)
    }
}
