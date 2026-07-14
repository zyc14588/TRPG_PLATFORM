use crate::runtime_state_machines::{
    append_runtime_event, commit_decision, RuntimeDecision, RuntimeEventPayload, RuntimeResult,
};
use trpg_shared_kernel::{AuthorityContract, CommandEnvelope, EntityId, EventEnvelope, EventStore};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SagaTransactionImplCompensation {
    pub saga_id: EntityId,
}

impl SagaTransactionImplCompensation {
    pub fn new(saga_id: impl Into<String>) -> RuntimeResult<Self> {
        Ok(Self {
            saga_id: EntityId::new(saga_id)?,
        })
    }
}

pub fn compensate_saga_transaction_impl<T: Clone>(
    store: &mut EventStore<RuntimeEventPayload>,
    contract: &AuthorityContract,
    command: &CommandEnvelope<T>,
    compensation: SagaTransactionImplCompensation,
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

pub fn commit_saga_transaction_impl_decision(
    store: &mut EventStore<RuntimeEventPayload>,
    contract: &AuthorityContract,
    command: &CommandEnvelope<RuntimeDecision>,
    decision: RuntimeDecision,
) -> RuntimeResult<Vec<EventEnvelope<RuntimeEventPayload>>> {
    commit_decision(store, contract, command, decision)
}
