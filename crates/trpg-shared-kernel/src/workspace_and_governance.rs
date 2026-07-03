use crate::rust_coding_model::validate_rust_symbol_name;
use crate::shared_kernel::{
    validate_command_envelope, CommandEnvelope, EventEnvelope, EventStore, KernelResult, TrpgError,
};

pub const FOUNDATION_GOVERNANCE_REVIEWED_EVENT: &str = "FoundationGovernanceReviewed";

pub const REQUIRED_COMMAND_FIELDS: &[&str] = &[
    "idempotency_key",
    "expected_version",
    "actor",
    "authority_mode",
    "authority_contract_version",
    "visibility",
    "fact_provenance",
    "correlation_id",
    "causation_id",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GovernanceSurface {
    Adr0001RustFirst,
    Constitution,
    DocumentSet,
    SystemContext,
    Readme,
    TechnologySelectionRust,
    WorkspaceAndGovernance,
    CargoWorkspaceImplementation,
    ConstitutionImplementation,
    DocumentSetImplementation,
    OpenSourceReferenceMatrixImplementation,
    SystemContextImplementation,
    TechnologySelectionRustImplementation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CanonicalStateBoundary {
    EventStore,
    Projection,
    Cache,
    RagIndex,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GovernanceContract {
    pub module_name: &'static str,
    pub source_file: &'static str,
    pub test_file: &'static str,
    pub surface: GovernanceSurface,
    pub command_fields: &'static [&'static str],
    pub requires_agent_gateway: bool,
    pub permits_direct_model_provider_access: bool,
    pub permits_direct_agent_state_write: bool,
    pub permits_authority_contract_mutation: bool,
    pub canonical_state_boundary: CanonicalStateBoundary,
    pub read_models_rebuildable: bool,
    pub propagates_visibility_and_provenance: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GovernanceReview {
    pub contract: GovernanceContract,
    pub checked_requirements: Vec<&'static str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GovernanceReviewedPayload {
    pub module_name: &'static str,
    pub surface: GovernanceSurface,
    pub reviewed_requirements: usize,
}

pub fn workspace_governance_contract() -> GovernanceContract {
    GovernanceContract {
        module_name: "workspace_and_governance",
        source_file: "crates/trpg-shared-kernel/src/workspace_and_governance.rs",
        test_file: "crates/trpg-shared-kernel/tests/workspace_and_governance_contract_tests.rs",
        surface: GovernanceSurface::WorkspaceAndGovernance,
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

pub fn workspace_governance_review() -> GovernanceReview {
    GovernanceReview {
        contract: workspace_governance_contract(),
        checked_requirements: vec![
            "authority_contract_is_immutable",
            "formal_writes_use_event_store",
            "read_models_are_rebuildable",
            "visibility_and_fact_provenance_propagate",
            "model_access_uses_agent_gateway",
        ],
    }
}

pub fn validate_governance_contract(contract: &GovernanceContract) -> KernelResult<()> {
    validate_rust_symbol_name(contract.module_name)?;

    if contract.module_name.trim().is_empty()
        || contract.module_name.contains('/')
        || contract.module_name.contains('\\')
    {
        return Err(TrpgError::WorkspaceViolation(
            "module name must be current-safe",
        ));
    }

    let source_suffix = format!("src/{}.rs", contract.module_name);
    let test_suffix = format!("tests/{}_contract_tests.rs", contract.module_name);
    if !contract
        .source_file
        .starts_with("crates/trpg-shared-kernel/")
        || !contract.source_file.ends_with(&source_suffix)
        || !contract.test_file.starts_with("crates/trpg-shared-kernel/")
        || !contract.test_file.ends_with(&test_suffix)
    {
        return Err(TrpgError::WorkspaceViolation(
            "contract outputs must use current-safe shared-kernel paths",
        ));
    }

    for required_field in REQUIRED_COMMAND_FIELDS {
        if !contract.command_fields.contains(required_field) {
            return Err(TrpgError::WorkspaceViolation(
                "command envelope field is missing from contract",
            ));
        }
    }

    if !contract.requires_agent_gateway || contract.permits_direct_model_provider_access {
        return Err(TrpgError::PolicyDenied);
    }

    if contract.permits_direct_agent_state_write {
        return Err(TrpgError::DirectAgentStateWrite);
    }

    if contract.permits_authority_contract_mutation {
        return Err(TrpgError::AuthorityContractMutation);
    }

    if contract.canonical_state_boundary != CanonicalStateBoundary::EventStore
        || !contract.read_models_rebuildable
    {
        return Err(TrpgError::WorkspaceViolation(
            "event store must remain the canonical state boundary",
        ));
    }

    if !contract.propagates_visibility_and_provenance {
        return Err(TrpgError::MissingFactProvenance);
    }

    Ok(())
}

pub fn validate_governance_review(review: &GovernanceReview) -> KernelResult<()> {
    validate_governance_contract(&review.contract)?;
    if review.checked_requirements.is_empty() {
        return Err(TrpgError::WorkspaceViolation(
            "governance review requires checked requirements",
        ));
    }

    Ok(())
}

pub fn append_governance_reviewed(
    store: &mut EventStore<GovernanceReviewedPayload>,
    command: &CommandEnvelope<GovernanceReview>,
) -> KernelResult<EventEnvelope<GovernanceReviewedPayload>> {
    validate_command_envelope(command)?;
    validate_governance_review(&command.payload)?;

    store.append(
        command,
        FOUNDATION_GOVERNANCE_REVIEWED_EVENT,
        GovernanceReviewedPayload {
            module_name: command.payload.contract.module_name,
            surface: command.payload.contract.surface,
            reviewed_requirements: command.payload.checked_requirements.len(),
        },
    )
}
