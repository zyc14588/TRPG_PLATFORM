use crate::authority_contract::DomainAuthorityContract;
use crate::command_cqrs::CommandAcceptedPayload;
use crate::ddd::{CommandEnvelope, DomainResult, EventEnvelope, EventStore};
use crate::domain_model::{reject_direct_state_write, DomainModelCommand, DomainModelService};

pub fn accept_domain_model_command<T>(
    contract: &DomainAuthorityContract,
    store: &mut EventStore<CommandAcceptedPayload>,
    command: &DomainModelCommand<T>,
) -> DomainResult<EventEnvelope<CommandAcceptedPayload>> {
    DomainModelService::accept(contract, store, command)
}

pub fn reject_ungoverned_domain_write<T>(command: &CommandEnvelope<T>) -> DomainResult<()> {
    reject_direct_state_write(command)
}
