use crate::runtime_state_machines::{
    replay_visible_runtime_events, EventStore, RuntimeEventPayload, RuntimeResult,
};
use trpg_identity::ReplayAuthorization;
use trpg_shared_kernel::EventEnvelope;

pub fn sync_visible_room_events(
    store: &EventStore<RuntimeEventPayload>,
    authorization: &ReplayAuthorization,
    now_unix_ms: u64,
) -> RuntimeResult<Vec<EventEnvelope<RuntimeEventPayload>>> {
    replay_visible_runtime_events(store, authorization, now_unix_ms)
}
