use crate::shared_kernel::{KernelResult, TrpgError};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CrateLayer {
    Contracts,
    SharedKernel,
    Domain,
    Ruleset,
    Infrastructure,
    Workflow,
    Runtime,
    Api,
    Agent,
    ProcessAdapter,
    Application,
    TestSupport,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DependencyEdge {
    pub from: CrateLayer,
    pub to: CrateLayer,
}

pub fn validate_dependency_direction(edges: &[DependencyEdge]) -> KernelResult<()> {
    for edge in edges {
        if edge.from == CrateLayer::Contracts && edge.to != CrateLayer::Contracts {
            return Err(TrpgError::DependencyViolation(
                "contracts must not depend on higher workspace layers",
            ));
        }

        if edge.from == CrateLayer::SharedKernel
            && !matches!(edge.to, CrateLayer::Contracts | CrateLayer::SharedKernel)
        {
            return Err(TrpgError::DependencyViolation(
                "shared kernel may only depend on contracts",
            ));
        }

        if edge.to > edge.from {
            return Err(TrpgError::DependencyViolation(
                "dependencies must point toward lower governance layers",
            ));
        }
    }

    Ok(())
}
