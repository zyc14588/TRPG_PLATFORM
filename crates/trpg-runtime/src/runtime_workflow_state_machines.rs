use crate::runtime_state_machines::{
    commit_decision, replay_visible_runtime_events, EventStore, RuntimeDecision,
    RuntimeEventPayload, RuntimeResult,
};
use trpg_shared_kernel::{AuthorityContract, CommandEnvelope, EventEnvelope, PrincipalScope};

pub fn commit_runtime_workflow_state_machine_decision(
    store: &mut EventStore<RuntimeEventPayload>,
    contract: &AuthorityContract,
    command: &CommandEnvelope<RuntimeDecision>,
    decision: RuntimeDecision,
) -> RuntimeResult<Vec<EventEnvelope<RuntimeEventPayload>>> {
    commit_decision(store, contract, command, decision)
}

pub fn replay_runtime_workflow_state_machine_events(
    store: &EventStore<RuntimeEventPayload>,
    principal: &PrincipalScope,
) -> Vec<EventEnvelope<RuntimeEventPayload>> {
    replay_visible_runtime_events(store, principal)
}
