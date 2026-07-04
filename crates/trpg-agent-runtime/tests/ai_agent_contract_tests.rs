use trpg_agent_runtime::ai_agent;
use trpg_agent_runtime::{
    ActorRole, AgentDecision, AgentEventPayload, AgentKind, AgentTool, AuthorityContract,
    AuthorityMode, CommandEnvelope, EventStore, ToolRequest,
};

fn ai_kp_command(payload: AgentDecision) -> CommandEnvelope<AgentDecision> {
    CommandEnvelope::governed(payload, ActorRole::Workflow, AuthorityMode::AiKp)
}

#[test]
fn ai_agent_commits_only_through_event_store_with_provenance() {
    assert_eq!(
        ai_agent::PROMPT_ID,
        "CODEX-0470-04-AI-AGENT-SYSTEM-01fd0c2f41"
    );
    let boundary = ai_agent::ai_agent_boundary();
    assert_eq!(boundary.ai_entrypoint, "Agent Gateway");
    assert!(boundary
        .formal_state_path
        .contains("Command -> Workflow -> Decision -> Event Store"));

    let request = ToolRequest::formal(
        AgentKind::AiKeeperOrchestrator,
        AgentTool::RequestSkillCheck,
    );
    let decision = AgentDecision::new("decision_b018_ai_agent", request, "Spot Hidden").unwrap();
    let command = ai_kp_command(decision.clone());
    let contract =
        AuthorityContract::new("campaign_b018_ai_agent", AuthorityMode::AiKp, 1).unwrap();
    let mut store = EventStore::default();

    let events =
        ai_agent::submit_ai_agent_decision(&mut store, &contract, &command, decision).unwrap();

    assert_eq!(events.len(), 2);
    assert_eq!(store.events().len(), 2);
    assert_eq!(events[0].event_type, "ToolRequestApproved");
    assert_eq!(events[1].event_type, "DecisionCommitted");
    assert_eq!(events[1].fact_provenance, command.fact_provenance);
    match &events[1].payload {
        AgentEventPayload::DecisionCommitted {
            linked_records,
            audit_fields,
            ..
        } => {
            assert!(linked_records.contains(&"DecisionRecord"));
            assert!(linked_records.contains(&"GameEvent"));
            assert!(audit_fields.contains(&"visibility_labels"));
            assert!(audit_fields.contains(&"model_provider"));
        }
        other => panic!("unexpected event payload: {other:?}"),
    }
}

#[test]
fn ai_agent_rejects_authority_contract_mismatch_without_event_write() {
    let request = ToolRequest::formal(
        AgentKind::AiKeeperOrchestrator,
        AgentTool::RequestSkillCheck,
    );
    let decision =
        AgentDecision::new("decision_b018_ai_agent_mismatch", request, "Listen").unwrap();
    let command = ai_kp_command(decision.clone());
    let contract =
        AuthorityContract::new("campaign_b018_ai_agent", AuthorityMode::HumanKp, 1).unwrap();
    let mut store = EventStore::default();

    let error =
        ai_agent::submit_ai_agent_decision(&mut store, &contract, &command, decision).unwrap_err();

    assert_eq!(error.code(), "AUTHORITY_CONTRACT_MUTATION");
    assert!(store.events().is_empty());
}
