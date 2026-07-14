use crate::runtime_state_machines::{
    commit_decision, create_pending_decision, PendingDecision, RuntimeDecision,
    RuntimeEventPayload, RuntimeResult,
};
use trpg_shared_kernel::{
    AuthorityContract, AuthorityMode, CommandEnvelope, EventEnvelope, EventStore,
};

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
    decision: RuntimeDecision,
) -> RuntimeResult<Vec<EventEnvelope<RuntimeEventPayload>>> {
    commit_decision(store, contract, command, decision)
}
