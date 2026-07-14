use trpg_runtime::runtime_state_machines::{
    RuntimeAgent, RuntimeDecision, RuntimeEventPayload, RuntimeTool, ToolRequest,
};
use trpg_runtime::workflow_engine_impl;
use trpg_runtime::{
    ActorRole, AuthorityMode, CommandEnvelope, EntityId, EventStore, FormalWritePath,
    PrincipalScope, Visibility, VisibilityLabel,
};

fn decision(decision_id: &str, request: ToolRequest) -> RuntimeDecision {
    RuntimeDecision::new(decision_id, "B014 workflow decision", request).unwrap()
}

fn command(payload: RuntimeDecision) -> CommandEnvelope<RuntimeDecision> {
    trpg_test_support::governed_command(payload, ActorRole::Workflow, AuthorityMode::AiKp)
}

fn string_command(
    payload: &str,
    expected_version: u64,
    idempotency_key: &str,
) -> CommandEnvelope<String> {
    let mut command = trpg_test_support::governed_command(
        payload.to_owned(),
        ActorRole::Workflow,
        AuthorityMode::AiKp,
    );
    command.command_id =
        EntityId::new(format!("command_{idempotency_key}")).expect("valid command id");
    command.idempotency_key = idempotency_key.to_owned();
    command.expected_version = expected_version;
    command.visibility = Visibility::new(VisibilityLabel::KeeperOnly);
    command
}

#[test]
fn workflow_engine_impl_preserves_governed_decision_event_contract() {
    assert_eq!(
        trpg_test_support::normalized_prompt_id("trpg-runtime", "workflow_engine_impl"),
        "CODEX-0392-03-RUNTIME-ORCHESTRATION-1cb6fb735e"
    );
    trpg_test_support::assert_normalized_product_module("trpg-runtime", "workflow_engine_impl");

    let request = ToolRequest::formal(
        RuntimeAgent::AiKeeperOrchestrator,
        RuntimeTool::RequestSkillCheck,
    );
    let decision = decision("decision_b014_workflow", request);
    let mut command = command(decision.clone());
    command.visibility = Visibility::new(VisibilityLabel::KeeperOnly);
    let contract =
        trpg_test_support::authority_contract("camp_ai_harbor", AuthorityMode::AiKp, 1).unwrap();
    let mut store = EventStore::default();

    let events = workflow_engine_impl::commit_workflow_engine_impl_decision(
        &mut store, &contract, &command, decision,
    )
    .unwrap();
    let workflow_event = workflow_engine_impl::advance_workflow_engine_impl(
        &mut store,
        &contract,
        &string_command("advance workflow", 2, "idem_b014_workflow"),
        "workflow_b014",
    )
    .unwrap();

    assert_eq!(events[0].event_type, "ToolRequestApproved");
    assert_eq!(events[1].event_type, "DecisionCommitted");
    assert_eq!(workflow_event.event_type, "WorkflowAdvanced");
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
            assert!(linked_records.contains(&"DiceRoll"));
            assert!(linked_records.contains(&"GameEvent"));
            assert!(audit_fields.contains(&"model_provider"));
            assert!(audit_fields.contains(&"context_hash"));
        }
        other => panic!("unexpected payload: {other:?}"),
    }
}

#[test]
fn workflow_engine_impl_denies_contract_tool_gate_and_direct_agent_write() {
    let contract =
        trpg_test_support::authority_contract("camp_ai_harbor", AuthorityMode::AiKp, 1).unwrap();
    assert_eq!(
        contract.fork(AuthorityMode::HumanKp, 1).unwrap_err().code(),
        "AUTHORITY_CONTRACT_MUTATION"
    );

    let allowed = decision(
        "decision_b014_workflow_contract",
        ToolRequest::formal(
            RuntimeAgent::AiKeeperOrchestrator,
            RuntimeTool::RequestSkillCheck,
        ),
    );
    let wrong_contract =
        trpg_test_support::authority_contract("camp_ai_harbor", AuthorityMode::HumanKp, 1).unwrap();
    let mut store = EventStore::default();
    assert_eq!(
        workflow_engine_impl::commit_workflow_engine_impl_decision(
            &mut store,
            &wrong_contract,
            &command(allowed.clone()),
            allowed,
        )
        .unwrap_err()
        .code(),
        "AUTHORITY_VIOLATION"
    );
    assert!(store.events().is_empty());

    let denied = decision(
        "decision_b014_workflow_tool",
        ToolRequest::formal(RuntimeAgent::AtmosphereWriter, RuntimeTool::ChangeScene),
    );
    let mut store = EventStore::default();
    assert_eq!(
        workflow_engine_impl::commit_workflow_engine_impl_decision(
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
        "decision_b014_workflow_direct",
        ToolRequest::formal(
            RuntimeAgent::AiKeeperOrchestrator,
            RuntimeTool::RequestSkillCheck,
        ),
    );
    let mut direct_command = command(direct.clone());
    direct_command.write_path = FormalWritePath::DirectAgent;
    let mut store = EventStore::default();
    assert_eq!(
        workflow_engine_impl::commit_workflow_engine_impl_decision(
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
