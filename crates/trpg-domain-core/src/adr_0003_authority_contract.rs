use crate::authority_contract::{
    patch_locked_authority_contract, ChangePolicy, DomainAuthorityContract,
};
use crate::command_cqrs::{submit_domain_command, CommandAcceptedPayload, DomainCommandKind};
use crate::ddd::{
    AuthorityMode, CommandEnvelope, DomainError, DomainResult, EventEnvelope, EventStore,
    FactSource,
};

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
    contract.fork_for_child(child_campaign_id, child_mode, child_owner)
}

pub fn record_authority_contract_decision<T>(
    contract: &DomainAuthorityContract,
    store: &mut EventStore<CommandAcceptedPayload>,
    command: &CommandEnvelope<T>,
) -> DomainResult<EventEnvelope<CommandAcceptedPayload>> {
    validate_adr_0003_contract(contract)?;
    submit_domain_command(
        contract,
        store,
        command,
        DomainCommandKind::RecordDecision,
        FactSource::DecisionRecord,
    )
}
