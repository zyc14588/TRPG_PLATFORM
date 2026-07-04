use crate::agent_runtime::{assemble_context, AssembledAgentContext, ContextFact};
use trpg_shared_kernel::PrincipalScope;

pub const PROMPT_ID: &str = "CODEX-0041-04-AI-AGENT-SYSTEM-570f17da9d";

pub fn assemble_agent_context(
    facts: &[ContextFact],
    principal: &PrincipalScope,
) -> AssembledAgentContext {
    assemble_context(facts, principal)
}
