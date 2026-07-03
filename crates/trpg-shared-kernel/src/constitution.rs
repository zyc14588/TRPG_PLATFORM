use crate::shared_kernel::{CommandEnvelope, EventEnvelope, EventStore, KernelResult, TrpgError};
use crate::workspace_and_governance::{
    append_governance_reviewed, validate_governance_contract, GovernanceContract, GovernanceReview,
    GovernanceReviewedPayload, GovernanceSurface, REQUIRED_COMMAND_FIELDS,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConstitutionArticle {
    AuthorityContractImmutable,
    AgentGatewayRequired,
    FormalWritesUseEventStore,
    VisibilityAndProvenanceRequired,
    ServerDiceOnly,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConstitutionChecklist {
    pub articles: Vec<ConstitutionArticle>,
}

pub fn constitution_contract() -> GovernanceContract {
    GovernanceContract {
        module_name: "constitution",
        source_file: "crates/trpg-shared-kernel/src/constitution.rs",
        test_file: "crates/trpg-shared-kernel/tests/constitution_contract_tests.rs",
        surface: GovernanceSurface::Constitution,
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

pub fn current_constitution_checklist() -> ConstitutionChecklist {
    ConstitutionChecklist {
        articles: vec![
            ConstitutionArticle::AuthorityContractImmutable,
            ConstitutionArticle::AgentGatewayRequired,
            ConstitutionArticle::FormalWritesUseEventStore,
            ConstitutionArticle::VisibilityAndProvenanceRequired,
            ConstitutionArticle::ServerDiceOnly,
        ],
    }
}

pub fn validate_constitution_checklist(checklist: &ConstitutionChecklist) -> KernelResult<()> {
    validate_governance_contract(&constitution_contract())?;

    for article in [
        ConstitutionArticle::AuthorityContractImmutable,
        ConstitutionArticle::AgentGatewayRequired,
        ConstitutionArticle::FormalWritesUseEventStore,
        ConstitutionArticle::VisibilityAndProvenanceRequired,
        ConstitutionArticle::ServerDiceOnly,
    ] {
        if !checklist.articles.contains(&article) {
            return Err(TrpgError::WorkspaceViolation(
                "constitution article is missing",
            ));
        }
    }

    Ok(())
}

pub fn constitution_review() -> GovernanceReview {
    GovernanceReview {
        contract: constitution_contract(),
        checked_requirements: vec![
            "authority_contract_immutable",
            "human_and_ai_keeper_modes_are_exclusive",
            "formal_decisions_use_rules_tools_state_and_event_log",
            "visibility_and_fact_provenance_are_required",
            "official_dice_are_server_generated",
        ],
    }
}

pub fn append_constitution_reviewed(
    store: &mut EventStore<GovernanceReviewedPayload>,
    command: &CommandEnvelope<GovernanceReview>,
) -> KernelResult<EventEnvelope<GovernanceReviewedPayload>> {
    append_governance_reviewed(store, command)
}
