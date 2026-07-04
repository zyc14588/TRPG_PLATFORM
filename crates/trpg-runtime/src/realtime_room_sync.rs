use crate::runtime_state_machines::{replay_visible_runtime_events, RuntimeEventPayload};
use trpg_shared_kernel::{EventEnvelope, EventStore, PrincipalScope};

pub const PROMPT_ID: &str = "CODEX-0347-03-RUNTIME-ORCHESTRATION-b0e055d98c";

pub fn sync_visible_room_events(
    store: &EventStore<RuntimeEventPayload>,
    principal: &PrincipalScope,
) -> Vec<EventEnvelope<RuntimeEventPayload>> {
    replay_visible_runtime_events(store, principal)
}
