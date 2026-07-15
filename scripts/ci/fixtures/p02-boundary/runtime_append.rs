use trpg_runtime::runtime_state_machines::{RuntimeEventPayload, RuntimeAgent, RuntimeDecision, RuntimeTool, ToolRequest};
use trpg_runtime::{ActorRole, AuthorityMode, EntityId, EventStore};

fn main() {
    let contract = trpg_test_support::authority_contract(
        "camp_ai_harbor",
        AuthorityMode::AiKp,
        1,
    )
    .unwrap();
    let decision = RuntimeDecision::new(
        "decision_bypass",
        "bypass",
        ToolRequest::formal(RuntimeAgent::AiKeeperOrchestrator, RuntimeTool::CommitDecision),
    )
    .unwrap();
    let command = trpg_test_support::governed_command_for_contract(
        &contract,
        decision,
        ActorRole::Workflow,
    );
    let mut store = EventStore::default();
    let _ = store.append(
        &command,
        "DecisionCommitted",
        RuntimeEventPayload::SessionStarted {
            session_id: EntityId::new("forged_session").unwrap(),
        },
    );
}
