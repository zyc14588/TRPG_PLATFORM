use crate::shared_kernel::{KernelResult, TrpgError};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CrateLayer {
    Contracts = 0,
    SharedKernel = 1,
    Domain = 2,
    Workflow = 3,
    Runtime = 4,
    Api = 5,
    Agent = 6,
    Infrastructure = 7,
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
                "wire contracts must not depend on product crates",
            ));
        }

        if edge.from == CrateLayer::SharedKernel
            && !matches!(edge.to, CrateLayer::Contracts | CrateLayer::SharedKernel)
        {
            return Err(TrpgError::DependencyViolation(
                "shared kernel may depend only on wire contracts",
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
