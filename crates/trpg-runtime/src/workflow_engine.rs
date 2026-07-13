use crate::runtime_state_machines::{
    append_runtime_event, commit_decision, RuntimeDecision, RuntimeEventPayload, RuntimeResult,
};
use trpg_shared_kernel::{AuthorityContract, CommandEnvelope, EntityId, EventEnvelope, EventStore};

pub fn advance_workflow<T: Clone>(
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

pub fn commit_workflow_decision(
    store: &mut EventStore<RuntimeEventPayload>,
    contract: &AuthorityContract,
    command: &CommandEnvelope<RuntimeDecision>,
    decision: RuntimeDecision,
) -> RuntimeResult<Vec<EventEnvelope<RuntimeEventPayload>>> {
    commit_decision(store, contract, command, decision)
}
