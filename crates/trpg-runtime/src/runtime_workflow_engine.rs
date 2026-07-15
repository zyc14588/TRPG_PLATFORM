use crate::runtime_state_machines::{
    commit_decision, EventStore, RuntimeDecision, RuntimeEventPayload, RuntimeResult,
};
use trpg_identity::AuthenticationContext;
use trpg_shared_kernel::{AuthorityContract, CommandEnvelope, EventEnvelope};

pub fn commit_runtime_workflow_decision(
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
