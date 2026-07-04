use crate::runtime_state_machines::{
    commit_decision, RuntimeDecision, RuntimeEventPayload, RuntimeResult,
};
use trpg_shared_kernel::{AuthorityContract, CommandEnvelope, EventEnvelope, EventStore};

pub const PROMPT_ID: &str = "CODEX-0344-03-RUNTIME-ORCHESTRATION-22393092aa";

pub fn commit_runtime_workflow_decision(
    store: &mut EventStore<RuntimeEventPayload>,
    contract: &AuthorityContract,
    command: &CommandEnvelope<RuntimeDecision>,
    decision: RuntimeDecision,
) -> RuntimeResult<Vec<EventEnvelope<RuntimeEventPayload>>> {
    commit_decision(store, contract, command, decision)
}
