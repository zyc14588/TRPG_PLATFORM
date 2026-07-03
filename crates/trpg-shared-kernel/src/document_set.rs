use crate::shared_kernel::{CommandEnvelope, EventEnvelope, EventStore, KernelResult, TrpgError};
use crate::workspace_and_governance::{
    append_governance_reviewed, validate_governance_contract, GovernanceContract, GovernanceReview,
    GovernanceReviewedPayload, GovernanceSurface, REQUIRED_COMMAND_FIELDS,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FoundationDocument {
    TopLevelDesign,
    BootstrapPrompt,
    SourceBundleGuide,
    NormalizedExecutionMap,
    SafeOutputMap,
    TokenRewriteTable,
    BatchPlan,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FoundationDocumentSet {
    pub documents: Vec<FoundationDocument>,
    pub historical_inputs_are_provenance_only: bool,
}

pub fn document_set_contract() -> GovernanceContract {
    GovernanceContract {
        module_name: "document_set",
        source_file: "crates/trpg-shared-kernel/src/document_set.rs",
        test_file: "crates/trpg-shared-kernel/tests/document_set_contract_tests.rs",
        surface: GovernanceSurface::DocumentSet,
        command_fields: REQUIRED_COMMAND_FIELDS,
        requires_agent_gateway: true,
        permits_direct_model_provider_access: false,
        permits_direct_agent_state_write: false,
        permits_authority_contract_mutation: false,
        canonical_state_boundary:
            crate::workspace_and_governance::CanonicalStateBoundary::EventStore,
        read_models_rebuildable: true,
        propagates_visibility_and_provenance: true,
    }
}

pub fn current_foundation_document_set() -> FoundationDocumentSet {
    FoundationDocumentSet {
        documents: vec![
            FoundationDocument::TopLevelDesign,
            FoundationDocument::BootstrapPrompt,
            FoundationDocument::SourceBundleGuide,
            FoundationDocument::NormalizedExecutionMap,
            FoundationDocument::SafeOutputMap,
            FoundationDocument::TokenRewriteTable,
            FoundationDocument::BatchPlan,
        ],
        historical_inputs_are_provenance_only: true,
    }
}

pub fn validate_foundation_document_set(document_set: &FoundationDocumentSet) -> KernelResult<()> {
    validate_governance_contract(&document_set_contract())?;

    for document in [
        FoundationDocument::TopLevelDesign,
        FoundationDocument::BootstrapPrompt,
        FoundationDocument::SourceBundleGuide,
        FoundationDocument::NormalizedExecutionMap,
        FoundationDocument::SafeOutputMap,
        FoundationDocument::TokenRewriteTable,
        FoundationDocument::BatchPlan,
    ] {
        if !document_set.documents.contains(&document) {
            return Err(TrpgError::WorkspaceViolation(
                "foundation document is missing",
            ));
        }
    }

    if !document_set.historical_inputs_are_provenance_only {
        return Err(TrpgError::WorkspaceViolation(
            "historical inputs must remain provenance only",
        ));
    }

    Ok(())
}

pub fn document_set_review() -> GovernanceReview {
    GovernanceReview {
        contract: document_set_contract(),
        checked_requirements: vec![
            "top_level_design_precedes_batch_execution",
            "normalized_maps_precede_per_file_prompts",
            "historical_inputs_are_provenance_only",
        ],
    }
}

pub fn append_document_set_reviewed(
    store: &mut EventStore<GovernanceReviewedPayload>,
    command: &CommandEnvelope<GovernanceReview>,
) -> KernelResult<EventEnvelope<GovernanceReviewedPayload>> {
    append_governance_reviewed(store, command)
}
