use crate::runtime_state_machines::{append_runtime_event, RuntimeEventPayload, RuntimeResult};
use trpg_shared_kernel::{AuthorityContract, CommandEnvelope, EntityId, EventEnvelope, EventStore};

pub fn start_session<T: Clone>(
    store: &mut EventStore<RuntimeEventPayload>,
    contract: &AuthorityContract,
    command: &CommandEnvelope<T>,
    session_id: impl Into<String>,
) -> RuntimeResult<EventEnvelope<RuntimeEventPayload>> {
    append_runtime_event(
        store,
        contract,
        command,
        "SessionStarted",
        RuntimeEventPayload::SessionStarted {
            session_id: EntityId::new(session_id)?,
        },
    )
}
