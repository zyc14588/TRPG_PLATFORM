use trpg_runtime::runtime_state_machines::{
    RuntimeAgent, RuntimeDecision, RuntimeEventPayload, RuntimeTool, ToolRequest,
};
use trpg_runtime::runtime_workflow_state_machines;
use trpg_runtime::{
    ActorRole, AuthorityMode, CommandEnvelope, EventStore, FormalWritePath, PrincipalScope,
    Visibility, VisibilityLabel,
};

fn decision(decision_id: &str, request: ToolRequest) -> RuntimeDecision {
    RuntimeDecision::new(decision_id, "B014 governed decision", request).unwrap()
}

fn command(payload: RuntimeDecision) -> CommandEnvelope<RuntimeDecision> {
    trpg_test_support::governed_command(payload, ActorRole::Workflow, AuthorityMode::AiKp)
}

#[test]
fn runtime_workflow_state_machines_preserves_governed_decision_event_contract() {
    assert_eq!(
        trpg_test_support::normalized_prompt_id("trpg-runtime", "runtime_workflow_state_machines"),
        "CODEX-0377-03-RUNTIME-ORCHESTRATION-fc718c91e6"
    );
    for module in [
        "runtime_workflow_state_machines",
        "capability_layer_impl",
        "pending_decision_impl",
        "realtime_room_sync_impl",
        "saga_transaction_impl",
        "scheduler_service_impl",
        "session_runtime_impl",
        "workflow_engine_impl",
    ] {
        trpg_test_support::assert_normalized_product_module("trpg-runtime", module);
    }

    let request = ToolRequest::formal(
        RuntimeAgent::AiKeeperOrchestrator,
        RuntimeTool::RequestSkillCheck,
    );
    let decision = decision("decision_b014_runtime_workflow", request);
    let mut command = command(decision.clone());
    command.visibility = Visibility::new(VisibilityLabel::KeeperOnly);
    let contract =
        trpg_test_support::authority_contract("camp_ai_harbor", AuthorityMode::AiKp, 1).unwrap();
    let mut store = EventStore::default();

    let events = runtime_workflow_state_machines::commit_runtime_workflow_state_machine_decision(
        &mut store, &contract, &command, decision,
    )
    .unwrap();

    assert_eq!(store.events().len(), 2);
    assert_eq!(events[0].event_type, "ToolRequestApproved");
    assert_eq!(events[1].event_type, "DecisionCommitted");
    for event in &events {
        assert_eq!(event.authority_contract_version, 1);
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
            for record in ["DecisionRecord", "DiceRoll", "GameEvent"] {
                assert!(linked_records.contains(&record));
            }
            for field in ["prompt_version", "model_provider", "context_hash"] {
                assert!(audit_fields.contains(&field));
            }
        }
        other => panic!("unexpected payload: {other:?}"),
    }
    assert!(
        runtime_workflow_state_machines::replay_runtime_workflow_state_machine_events(
            &store,
            &PrincipalScope::Public,
        )
        .is_empty()
    );
    assert_eq!(
        runtime_workflow_state_machines::replay_runtime_workflow_state_machine_events(
            &store,
            &PrincipalScope::Keeper,
        )
        .len(),
        2
    );
}

#[test]
fn runtime_workflow_state_machines_denies_contract_tool_gate_and_direct_agent_write() {
    let contract =
        trpg_test_support::authority_contract("camp_ai_harbor", AuthorityMode::AiKp, 1).unwrap();
    assert_eq!(
        contract.fork(AuthorityMode::HumanKp, 1).unwrap_err().code(),
        "AUTHORITY_CONTRACT_MUTATION"
    );

    let allowed = decision(
        "decision_b014_runtime_contract",
        ToolRequest::formal(
            RuntimeAgent::AiKeeperOrchestrator,
            RuntimeTool::RequestSkillCheck,
        ),
    );
    let wrong_contract =
        trpg_test_support::authority_contract("camp_ai_harbor", AuthorityMode::HumanKp, 1).unwrap();
    let mut store = EventStore::default();
    assert_eq!(
        runtime_workflow_state_machines::commit_runtime_workflow_state_machine_decision(
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
        "decision_b014_runtime_tool_gate",
        ToolRequest::formal(RuntimeAgent::AtmosphereWriter, RuntimeTool::ChangeScene),
    );
    let mut store = EventStore::default();
    assert_eq!(
        runtime_workflow_state_machines::commit_runtime_workflow_state_machine_decision(
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
        "decision_b014_runtime_direct",
        ToolRequest::formal(
            RuntimeAgent::AiKeeperOrchestrator,
            RuntimeTool::RequestSkillCheck,
        ),
    );
    let mut direct_command = command(direct.clone());
    direct_command.write_path = FormalWritePath::DirectAgent;
    let mut store = EventStore::default();
    assert_eq!(
        runtime_workflow_state_machines::commit_runtime_workflow_state_machine_decision(
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
