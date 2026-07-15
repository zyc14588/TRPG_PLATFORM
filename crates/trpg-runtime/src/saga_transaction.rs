use crate::runtime_state_machines::{
    append_runtime_event, EventStore, RuntimeEventPayload, RuntimeResult,
};
use trpg_shared_kernel::{AuthorityContract, CommandEnvelope, EntityId, EventEnvelope};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SagaCompensation {
    pub saga_id: EntityId,
}

impl SagaCompensation {
    pub fn new(saga_id: impl Into<String>) -> RuntimeResult<Self> {
        Ok(Self {
            saga_id: EntityId::new(saga_id)?,
        })
    }
}

pub fn compensate_saga<T: Clone>(
    store: &mut EventStore<RuntimeEventPayload>,
    contract: &AuthorityContract,
    command: &CommandEnvelope<T>,
    compensation: SagaCompensation,
) -> RuntimeResult<EventEnvelope<RuntimeEventPayload>> {
    append_runtime_event(
        store,
        contract,
        command,
        "SagaCompensated",
        RuntimeEventPayload::SagaCompensated {
            saga_id: compensation.saga_id,
        },
    )
}
