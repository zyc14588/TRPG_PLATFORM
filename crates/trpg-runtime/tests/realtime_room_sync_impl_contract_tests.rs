use trpg_runtime::realtime_room_sync_impl;
use trpg_runtime::runtime_state_machines::{
    RuntimeAgent, RuntimeDecision, RuntimeEventPayload, RuntimeModule, RuntimeTool, ToolRequest,
    BATCH_014_PRIMARY_MODULES,
};
use trpg_runtime::{
    ActorRole, AuthorityContract, AuthorityMode, CommandEnvelope, EventStore, FormalWritePath,
    PrincipalScope, Visibility, VisibilityLabel,
};

fn decision(decision_id: &str, request: ToolRequest) -> RuntimeDecision {
    RuntimeDecision::new(decision_id, "B014 realtime decision", request).unwrap()
}

fn command(payload: RuntimeDecision) -> CommandEnvelope<RuntimeDecision> {
    CommandEnvelope::governed(payload, ActorRole::Workflow, AuthorityMode::AiKp)
}

#[test]
fn realtime_room_sync_impl_preserves_governed_decision_event_contract() {
    assert_eq!(
        realtime_room_sync_impl::PROMPT_ID,
        "CODEX-0388-03-RUNTIME-ORCHESTRATION-705a854eb2"
    );
    assert!(BATCH_014_PRIMARY_MODULES.contains(&RuntimeModule::RealtimeRoomSyncImpl));

    let request = ToolRequest::formal(
        RuntimeAgent::AiKeeperOrchestrator,
        RuntimeTool::RequestSkillCheck,
    );
    let decision = decision("decision_b014_realtime", request);
    let mut command = command(decision.clone());
    command.visibility = Visibility::new(VisibilityLabel::KeeperOnly);
    let contract = AuthorityContract::new("camp_ai_harbor", AuthorityMode::AiKp, 1).unwrap();
    let mut store = EventStore::default();

    let events = realtime_room_sync_impl::commit_realtime_room_sync_impl_decision(
        &mut store, &contract, &command, decision,
    )
    .unwrap();

    assert_eq!(store.events().len(), 2);
    assert_eq!(events[0].event_type, "ToolRequestApproved");
    assert_eq!(events[1].event_type, "DecisionCommitted");
    assert!(
        realtime_room_sync_impl::sync_realtime_room_sync_impl_events(
            &store,
            &PrincipalScope::Public,
        )
        .is_empty()
    );
    assert_eq!(
        realtime_room_sync_impl::sync_realtime_room_sync_impl_events(
            &store,
            &PrincipalScope::Keeper,
        )
        .len(),
        2
    );
    for event in &events {
        assert_eq!(event.visibility.label(), &VisibilityLabel::KeeperOnly);
        assert_eq!(event.fact_provenance.reference.as_str(), "fact_001");
        assert_eq!(event.fact_provenance.recorded_by.as_str(), "rules_001");
    }
    match &events[1].payload {
        RuntimeEventPayload::DecisionCommitted {
            linked_records,
            audit_fields,
            ..
        } => {
            assert!(linked_records.contains(&"DecisionRecord"));
            assert!(audit_fields.contains(&"model_provider"));
        }
        other => panic!("unexpected payload: {other:?}"),
    }
}

#[test]
fn realtime_room_sync_impl_denies_contract_tool_gate_and_direct_agent_write() {
    let contract = AuthorityContract::new("camp_ai_harbor", AuthorityMode::AiKp, 1).unwrap();
    assert_eq!(
        contract.fork(AuthorityMode::HumanKp, 1).unwrap_err().code(),
        "AUTHORITY_CONTRACT_MUTATION"
    );

    let allowed = decision(
        "decision_b014_realtime_contract",
        ToolRequest::formal(
            RuntimeAgent::AiKeeperOrchestrator,
            RuntimeTool::RequestSkillCheck,
        ),
    );
    let wrong_contract =
        AuthorityContract::new("camp_ai_harbor", AuthorityMode::HumanKp, 1).unwrap();
    let mut store = EventStore::default();
    assert_eq!(
        realtime_room_sync_impl::commit_realtime_room_sync_impl_decision(
            &mut store,
            &wrong_contract,
            &command(allowed.clone()),
            allowed,
        )
        .unwrap_err()
        .code(),
        "AUTHORITY_CONTRACT_MUTATION"
    );
    assert!(store.events().is_empty());

    let denied = decision(
        "decision_b014_realtime_tool",
        ToolRequest::formal(RuntimeAgent::AtmosphereWriter, RuntimeTool::ChangeScene),
    );
    let mut store = EventStore::default();
    assert_eq!(
        realtime_room_sync_impl::commit_realtime_room_sync_impl_decision(
            &mut store,
            &contract,
            &command(denied.clone()),
            denied,
        )
        .unwrap_err()
        .code(),
        "AGENT_TOOL_NOT_ALLOWED"
    );
    assert!(store.events().is_empty());

    let direct = decision(
        "decision_b014_realtime_direct",
        ToolRequest::formal(
            RuntimeAgent::AiKeeperOrchestrator,
            RuntimeTool::RequestSkillCheck,
        ),
    );
    let mut direct_command = command(direct.clone());
    direct_command.write_path = FormalWritePath::DirectAgent;
    let mut store = EventStore::default();
    assert_eq!(
        realtime_room_sync_impl::commit_realtime_room_sync_impl_decision(
            &mut store,
            &contract,
            &direct_command,
            direct,
        )
        .unwrap_err()
        .code(),
        "AGENT_DIRECT_STATE_WRITE_FORBIDDEN"
    );
    assert!(store.events().is_empty());
}
