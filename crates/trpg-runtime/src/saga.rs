use crate::runtime_state_machines::{
    append_runtime_event, EventStore, RuntimeEventPayload, RuntimeResult,
};
use trpg_shared_kernel::{AuthorityContract, CommandEnvelope, EntityId, EventEnvelope};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SagaCompensationRequest {
    pub saga_id: EntityId,
}

impl SagaCompensationRequest {
    pub fn new(saga_id: impl Into<String>) -> RuntimeResult<Self> {
        Ok(Self {
            saga_id: EntityId::new(saga_id)?,
        })
    }
}

pub fn record_saga_compensation<T: Clone>(
    store: &mut EventStore<RuntimeEventPayload>,
    contract: &AuthorityContract,
    command: &CommandEnvelope<T>,
    request: SagaCompensationRequest,
) -> RuntimeResult<EventEnvelope<RuntimeEventPayload>> {
    append_runtime_event(
        store,
        contract,
        command,
        "SagaCompensated",
        RuntimeEventPayload::SagaCompensated {
            saga_id: request.saga_id,
        },
    )
}
