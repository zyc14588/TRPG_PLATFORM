use crate::rust_cargo_workspace::{validate_workspace_manifest, WorkspaceManifest};
use crate::shared_kernel::{CommandEnvelope, EventEnvelope, EventStore, KernelResult};
use crate::workspace_and_governance::{
    append_governance_reviewed, validate_governance_contract, GovernanceContract, GovernanceReview,
    GovernanceReviewedPayload, GovernanceSurface, REQUIRED_COMMAND_FIELDS,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CargoWorkspaceLanding {
    pub manifest: WorkspaceManifest,
    pub governance_contract: GovernanceContract,
}

pub fn cargo_workspace_impl_contract() -> GovernanceContract {
    GovernanceContract {
        module_name: "cargo_workspace_impl",
        source_file: "crates/trpg-shared-kernel/src/cargo_workspace_impl.rs",
        test_file: "crates/trpg-shared-kernel/tests/cargo_workspace_impl_contract_tests.rs",
        surface: GovernanceSurface::CargoWorkspaceImplementation,
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

pub fn cargo_workspace_landing(manifest: WorkspaceManifest) -> CargoWorkspaceLanding {
    CargoWorkspaceLanding {
        manifest,
        governance_contract: cargo_workspace_impl_contract(),
    }
}

pub fn validate_cargo_workspace_landing(landing: &CargoWorkspaceLanding) -> KernelResult<()> {
    validate_governance_contract(&landing.governance_contract)?;
    validate_workspace_manifest(&landing.manifest)
}

pub fn cargo_workspace_impl_review() -> GovernanceReview {
    GovernanceReview {
        contract: cargo_workspace_impl_contract(),
        checked_requirements: vec![
            "workspace_manifest_uses_resolver_two",
            "shared_kernel_member_exists",
            "workspace_names_are_current_safe",
        ],
    }
}

pub fn append_cargo_workspace_impl_reviewed(
    store: &mut EventStore<GovernanceReviewedPayload>,
    command: &CommandEnvelope<GovernanceReview>,
) -> KernelResult<EventEnvelope<GovernanceReviewedPayload>> {
    append_governance_reviewed(store, command)
}
