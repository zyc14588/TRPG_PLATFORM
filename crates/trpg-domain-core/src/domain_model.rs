use crate::authority_contract::DomainAuthorityContract;
use crate::command_cqrs::{submit_domain_command, CommandAcceptedPayload, DomainCommandKind};
use crate::ddd::{
    CommandEnvelope, DomainError, DomainResult, EventEnvelope, EventStore, FactSource,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DomainModelCommand<T> {
    pub kind: DomainCommandKind,
    pub fact_source: FactSource,
    pub envelope: CommandEnvelope<T>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DomainModelEvent {
    pub event_type: &'static str,
    pub fact_source: FactSource,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DomainModelService;

impl DomainModelService {
    pub fn accept<T>(
        contract: &DomainAuthorityContract,
        store: &mut EventStore<CommandAcceptedPayload>,
        command: &DomainModelCommand<T>,
    ) -> DomainResult<EventEnvelope<CommandAcceptedPayload>> {
        submit_domain_command(
            contract,
            store,
            &command.envelope,
            command.kind,
            command.fact_source,
        )
    }
}

pub fn reject_direct_state_write<T>(command: &CommandEnvelope<T>) -> DomainResult<()> {
    match command.write_path {
        crate::ddd::FormalWritePath::DirectAgent | crate::ddd::FormalWritePath::DirectBusiness => {
            Err(DomainError::PolicyDenied)
        }
        _ => Ok(()),
    }
}
