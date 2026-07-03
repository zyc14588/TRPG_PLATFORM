use crate::ddd::{DomainError, DomainResult, PrincipalScope, Visibility};
use crate::visibility_fact_provenance::{redaction_for, DerivedObject, RedactionOutcome};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VisibilityEnforcementPoint {
    ApiResponse,
    AgentContext,
    Summary,
    RagChunk,
    Export,
    Replay,
    LogMetric,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VisibilityEnforcementDecision {
    pub point: VisibilityEnforcementPoint,
    pub outcome: RedactionOutcome,
}

pub fn enforce_visibility_at(
    point: VisibilityEnforcementPoint,
    visibility: &Visibility,
    principal: &PrincipalScope,
) -> DomainResult<VisibilityEnforcementDecision> {
    let derived_object = match point {
        VisibilityEnforcementPoint::AgentContext => DerivedObject::AgentContextForPlayer,
        VisibilityEnforcementPoint::Export => DerivedObject::AnyPlayerOrKeeperExport,
        VisibilityEnforcementPoint::Summary => DerivedObject::SessionSummaryParty,
        _ => DerivedObject::PlayerExport,
    };
    let outcome = redaction_for(visibility, derived_object, principal);

    if outcome == RedactionOutcome::Visible {
        Ok(VisibilityEnforcementDecision { point, outcome })
    } else {
        Err(DomainError::VisibilityDenied)
    }
}
