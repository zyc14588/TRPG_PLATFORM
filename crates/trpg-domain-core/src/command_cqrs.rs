use crate::authority_contract::DomainAuthorityContract;
use crate::command_cqrs_idempotency::append_idempotent_event;
use crate::ddd::{CommandEnvelope, DomainResult, EventEnvelope, EventStore, FactSource};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub enum DomainCommandKind {
    SubmitPlayerAction,
    RecordDecision,
    ForkCampaign,
    PromoteFact,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct CommandAcceptedPayload {
    pub kind: DomainCommandKind,
    pub fact_source: FactSource,
}

pub fn submit_domain_command<T>(
    contract: &DomainAuthorityContract,
    store: &mut EventStore<CommandAcceptedPayload>,
    command: &CommandEnvelope<T>,
    kind: DomainCommandKind,
    fact_source: FactSource,
) -> DomainResult<EventEnvelope<CommandAcceptedPayload>> {
    contract.validate_command(command)?;
    append_idempotent_event(
        store,
        command,
        "CommandAccepted",
        CommandAcceptedPayload { kind, fact_source },
    )
}
