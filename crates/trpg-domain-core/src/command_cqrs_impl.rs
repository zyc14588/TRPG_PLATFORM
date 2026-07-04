use crate::authority_contract::DomainAuthorityContract;
use crate::command_cqrs::{CommandAcceptedPayload, DomainCommandKind};
use crate::ddd::{CommandEnvelope, DomainResult, EventEnvelope, EventStore, FactSource};
use crate::domain_command_cqrs::{decide_and_append, DomainCommandDecision};

pub fn command_accepted_decision(
    kind: DomainCommandKind,
    fact_source: FactSource,
) -> DomainCommandDecision {
    DomainCommandDecision::command_accepted(kind, fact_source)
}

pub fn commit_governed_command<T>(
    contract: &DomainAuthorityContract,
    store: &mut EventStore<CommandAcceptedPayload>,
    command: &CommandEnvelope<T>,
    decision: DomainCommandDecision,
) -> DomainResult<EventEnvelope<CommandAcceptedPayload>> {
    decide_and_append(contract, store, command, decision)
}
