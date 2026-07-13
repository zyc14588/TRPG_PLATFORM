use crate::shared_kernel::{CommandEnvelope, EventEnvelope, EventStore, KernelResult, TrpgError};
use crate::workspace_and_governance::{
    append_governance_reviewed, validate_governance_contract, GovernanceContract, GovernanceReview,
    GovernanceReviewedPayload, GovernanceSurface,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProductEntryBoundary {
    pub agent_gateway_required: bool,
    pub direct_model_provider_access_forbidden: bool,
    pub event_store_is_canonical: bool,
    pub visibility_and_provenance_required: bool,
}

pub fn readme_contract() -> GovernanceContract {
    GovernanceContract::new("readme", GovernanceSurface::Readme)
}

pub fn current_product_entry_boundary() -> ProductEntryBoundary {
    ProductEntryBoundary {
        agent_gateway_required: true,
        direct_model_provider_access_forbidden: true,
        event_store_is_canonical: true,
        visibility_and_provenance_required: true,
    }
}

pub fn validate_product_entry_boundary(boundary: &ProductEntryBoundary) -> KernelResult<()> {
    validate_governance_contract(&readme_contract())?;

    if !boundary.agent_gateway_required
        || !boundary.direct_model_provider_access_forbidden
        || !boundary.event_store_is_canonical
        || !boundary.visibility_and_provenance_required
    {
        return Err(TrpgError::WorkspaceViolation(
            "product entry must preserve governance boundaries",
        ));
    }

    Ok(())
}

pub fn readme_review() -> GovernanceReview {
    GovernanceReview {
        contract: readme_contract(),
        checked_requirements: vec![
            "agent_gateway_is_required",
            "direct_model_provider_access_is_forbidden",
            "event_store_is_canonical",
            "visibility_and_fact_provenance_propagate",
        ],
    }
}

pub fn append_readme_reviewed(
    store: &mut EventStore<GovernanceReviewedPayload>,
    command: &CommandEnvelope<GovernanceReview>,
) -> KernelResult<EventEnvelope<GovernanceReviewedPayload>> {
    append_governance_reviewed(store, command)
}
