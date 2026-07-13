use crate::shared_kernel::{CommandEnvelope, EventEnvelope, EventStore, KernelResult, TrpgError};
use crate::workspace_and_governance::{
    append_governance_reviewed, validate_governance_contract, GovernanceContract, GovernanceReview,
    GovernanceReviewedPayload, GovernanceSurface,
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
    GovernanceContract::new("constitution", GovernanceSurface::Constitution)
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
