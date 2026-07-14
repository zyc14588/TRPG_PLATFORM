use crate::runtime_state_machines::{
    append_runtime_event, commit_decision, replay_visible_runtime_events, RuntimeDecision,
    RuntimeEventPayload, RuntimeResult,
};
use trpg_shared_kernel::{
    AuthorityContract, CommandEnvelope, EntityId, EventEnvelope, EventStore, PrincipalScope,
};

pub fn publish_realtime_room_sync_impl_delta<T: Clone>(
    store: &mut EventStore<RuntimeEventPayload>,
    contract: &AuthorityContract,
    command: &CommandEnvelope<T>,
    delta_id: impl Into<String>,
) -> RuntimeResult<EventEnvelope<RuntimeEventPayload>> {
    append_runtime_event(
        store,
        contract,
        command,
        "RealtimeDeltaPublished",
        RuntimeEventPayload::RealtimeDeltaPublished {
            delta_id: EntityId::new(delta_id)?,
        },
    )
}

pub fn commit_realtime_room_sync_impl_decision(
    store: &mut EventStore<RuntimeEventPayload>,
    contract: &AuthorityContract,
    command: &CommandEnvelope<RuntimeDecision>,
    decision: RuntimeDecision,
) -> RuntimeResult<Vec<EventEnvelope<RuntimeEventPayload>>> {
    commit_decision(store, contract, command, decision)
}

pub fn sync_realtime_room_sync_impl_events(
    store: &EventStore<RuntimeEventPayload>,
    principal: &PrincipalScope,
) -> Vec<EventEnvelope<RuntimeEventPayload>> {
    replay_visible_runtime_events(store, principal)
}
