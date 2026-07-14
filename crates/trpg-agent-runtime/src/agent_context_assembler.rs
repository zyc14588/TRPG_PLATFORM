use crate::agent_runtime::{assemble_context, AssembledAgentContext, ContextFact};
use trpg_shared_kernel::PrincipalScope;

pub fn assemble_agent_context(
    facts: &[ContextFact],
    principal: &PrincipalScope,
) -> AssembledAgentContext {
    assemble_context(facts, principal)
}
