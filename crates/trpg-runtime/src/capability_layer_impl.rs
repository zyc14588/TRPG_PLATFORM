use crate::runtime_state_machines::{
    approve_tool_request, commit_decision, evaluate_tool_grant, EventStore, RuntimeDecision,
    RuntimeEventPayload, RuntimeResult, ToolGrantDecision, ToolRequest,
};
use trpg_identity::AuthenticationContext;
use trpg_shared_kernel::{AuthorityContract, AuthorityMode, CommandEnvelope, EventEnvelope};

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
    workflow_authentication: &AuthenticationContext,
    decision: RuntimeDecision,
    now_unix_ms: u64,
) -> RuntimeResult<Vec<EventEnvelope<RuntimeEventPayload>>> {
    commit_decision(
        store,
        contract,
        command,
        workflow_authentication,
        decision,
        now_unix_ms,
    )
}
