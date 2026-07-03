use crate::authority_contract::DomainAuthorityContract;
use crate::command_cqrs::{submit_domain_command, CommandAcceptedPayload, DomainCommandKind};
use crate::ddd::{CommandEnvelope, DomainResult, EventEnvelope, EventStore, FactSource};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DomainCommandDecision {
    pub kind: DomainCommandKind,
    pub fact_source: FactSource,
    pub event_type: &'static str,
}

impl DomainCommandDecision {
    pub fn command_accepted(kind: DomainCommandKind, fact_source: FactSource) -> Self {
        Self {
            kind,
            fact_source,
            event_type: "CommandAccepted",
        }
    }
}

pub fn decide_and_append<T>(
    contract: &DomainAuthorityContract,
    store: &mut EventStore<CommandAcceptedPayload>,
    command: &CommandEnvelope<T>,
    decision: DomainCommandDecision,
) -> DomainResult<EventEnvelope<CommandAcceptedPayload>> {
    submit_domain_command(
        contract,
        store,
        command,
        decision.kind,
        decision.fact_source,
    )
}
