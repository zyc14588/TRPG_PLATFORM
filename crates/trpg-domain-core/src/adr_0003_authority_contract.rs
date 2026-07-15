use crate::authority_contract::{
    patch_locked_authority_contract, ChangePolicy, DomainAuthorityContract,
};
use crate::command_cqrs::CommandAcceptedPayload;
use crate::ddd::{
    AuthorityMode, CommandEnvelope, DomainError, DomainResult, EventEnvelope, EventStore,
};
use crate::decision_record_model::{DecisionEvidenceCatalog, DecisionRecord};

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

pub fn reject_authority_contract_update(
    contract: &DomainAuthorityContract,
    attempted_mode: AuthorityMode,
    attempted_owner: impl Into<String>,
) -> DomainResult<()> {
    patch_locked_authority_contract(contract, attempted_mode, attempted_owner)
}

pub fn fork_locked_authority_contract(
    contract: &DomainAuthorityContract,
    child_campaign_id: impl Into<String>,
    child_mode: AuthorityMode,
    child_owner: impl Into<String>,
) -> DomainResult<DomainAuthorityContract> {
    contract
        .fork_for_child(child_campaign_id, child_mode, child_owner)
        .map_err(crate::ddd::DomainError::from)
}

pub fn record_authority_contract_decision(
    contract: &DomainAuthorityContract,
    evidence: &DecisionEvidenceCatalog,
    store: &mut EventStore<CommandAcceptedPayload>,
    command: &CommandEnvelope<DecisionRecord>,
) -> DomainResult<EventEnvelope<CommandAcceptedPayload>> {
    validate_adr_0003_contract(contract)?;
    crate::authority_contract_impl::append_authority_contract_decision(
        contract, evidence, store, command,
    )
}
