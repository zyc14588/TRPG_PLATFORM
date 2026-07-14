use trpg_runtime::runtime_state_machines::{
    RuntimeAgent, RuntimeDecision, RuntimeEventPayload, RuntimeTool, ToolRequest,
};
use trpg_runtime::workflow_engine;
use trpg_runtime::{ActorRole, AuthorityMode, EventStore};

#[test]
fn workflow_engine_contract_commits_decision_event_chain() {
    let request = ToolRequest::formal(
        RuntimeAgent::AiKeeperOrchestrator,
        RuntimeTool::RequestSkillCheck,
    );
    let decision = RuntimeDecision::new("decision_workflow", "workflow contract", request).unwrap();
    let command = trpg_test_support::governed_command(
        decision.clone(),
        ActorRole::Workflow,
        AuthorityMode::AiKp,
    );
    let contract =
        trpg_test_support::authority_contract("camp_ai_harbor", AuthorityMode::AiKp, 1).unwrap();
    let mut store = EventStore::default();

    let events =
        workflow_engine::commit_workflow_decision(&mut store, &contract, &command, decision)
            .unwrap();

    assert_eq!(events[0].event_type, "ToolRequestApproved");
    assert_eq!(events[1].event_type, "DecisionCommitted");
    assert!(matches!(
        events[1].payload,
        RuntimeEventPayload::DecisionCommitted { .. }
    ));
}
