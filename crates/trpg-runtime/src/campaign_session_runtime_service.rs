use crate::runtime_state_machines::{
    append_runtime_event, EventStore, RuntimeEventPayload, RuntimeResult,
};
use trpg_shared_kernel::{AuthorityContract, CommandEnvelope, EntityId, EventEnvelope};

pub fn start_campaign_session<T: Clone>(
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

pub fn advance_campaign_workflow<T: Clone>(
    store: &mut EventStore<RuntimeEventPayload>,
    contract: &AuthorityContract,
    command: &CommandEnvelope<T>,
    workflow_id: impl Into<String>,
) -> RuntimeResult<EventEnvelope<RuntimeEventPayload>> {
    append_runtime_event(
        store,
        contract,
        command,
        "WorkflowAdvanced",
        RuntimeEventPayload::WorkflowAdvanced {
            workflow_id: EntityId::new(workflow_id)?,
        },
    )
}
