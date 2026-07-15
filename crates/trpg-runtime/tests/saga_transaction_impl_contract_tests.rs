mod common;

use trpg_runtime::runtime_state_machines::{
    RuntimeAgent, RuntimeDecision, RuntimeEventPayload, RuntimeTool, ToolRequest,
};
use trpg_runtime::saga_transaction_impl::{self, SagaTransactionImplCompensation};
use trpg_runtime::{
    ActorRole, AuthorityMode, CommandEnvelope, EntityId, FormalWritePath, PrincipalScope,
    Visibility, VisibilityLabel,
};

fn decision(decision_id: &str, request: ToolRequest) -> RuntimeDecision {
    RuntimeDecision::new(decision_id, "B014 saga decision", request).unwrap()
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
fn saga_transaction_impl_preserves_governed_decision_event_contract() {
    assert_eq!(
        trpg_test_support::normalized_prompt_id("trpg-runtime", "saga_transaction_impl"),
        "CODEX-0389-03-RUNTIME-ORCHESTRATION-1b60a8b386"
    );
    trpg_test_support::assert_normalized_product_module("trpg-runtime", "saga_transaction_impl");

    let request = ToolRequest::formal(
        RuntimeAgent::AiKeeperOrchestrator,
        RuntimeTool::RequestSkillCheck,
    );
    let decision = decision("decision_b014_saga", request);
    let mut command = command(decision.clone());
    command.visibility = Visibility::new(VisibilityLabel::KeeperOnly);
    let contract =
        trpg_test_support::authority_contract("camp_ai_harbor", AuthorityMode::AiKp, 1).unwrap();
    let mut store = common::audited_store();

    let events = saga_transaction_impl::commit_saga_transaction_impl_decision(
        &mut store, &contract, &command, decision,
    )
    .unwrap();

    assert_eq!(store.events().len(), 2);
    assert_eq!(events[0].event_type, "ToolRequestApproved");
    assert_eq!(events[1].event_type, "DecisionCommitted");
    let saga_event = saga_transaction_impl::compensate_saga_transaction_impl(
        &mut store,
        &contract,
        &string_command("compensate saga", 2, "idem_b014_saga"),
        SagaTransactionImplCompensation::new("saga_b014").unwrap(),
    )
    .unwrap();
    assert_eq!(saga_event.event_type, "SagaCompensated");
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
            assert!(linked_records.contains(&"DiceRoll"));
            assert!(audit_fields.contains(&"model_provider"));
        }
        other => panic!("unexpected payload: {other:?}"),
    }
}

#[test]
fn saga_transaction_impl_denies_contract_tool_gate_and_direct_agent_write() {
    let contract =
        trpg_test_support::authority_contract("camp_ai_harbor", AuthorityMode::AiKp, 1).unwrap();
    assert_eq!(
        contract.fork(AuthorityMode::HumanKp, 1).unwrap_err().code(),
        "AUTHORITY_CONTRACT_MUTATION"
    );

    let allowed = decision(
        "decision_b014_saga_contract",
        ToolRequest::formal(
            RuntimeAgent::AiKeeperOrchestrator,
            RuntimeTool::RequestSkillCheck,
        ),
    );
    let wrong_contract =
        trpg_test_support::authority_contract("camp_ai_harbor", AuthorityMode::HumanKp, 1).unwrap();
    let mut store = common::audited_store();
    assert_eq!(
        saga_transaction_impl::commit_saga_transaction_impl_decision(
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
        "decision_b014_saga_tool",
        ToolRequest::formal(RuntimeAgent::AtmosphereWriter, RuntimeTool::ChangeScene),
    );
    let mut store = common::audited_store();
    assert_eq!(
        saga_transaction_impl::commit_saga_transaction_impl_decision(
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
        "decision_b014_saga_direct",
        ToolRequest::formal(
            RuntimeAgent::AiKeeperOrchestrator,
            RuntimeTool::RequestSkillCheck,
        ),
    );
    let mut direct_command = command(direct.clone());
    direct_command.write_path = FormalWritePath::DirectAgent;
    let mut store = common::audited_store();
    assert_eq!(
        saga_transaction_impl::commit_saga_transaction_impl_decision(
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
