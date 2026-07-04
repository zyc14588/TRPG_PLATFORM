use crate::authority_contract::DomainAuthorityContract;
use crate::command_cqrs::{submit_domain_command, CommandAcceptedPayload, DomainCommandKind};
use crate::ddd::{
    CommandEnvelope, DomainResult, EventEnvelope, EventStore, FactProvenance, FactSource,
    PrincipalScope, Visibility,
};
use crate::domain_entities_value_objects::MemoryFact;
use crate::visibility_fact_provenance::{redaction_for, DerivedObject, RedactionOutcome};

pub fn confirm_visibility_fact(
    fact_id: impl Into<String>,
    source: FactSource,
    visibility: Visibility,
    fact_provenance: FactProvenance,
) -> DomainResult<MemoryFact> {
    MemoryFact::confirmed(fact_id, source, visibility, fact_provenance)
}

pub fn redact_for_derived_object(
    visibility: &Visibility,
    object: DerivedObject,
    principal: &PrincipalScope,
) -> RedactionOutcome {
    redaction_for(visibility, object, principal)
}

pub fn append_visibility_fact_decision<T>(
    contract: &DomainAuthorityContract,
    store: &mut EventStore<CommandAcceptedPayload>,
    command: &CommandEnvelope<T>,
    fact_source: FactSource,
) -> DomainResult<EventEnvelope<CommandAcceptedPayload>> {
    submit_domain_command(
        contract,
        store,
        command,
        DomainCommandKind::PromoteFact,
        fact_source,
    )
}
