use trpg_runtime::runtime_state_machines::{
    RuntimeAgent, RuntimeDecision, RuntimeEventPayload, RuntimeModule, RuntimeTool, ToolRequest,
    RUNTIME_MODULES,
};
use trpg_runtime::session_runtime_impl;
use trpg_runtime::{
    ActorRole, AuthorityContract, AuthorityMode, CommandEnvelope, EntityId, EventStore,
    FormalWritePath, PrincipalScope, Visibility, VisibilityLabel,
};

fn decision(decision_id: &str, request: ToolRequest) -> RuntimeDecision {
    RuntimeDecision::new(decision_id, "B014 session decision", request).unwrap()
}

fn command(payload: RuntimeDecision) -> CommandEnvelope<RuntimeDecision> {
    trpg_test_support::governed_command!(payload, ActorRole::Workflow, AuthorityMode::AiKp)
}

fn string_command(
    payload: &str,
    expected_version: u64,
    idempotency_key: &str,
) -> CommandEnvelope<String> {
    let mut command = trpg_test_support::governed_command!(
        payload.to_owned(),
        ActorRole::Workflow,
        AuthorityMode::AiKp
    );
    command.command_id =
        EntityId::new(format!("command_{idempotency_key}")).expect("valid command id");
    command.idempotency_key = idempotency_key.to_owned();
    command.expected_version = expected_version;
    command.visibility = Visibility::new(VisibilityLabel::KeeperOnly);
    command
}

#[test]
fn session_runtime_impl_preserves_governed_decision_event_contract() {
    assert!(RUNTIME_MODULES.contains(&RuntimeModule::SessionRuntimeImpl));

    let request = ToolRequest::formal(
        RuntimeAgent::AiKeeperOrchestrator,
        RuntimeTool::RequestSkillCheck,
    );
    let decision = decision("decision_b014_session", request);
    let mut command = command(decision.clone());
    command.visibility = Visibility::new(VisibilityLabel::KeeperOnly);
    let contract = AuthorityContract::new("camp_ai_harbor", AuthorityMode::AiKp, 1).unwrap();
    let mut store = EventStore::default();

    let events = session_runtime_impl::commit_session_runtime_impl_decision(
        &mut store, &contract, &command, decision,
    )
    .unwrap();
    let session_event = session_runtime_impl::start_session_runtime_impl(
        &mut store,
        &contract,
        &string_command("start session", 2, "idem_b014_session"),
        "session_b014",
    )
    .unwrap();

    assert_eq!(events[0].event_type, "ToolRequestApproved");
    assert_eq!(events[1].event_type, "DecisionCommitted");
    assert_eq!(session_event.event_type, "SessionStarted");
    assert!(store.replay_visible(&PrincipalScope::Public).is_empty());
    assert_eq!(store.replay_visible(&PrincipalScope::Keeper).len(), 3);
    for event in store.events() {
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
fn session_runtime_impl_denies_contract_tool_gate_and_direct_agent_write() {
    let contract = AuthorityContract::new("camp_ai_harbor", AuthorityMode::AiKp, 1).unwrap();
    assert_eq!(
        contract.fork(AuthorityMode::HumanKp, 1).unwrap_err().code(),
        "AUTHORITY_CONTRACT_MUTATION"
    );

    let allowed = decision(
        "decision_b014_session_contract",
        ToolRequest::formal(
            RuntimeAgent::AiKeeperOrchestrator,
            RuntimeTool::RequestSkillCheck,
        ),
    );
    let wrong_contract =
        AuthorityContract::new("camp_ai_harbor", AuthorityMode::HumanKp, 1).unwrap();
    let mut store = EventStore::default();
    assert_eq!(
        session_runtime_impl::commit_session_runtime_impl_decision(
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
        "decision_b014_session_tool",
        ToolRequest::formal(RuntimeAgent::AtmosphereWriter, RuntimeTool::ChangeScene),
    );
    let mut store = EventStore::default();
    assert_eq!(
        session_runtime_impl::commit_session_runtime_impl_decision(
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
        "decision_b014_session_direct",
        ToolRequest::formal(
            RuntimeAgent::AiKeeperOrchestrator,
            RuntimeTool::RequestSkillCheck,
        ),
    );
    let mut direct_command = command(direct.clone());
    direct_command.write_path = FormalWritePath::DirectAgent;
    let mut store = EventStore::default();
    assert_eq!(
        session_runtime_impl::commit_session_runtime_impl_decision(
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
