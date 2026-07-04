use trpg_shared_kernel::EntityId;

pub const PROMPT_ID: &str = "CODEX-0037-03-RUNTIME-ORCHESTRATION-c9bd0a0635";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScheduledRuntimeTask {
    pub task_id: EntityId,
    pub due_tick: u64,
}

impl ScheduledRuntimeTask {
    pub fn new(
        task_id: impl Into<String>,
        due_tick: u64,
    ) -> Result<Self, trpg_shared_kernel::TrpgError> {
        Ok(Self {
            task_id: EntityId::new(task_id)?,
            due_tick,
        })
    }
}

pub fn due_tasks(tasks: &[ScheduledRuntimeTask], current_tick: u64) -> Vec<ScheduledRuntimeTask> {
    tasks
        .iter()
        .filter(|task| task.due_tick <= current_tick)
        .cloned()
        .collect()
}
