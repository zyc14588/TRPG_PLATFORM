use crate::shared_kernel::{CommandEnvelope, EventEnvelope, EventStore, KernelResult, TrpgError};
use crate::workspace_and_governance::{
    append_governance_reviewed, validate_governance_contract, GovernanceContract, GovernanceReview,
    GovernanceReviewedPayload, GovernanceSurface,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TechnologySelectionRole {
    Runtime,
    Persistence,
    Realtime,
    Policy,
    Observability,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TechnologySelectionDecision {
    pub name: &'static str,
    pub role: TechnologySelectionRole,
    pub selected: bool,
    pub direct_model_provider_access: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TechnologySelectionRustRecord {
    pub decisions: Vec<TechnologySelectionDecision>,
    pub governance_contract: GovernanceContract,
}

pub fn technology_selection_rust_contract() -> GovernanceContract {
    GovernanceContract::new(
        "technology_selection_rust",
        GovernanceSurface::TechnologySelectionRust,
    )
}

pub fn current_rust_technology_selections() -> Vec<TechnologySelectionDecision> {
    vec![
        TechnologySelectionDecision {
            name: "rust",
            role: TechnologySelectionRole::Runtime,
            selected: true,
            direct_model_provider_access: false,
        },
        TechnologySelectionDecision {
            name: "sqlx",
            role: TechnologySelectionRole::Persistence,
            selected: true,
            direct_model_provider_access: false,
        },
        TechnologySelectionDecision {
            name: "nats-jetstream",
            role: TechnologySelectionRole::Realtime,
            selected: true,
            direct_model_provider_access: false,
        },
        TechnologySelectionDecision {
            name: "policy-gate",
            role: TechnologySelectionRole::Policy,
            selected: true,
            direct_model_provider_access: false,
        },
        TechnologySelectionDecision {
            name: "opentelemetry",
            role: TechnologySelectionRole::Observability,
            selected: true,
            direct_model_provider_access: false,
        },
    ]
}

pub fn technology_selection_rust_record(
    decisions: Vec<TechnologySelectionDecision>,
) -> TechnologySelectionRustRecord {
    TechnologySelectionRustRecord {
        decisions,
        governance_contract: technology_selection_rust_contract(),
    }
}

pub fn validate_technology_selection_rust_record(
    record: &TechnologySelectionRustRecord,
) -> KernelResult<()> {
    validate_governance_contract(&record.governance_contract)?;

    if !record.decisions.iter().any(|decision| decision.selected) {
        return Err(TrpgError::WorkspaceViolation(
            "at least one current Rust technology must be selected",
        ));
    }

    let has_persistence = record
        .decisions
        .iter()
        .any(|decision| decision.selected && decision.role == TechnologySelectionRole::Persistence);
    let has_policy = record
        .decisions
        .iter()
        .any(|decision| decision.selected && decision.role == TechnologySelectionRole::Policy);

    if !has_persistence || !has_policy {
        return Err(TrpgError::WorkspaceViolation(
            "Rust technology selection requires persistence and policy gates",
        ));
    }

    if record
        .decisions
        .iter()
        .any(|decision| decision.selected && decision.direct_model_provider_access)
    {
        return Err(TrpgError::PolicyDenied);
    }

    Ok(())
}

pub fn technology_selection_rust_review() -> GovernanceReview {
    GovernanceReview {
        contract: technology_selection_rust_contract(),
        checked_requirements: vec![
            "rust_stack_choices_are_current_safe",
            "persistence_and_policy_gates_are_selected",
            "model_access_does_not_bypass_agent_gateway",
            "formal_state_boundary_remains_event_store",
        ],
    }
}

pub fn append_technology_selection_rust_reviewed(
    store: &mut EventStore<GovernanceReviewedPayload>,
    command: &CommandEnvelope<GovernanceReview>,
) -> KernelResult<EventEnvelope<GovernanceReviewedPayload>> {
    append_governance_reviewed(store, command)
}
