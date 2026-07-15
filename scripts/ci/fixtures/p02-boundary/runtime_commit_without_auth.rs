use trpg_runtime::runtime_state_machines::{
    commit_decision, EventStore, RuntimeAgent, RuntimeDecision, RuntimeTool, ToolRequest,
};
use trpg_runtime::{ActorRole, AuthorityMode};

fn main() {
    let contract =
        trpg_test_support::authority_contract("camp_ai_harbor", AuthorityMode::AiKp, 1).unwrap();
    let decision = RuntimeDecision::new(
        "decision_missing_auth",
        "missing trusted authentication",
        ToolRequest::formal(RuntimeAgent::AiKeeperOrchestrator, RuntimeTool::CommitDecision),
    )
    .unwrap();
    let command = trpg_test_support::governed_command_for_contract(
        &contract,
        decision.clone(),
        ActorRole::Workflow,
    );
    let mut store = EventStore::default();
    let _ = commit_decision(&mut store, &contract, &command, decision);
}
