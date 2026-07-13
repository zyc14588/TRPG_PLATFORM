use crate::constitution::{
    current_constitution_checklist, validate_constitution_checklist, ConstitutionChecklist,
};
use crate::shared_kernel::{CommandEnvelope, EventEnvelope, EventStore, KernelResult};
use crate::workspace_and_governance::{
    append_governance_reviewed, validate_governance_contract, GovernanceContract, GovernanceReview,
    GovernanceReviewedPayload, GovernanceSurface,
};

pub fn document_set_contract() -> GovernanceContract {
    GovernanceContract::new("document_set", GovernanceSurface::DocumentSet)
}

pub fn current_governance_checklist() -> ConstitutionChecklist {
    current_constitution_checklist()
}

pub fn validate_governance_checklist(checklist: &ConstitutionChecklist) -> KernelResult<()> {
    validate_governance_contract(&document_set_contract())?;
    validate_constitution_checklist(checklist)
}

pub fn document_set_review() -> GovernanceReview {
    GovernanceReview {
        contract: document_set_contract(),
        checked_requirements: vec![
            "authority_contract_is_immutable",
            "agent_gateway_is_required",
            "formal_writes_use_event_store",
            "visibility_and_fact_provenance_propagate",
            "server_generates_formal_dice",
        ],
    }
}

pub fn append_document_set_reviewed(
    store: &mut EventStore<GovernanceReviewedPayload>,
    command: &CommandEnvelope<GovernanceReview>,
) -> KernelResult<EventEnvelope<GovernanceReviewedPayload>> {
    append_governance_reviewed(store, command)
}
