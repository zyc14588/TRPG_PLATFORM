use crate::ddd::{PrincipalScope, Visibility};
use crate::visibility_fact_provenance::{redaction_for, DerivedObject, RedactionOutcome};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VisibilityLeakageProbe {
    pub derived_object: DerivedObject,
    pub principal: PrincipalScope,
}

pub fn detect_visibility_leakage(visibility: &Visibility, probe: &VisibilityLeakageProbe) -> bool {
    redaction_for(visibility, probe.derived_object, &probe.principal) == RedactionOutcome::Visible
}
