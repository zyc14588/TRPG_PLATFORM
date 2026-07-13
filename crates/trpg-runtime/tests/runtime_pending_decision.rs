use trpg_runtime::runtime_pending_decision;
use trpg_runtime::runtime_state_machines::{
    PendingDecisionStatus, RuntimeAgent, RuntimeDecision, RuntimeTool, ToolRequest,
};
use trpg_runtime::{ActorRole, AuthorityContract, AuthorityMode, EventStore};

#[test]
fn runtime_pending_decision_target_commits_ready_decision() {
    let request = ToolRequest::formal(
        RuntimeAgent::AiKeeperOrchestrator,
        RuntimeTool::RequestSkillCheck,
    );
    let decision = RuntimeDecision::new("decision_target", "target test", request).unwrap();
    let pending = runtime_pending_decision::open_runtime_pending_decision(
        &AuthorityMode::AiKp,
        decision.clone(),
    );

    assert_eq!(pending.status, PendingDecisionStatus::ReadyToCommit);

    let command = trpg_test_support::governed_command!(
        decision.clone(),
        ActorRole::Workflow,
        AuthorityMode::AiKp
    );
    let contract = AuthorityContract::new("camp_ai_harbor", AuthorityMode::AiKp, 1).unwrap();
    let mut store = EventStore::default();

    let events = runtime_pending_decision::commit_runtime_pending_decision(
        &mut store, &contract, &command, decision,
    )
    .unwrap();

    assert_eq!(events.len(), 2);
    assert_eq!(events[1].event_type, "DecisionCommitted");
}
