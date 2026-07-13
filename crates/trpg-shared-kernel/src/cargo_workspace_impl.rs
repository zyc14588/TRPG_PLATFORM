use crate::rust_cargo_workspace::{validate_workspace_manifest, WorkspaceManifest};
use crate::shared_kernel::{CommandEnvelope, EventEnvelope, EventStore, KernelResult};
use crate::workspace_and_governance::{
    append_governance_reviewed, validate_governance_contract, GovernanceContract, GovernanceReview,
    GovernanceReviewedPayload, GovernanceSurface,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CargoWorkspaceLanding {
    pub manifest: WorkspaceManifest,
    pub governance_contract: GovernanceContract,
}

pub fn cargo_workspace_impl_contract() -> GovernanceContract {
    GovernanceContract::new(
        "cargo_workspace_impl",
        GovernanceSurface::CargoWorkspaceImplementation,
    )
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
