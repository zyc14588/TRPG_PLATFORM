use crate::shared_kernel::{CommandEnvelope, EventEnvelope, EventStore, KernelResult, TrpgError};
use crate::workspace_and_governance::{
    append_governance_reviewed, validate_governance_contract, CanonicalStateBoundary,
    GovernanceContract, GovernanceReview, GovernanceReviewedPayload, GovernanceSurface,
    REQUIRED_COMMAND_FIELDS,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RustFirstConstraint {
    AuthorityContract,
    AgentGateway,
    EventStoreCanon,
    VisibilityProvenance,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RustFirstDecision {
    pub name: &'static str,
    pub constraint: RustFirstConstraint,
    pub accepted: bool,
    pub bypasses_event_store: bool,
    pub direct_model_provider_access: bool,
    pub mutates_authority_contract: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Adr0001RustFirstRecord {
    pub decisions: Vec<RustFirstDecision>,
    pub governance_contract: GovernanceContract,
}

pub fn adr_0001_rust_first_contract() -> GovernanceContract {
    GovernanceContract {
        module_name: "adr_0001_rust_first",
        source_file: "crates/trpg-shared-kernel/src/adr_0001_rust_first.rs",
        test_file: "crates/trpg-shared-kernel/tests/adr_0001_rust_first_contract_tests.rs",
        surface: GovernanceSurface::Adr0001RustFirst,
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

pub fn current_rust_first_decisions() -> Vec<RustFirstDecision> {
    vec![
        RustFirstDecision {
            name: "authority_contract_is_fork_only",
            constraint: RustFirstConstraint::AuthorityContract,
            accepted: true,
            bypasses_event_store: false,
            direct_model_provider_access: false,
            mutates_authority_contract: false,
        },
        RustFirstDecision {
            name: "ai_access_uses_agent_gateway",
            constraint: RustFirstConstraint::AgentGateway,
            accepted: true,
            bypasses_event_store: false,
            direct_model_provider_access: false,
            mutates_authority_contract: false,
        },
        RustFirstDecision {
            name: "event_store_is_canonical",
            constraint: RustFirstConstraint::EventStoreCanon,
            accepted: true,
            bypasses_event_store: false,
            direct_model_provider_access: false,
            mutates_authority_contract: false,
        },
        RustFirstDecision {
            name: "visibility_and_provenance_propagate",
            constraint: RustFirstConstraint::VisibilityProvenance,
            accepted: true,
            bypasses_event_store: false,
            direct_model_provider_access: false,
            mutates_authority_contract: false,
        },
    ]
}

pub fn adr_0001_rust_first_record(decisions: Vec<RustFirstDecision>) -> Adr0001RustFirstRecord {
    Adr0001RustFirstRecord {
        decisions,
        governance_contract: adr_0001_rust_first_contract(),
    }
}

pub fn validate_adr_0001_rust_first_record(record: &Adr0001RustFirstRecord) -> KernelResult<()> {
    validate_governance_contract(&record.governance_contract)?;

    if !record.decisions.iter().any(|decision| decision.accepted) {
        return Err(TrpgError::WorkspaceViolation(
            "ADR-0001 requires at least one accepted Rust-first decision",
        ));
    }

    if record
        .decisions
        .iter()
        .any(|decision| decision.bypasses_event_store)
    {
        return Err(TrpgError::WorkspaceViolation(
            "formal state must remain event-store canonical",
        ));
    }

    if record
        .decisions
        .iter()
        .any(|decision| decision.direct_model_provider_access)
    {
        return Err(TrpgError::PolicyDenied);
    }

    if record
        .decisions
        .iter()
        .any(|decision| decision.mutates_authority_contract)
    {
        return Err(TrpgError::AuthorityContractMutation);
    }

    Ok(())
}

pub fn adr_0001_rust_first_review() -> GovernanceReview {
    GovernanceReview {
        contract: adr_0001_rust_first_contract(),
        checked_requirements: vec![
            "authority_contract_is_immutable",
            "agent_access_uses_gateway_and_runtime",
            "formal_writes_use_event_store",
            "visibility_and_fact_provenance_propagate",
        ],
    }
}

pub fn append_adr_0001_rust_first_reviewed(
    store: &mut EventStore<GovernanceReviewedPayload>,
    command: &CommandEnvelope<GovernanceReview>,
) -> KernelResult<EventEnvelope<GovernanceReviewedPayload>> {
    append_governance_reviewed(store, command)
}
