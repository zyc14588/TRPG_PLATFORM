use crate::runtime_state_machines::{
    replay_visible_runtime_events, EventStore, RuntimeEventPayload,
};
use trpg_shared_kernel::{EventEnvelope, PrincipalScope};

pub fn sync_visible_room_events(
    store: &EventStore<RuntimeEventPayload>,
    principal: &PrincipalScope,
) -> Vec<EventEnvelope<RuntimeEventPayload>> {
    replay_visible_runtime_events(store, principal)
}
