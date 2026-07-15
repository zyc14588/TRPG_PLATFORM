use crate::runtime_state_machines::{
    commit_decision, create_pending_decision, EventStore, PendingDecision, RuntimeDecision,
    RuntimeEventPayload, RuntimeResult,
};
use trpg_identity::AuthenticationContext;
use trpg_shared_kernel::{AuthorityContract, AuthorityMode, CommandEnvelope, EventEnvelope};

pub fn open_runtime_pending_decision(
    authority_mode: &AuthorityMode,
    decision: RuntimeDecision,
) -> PendingDecision {
    create_pending_decision(authority_mode, decision)
}

pub fn commit_runtime_pending_decision(
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
