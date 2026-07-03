use crate::ddd::{
    DomainResult, FactProvenance, FactSource, PrincipalScope, Visibility, VisibilityLabel,
};
use crate::visibility_fact_provenance::{
    most_restrictive_label, promote_fact_to_confirmed, redaction_for, ConfirmedFact, DerivedObject,
    RedactionOutcome,
};

pub fn derive_visibility_label(labels: &[VisibilityLabel]) -> Option<VisibilityLabel> {
    most_restrictive_label(labels)
}

pub fn redact_for_derived_object(
    visibility: &Visibility,
    derived_object: DerivedObject,
    principal: &PrincipalScope,
) -> RedactionOutcome {
    redaction_for(visibility, derived_object, principal)
}

pub fn confirm_event_sourced_fact(
    fact_id: impl Into<String>,
    source: FactSource,
    visibility: Visibility,
    fact_provenance: FactProvenance,
) -> DomainResult<ConfirmedFact> {
    promote_fact_to_confirmed(fact_id, source, visibility, fact_provenance)
}
