use crate::{
    evaluate_security_governance, SecurityGovernanceCommand, SecurityGovernanceEventEnvelope,
    SecurityGovernanceRepository,
};
use trpg_shared_kernel::{CommandEnvelope, KernelResult};

pub const MODULE: &str = "security_governance::audit_log_contract";

pub fn evaluate(
    repository: &mut SecurityGovernanceRepository,
    command: &CommandEnvelope<SecurityGovernanceCommand>,
) -> KernelResult<SecurityGovernanceEventEnvelope> {
    evaluate_security_governance(MODULE, repository, command)
}
