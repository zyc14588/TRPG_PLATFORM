use crate::runtime_state_machines::{
    approve_tool_request, commit_decision, evaluate_tool_grant, RuntimeDecision,
    RuntimeEventPayload, RuntimeResult, ToolGrantDecision, ToolRequest,
};
use trpg_shared_kernel::{
    AuthorityContract, AuthorityMode, CommandEnvelope, EventEnvelope, EventStore,
};

pub const PROMPT_ID: &str = "CODEX-0386-03-RUNTIME-ORCHESTRATION-027bb089fe";

pub fn evaluate_capability_layer_impl(
    authority_mode: &AuthorityMode,
    request: &ToolRequest,
) -> ToolGrantDecision {
    evaluate_tool_grant(authority_mode, request)
}

pub fn approve_capability_layer_impl_tool(
    authority_mode: &AuthorityMode,
    request: &ToolRequest,
) -> RuntimeResult<ToolGrantDecision> {
    approve_tool_request(authority_mode, request)
}

pub fn commit_capability_layer_impl_decision(
    store: &mut EventStore<RuntimeEventPayload>,
    contract: &AuthorityContract,
    command: &CommandEnvelope<RuntimeDecision>,
    decision: RuntimeDecision,
) -> RuntimeResult<Vec<EventEnvelope<RuntimeEventPayload>>> {
    commit_decision(store, contract, command, decision)
}
