use crate::runtime_state_machines::{replay_visible_runtime_events, RuntimeEventPayload};
use trpg_shared_kernel::{EventEnvelope, EventStore, PrincipalScope};

pub fn visible_runtime_deltas(
    store: &EventStore<RuntimeEventPayload>,
    principal: &PrincipalScope,
) -> Vec<EventEnvelope<RuntimeEventPayload>> {
    replay_visible_runtime_events(store, principal)
}
