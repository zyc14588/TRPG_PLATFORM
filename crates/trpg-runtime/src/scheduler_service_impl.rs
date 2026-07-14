use crate::runtime_state_machines::{
    append_runtime_event, commit_decision, RuntimeDecision, RuntimeEventPayload, RuntimeResult,
};
use trpg_shared_kernel::{AuthorityContract, CommandEnvelope, EntityId, EventEnvelope, EventStore};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SchedulerServiceImplTask {
    pub task_id: EntityId,
    pub due_tick: u64,
}

impl SchedulerServiceImplTask {
    pub fn new(task_id: impl Into<String>, due_tick: u64) -> RuntimeResult<Self> {
        Ok(Self {
            task_id: EntityId::new(task_id)?,
            due_tick,
        })
    }
}

pub fn due_scheduler_service_impl_tasks(
    tasks: &[SchedulerServiceImplTask],
    current_tick: u64,
) -> Vec<SchedulerServiceImplTask> {
    tasks
        .iter()
        .filter(|task| task.due_tick <= current_tick)
        .cloned()
        .collect()
}

pub fn record_scheduler_service_impl_due_task<T: Clone>(
    store: &mut EventStore<RuntimeEventPayload>,
    contract: &AuthorityContract,
    command: &CommandEnvelope<T>,
    task: SchedulerServiceImplTask,
) -> RuntimeResult<EventEnvelope<RuntimeEventPayload>> {
    append_runtime_event(
        store,
        contract,
        command,
        "ScheduledTaskDue",
        RuntimeEventPayload::ScheduledTaskDue {
            task_id: task.task_id,
        },
    )
}

pub fn commit_scheduler_service_impl_decision(
    store: &mut EventStore<RuntimeEventPayload>,
    contract: &AuthorityContract,
    command: &CommandEnvelope<RuntimeDecision>,
    decision: RuntimeDecision,
) -> RuntimeResult<Vec<EventEnvelope<RuntimeEventPayload>>> {
    commit_decision(store, contract, command, decision)
}
