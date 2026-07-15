use crate::runtime_state_machines::{
    append_runtime_event, commit_decision, replay_visible_runtime_events, EventStore,
    RuntimeDecision, RuntimeEventPayload, RuntimeResult,
};
use trpg_identity::{AuthenticationContext, ReplayAuthorization};
use trpg_shared_kernel::{AuthorityContract, CommandEnvelope, EntityId, EventEnvelope};

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
    workflow_authentication: &AuthenticationContext,
    decision: RuntimeDecision,
    now_unix_ms: u64,
) -> RuntimeResult<Vec<EventEnvelope<RuntimeEventPayload>>> {
    commit_decision(
        store,
        contract,
        command,
        workflow_authentication,
        decision,
        now_unix_ms,
    )
}

pub fn sync_realtime_room_sync_impl_events(
    store: &EventStore<RuntimeEventPayload>,
    authorization: &ReplayAuthorization,
    now_unix_ms: u64,
) -> RuntimeResult<Vec<EventEnvelope<RuntimeEventPayload>>> {
    replay_visible_runtime_events(store, authorization, now_unix_ms)
}
