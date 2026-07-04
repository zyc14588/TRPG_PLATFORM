use crate::runtime_state_machines::{replay_visible_runtime_events, RuntimeEventPayload};
use trpg_shared_kernel::{EventEnvelope, EventStore, PrincipalScope};

pub const PROMPT_ID: &str = "CODEX-0034-03-RUNTIME-ORCHESTRATION-20e1521d8e";

pub fn visible_runtime_deltas(
    store: &EventStore<RuntimeEventPayload>,
    principal: &PrincipalScope,
) -> Vec<EventEnvelope<RuntimeEventPayload>> {
    replay_visible_runtime_events(store, principal)
}
