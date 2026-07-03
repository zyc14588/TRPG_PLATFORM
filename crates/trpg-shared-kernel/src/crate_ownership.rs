use crate::shared_kernel::{KernelResult, TrpgError};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CrateOwner {
    SharedKernel,
    Domain,
    Runtime,
    Api,
    Agent,
    Infrastructure,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WriteAuthority {
    SharedKernelCore,
    DomainWorkflow,
    RuntimeAdapter,
    ApiBoundary,
    AgentProposalOnly,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CrateOwnership {
    pub crate_name: String,
    pub owner: CrateOwner,
    pub write_authority: WriteAuthority,
}

pub fn validate_crate_ownership(ownership: &CrateOwnership) -> KernelResult<()> {
    if ownership.crate_name == "trpg-shared-kernel" && ownership.owner != CrateOwner::SharedKernel {
        return Err(TrpgError::CrateOwnershipViolation(
            "shared kernel ownership is fixed",
        ));
    }

    if ownership.owner == CrateOwner::Agent
        && ownership.write_authority != WriteAuthority::AgentProposalOnly
    {
        return Err(TrpgError::CrateOwnershipViolation(
            "agent crates can only emit proposals or tool calls",
        ));
    }

    if ownership.owner == CrateOwner::SharedKernel
        && ownership.write_authority != WriteAuthority::SharedKernelCore
    {
        return Err(TrpgError::CrateOwnershipViolation(
            "shared kernel changes require shared kernel core authority",
        ));
    }

    Ok(())
}
