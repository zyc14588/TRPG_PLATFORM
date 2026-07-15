use crate::runtime_state_machines::{
    append_runtime_event, commit_decision, EventStore, RuntimeDecision, RuntimeEventPayload,
    RuntimeResult,
};
use trpg_identity::AuthenticationContext;
use trpg_shared_kernel::{AuthorityContract, CommandEnvelope, EntityId, EventEnvelope};

pub fn start_session_runtime_impl<T: Clone>(
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

pub fn commit_session_runtime_impl_decision(
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
