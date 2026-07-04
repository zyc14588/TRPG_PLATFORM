use crate::authority_contract::{patch_locked_authority_contract, DomainAuthorityContract};
use crate::command_cqrs::{submit_domain_command, CommandAcceptedPayload, DomainCommandKind};
use crate::ddd::{
    AuthorityMode, CommandEnvelope, DomainResult, EventEnvelope, EventStore, FactSource,
};

pub fn validate_locked_authority_command<T>(
    contract: &DomainAuthorityContract,
    command: &CommandEnvelope<T>,
) -> DomainResult<()> {
    contract.validate_command(command)
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

pub fn append_authority_contract_decision<T>(
    contract: &DomainAuthorityContract,
    store: &mut EventStore<CommandAcceptedPayload>,
    command: &CommandEnvelope<T>,
) -> DomainResult<EventEnvelope<CommandAcceptedPayload>> {
    submit_domain_command(
        contract,
        store,
        command,
        DomainCommandKind::RecordDecision,
        FactSource::DecisionRecord,
    )
}
