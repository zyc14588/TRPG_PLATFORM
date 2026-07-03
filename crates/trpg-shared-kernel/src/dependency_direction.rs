use crate::shared_kernel::{KernelResult, TrpgError};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CrateLayer {
    SharedKernel = 0,
    Domain = 1,
    Workflow = 2,
    Runtime = 3,
    Api = 4,
    Agent = 5,
    Infrastructure = 6,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DependencyEdge {
    pub from: CrateLayer,
    pub to: CrateLayer,
}

pub fn validate_dependency_direction(edges: &[DependencyEdge]) -> KernelResult<()> {
    for edge in edges {
        if edge.from == CrateLayer::SharedKernel && edge.to != CrateLayer::SharedKernel {
            return Err(TrpgError::DependencyViolation(
                "shared kernel must not depend on domain, runtime, api, or agent crates",
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
