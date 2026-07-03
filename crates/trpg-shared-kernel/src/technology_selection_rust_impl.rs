use crate::shared_kernel::{CommandEnvelope, EventEnvelope, EventStore, KernelResult, TrpgError};
use crate::workspace_and_governance::{
    append_governance_reviewed, validate_governance_contract, CanonicalStateBoundary,
    GovernanceContract, GovernanceReview, GovernanceReviewedPayload, GovernanceSurface,
    REQUIRED_COMMAND_FIELDS,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RustTechnologyRole {
    Runtime,
    Persistence,
    Realtime,
    Policy,
    Observability,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RustTechnologyDecision {
    pub name: &'static str,
    pub role: RustTechnologyRole,
    pub selected: bool,
    pub direct_model_provider_access: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TechnologySelectionRustLanding {
    pub decisions: Vec<RustTechnologyDecision>,
    pub governance_contract: GovernanceContract,
}

pub fn technology_selection_rust_impl_contract() -> GovernanceContract {
    GovernanceContract {
        module_name: "technology_selection_rust_impl",
        source_file: "crates/trpg-shared-kernel/src/technology_selection_rust_impl.rs",
        test_file:
            "crates/trpg-shared-kernel/tests/technology_selection_rust_impl_contract_tests.rs",
        surface: GovernanceSurface::TechnologySelectionRustImplementation,
        command_fields: REQUIRED_COMMAND_FIELDS,
        requires_agent_gateway: true,
        permits_direct_model_provider_access: false,
        permits_direct_agent_state_write: false,
        permits_authority_contract_mutation: false,
        canonical_state_boundary: CanonicalStateBoundary::EventStore,
        read_models_rebuildable: true,
        propagates_visibility_and_provenance: true,
    }
}

pub fn current_rust_technology_decisions() -> Vec<RustTechnologyDecision> {
    vec![
        RustTechnologyDecision {
            name: "rust",
            role: RustTechnologyRole::Runtime,
            selected: true,
            direct_model_provider_access: false,
        },
        RustTechnologyDecision {
            name: "sqlx",
            role: RustTechnologyRole::Persistence,
            selected: true,
            direct_model_provider_access: false,
        },
        RustTechnologyDecision {
            name: "nats-jetstream",
            role: RustTechnologyRole::Realtime,
            selected: true,
            direct_model_provider_access: false,
        },
        RustTechnologyDecision {
            name: "policy-gate",
            role: RustTechnologyRole::Policy,
            selected: true,
            direct_model_provider_access: false,
        },
        RustTechnologyDecision {
            name: "opentelemetry",
            role: RustTechnologyRole::Observability,
            selected: true,
            direct_model_provider_access: false,
        },
    ]
}

pub fn technology_selection_rust_landing(
    decisions: Vec<RustTechnologyDecision>,
) -> TechnologySelectionRustLanding {
    TechnologySelectionRustLanding {
        decisions,
        governance_contract: technology_selection_rust_impl_contract(),
    }
}

pub fn validate_technology_selection_rust_landing(
    landing: &TechnologySelectionRustLanding,
) -> KernelResult<()> {
    validate_governance_contract(&landing.governance_contract)?;

    if !landing.decisions.iter().any(|decision| decision.selected) {
        return Err(TrpgError::WorkspaceViolation(
            "at least one current Rust technology must be selected",
        ));
    }

    if landing
        .decisions
        .iter()
        .any(|decision| decision.selected && decision.direct_model_provider_access)
    {
        return Err(TrpgError::PolicyDenied);
    }

    Ok(())
}

pub fn technology_selection_rust_impl_review() -> GovernanceReview {
    GovernanceReview {
        contract: technology_selection_rust_impl_contract(),
        checked_requirements: vec![
            "rust_stack_choices_are_current_safe",
            "selected_technologies_do_not_bypass_agent_gateway",
            "formal_state_boundary_remains_event_store",
        ],
    }
}

pub fn append_technology_selection_rust_impl_reviewed(
    store: &mut EventStore<GovernanceReviewedPayload>,
    command: &CommandEnvelope<GovernanceReview>,
) -> KernelResult<EventEnvelope<GovernanceReviewedPayload>> {
    append_governance_reviewed(store, command)
}
