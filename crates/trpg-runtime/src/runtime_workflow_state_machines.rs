use crate::runtime_state_machines::{
    append_runtime_event, commit_decision, replay_visible_runtime_events, RuntimeDecision,
    RuntimeEventPayload, RuntimeResult,
};
use trpg_shared_kernel::{
    AuthorityContract, CommandEnvelope, EventEnvelope, EventStore, PrincipalScope,
};

pub fn append_runtime_workflow_state_machine_event<T: Clone>(
    store: &mut EventStore<RuntimeEventPayload>,
    contract: &AuthorityContract,
    command: &CommandEnvelope<T>,
    event_type: &'static str,
    payload: RuntimeEventPayload,
) -> RuntimeResult<EventEnvelope<RuntimeEventPayload>> {
    append_runtime_event(store, contract, command, event_type, payload)
}

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
