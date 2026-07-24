use crate::authority_contract::DomainAuthorityContract;
use crate::command_cqrs::{submit_domain_command, CommandAcceptedPayload, DomainCommandKind};
use crate::ddd::{
    CommandEnvelope, DomainResult, EventEnvelope, EventStore, FactSource, PrincipalScope,
    Visibility,
};
use crate::domain_entities_value_objects::MemoryFact;
use crate::visibility_fact_provenance::{
    redaction_for, CommittedFactEvidence, DerivedObject, RedactionOutcome,
};

pub fn confirm_visibility_fact(
    fact_id: impl Into<String>,
    evidence: &CommittedFactEvidence,
) -> DomainResult<MemoryFact> {
    MemoryFact::confirmed(fact_id, evidence)
}

pub fn redact_for_derived_object(
    visibility: &Visibility,
    object: DerivedObject,
    processor: &PrincipalScope,
    target_audience: &PrincipalScope,
) -> RedactionOutcome {
    redaction_for(visibility, object, processor, target_audience)
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
