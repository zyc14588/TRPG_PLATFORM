use crate::runtime_state_machines::{
    commit_decision, EventStore, RuntimeDecision, RuntimeEventPayload, RuntimeResult,
};
use trpg_shared_kernel::{AuthorityContract, CommandEnvelope, EventEnvelope};

pub fn commit_runtime_workflow_decision(
    store: &mut EventStore<RuntimeEventPayload>,
    contract: &AuthorityContract,
    command: &CommandEnvelope<RuntimeDecision>,
    decision: RuntimeDecision,
) -> RuntimeResult<Vec<EventEnvelope<RuntimeEventPayload>>> {
    commit_decision(store, contract, command, decision)
}
