use crate::ddd::{DomainResult, PrincipalScope, Visibility, VisibilityLabel};
use crate::visibility_fact_provenance::{
    most_restrictive_label, most_restrictive_visibility, promote_fact_to_confirmed, redaction_for,
    CommittedFactEvidence, ConfirmedFact, DerivedObject, RedactionOutcome,
};

pub fn derive_visibility_label(labels: &[VisibilityLabel]) -> Option<VisibilityLabel> {
    most_restrictive_label(labels)
}

pub fn derive_visibility(visibilities: &[Visibility]) -> DomainResult<Option<Visibility>> {
    most_restrictive_visibility(visibilities)
}

pub fn redact_for_derived_object(
    visibility: &Visibility,
    derived_object: DerivedObject,
    processor: &PrincipalScope,
    target_audience: &PrincipalScope,
) -> RedactionOutcome {
    redaction_for(visibility, derived_object, processor, target_audience)
}

pub fn confirm_event_sourced_fact(
    fact_id: impl Into<String>,
    evidence: &CommittedFactEvidence,
) -> DomainResult<ConfirmedFact> {
    promote_fact_to_confirmed(fact_id, evidence)
}
