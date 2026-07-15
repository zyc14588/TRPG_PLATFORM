mod common;

use trpg_runtime::capability_layer_impl;
use trpg_runtime::runtime_state_machines::{
    RuntimeAgent, RuntimeDecision, RuntimeEventPayload, RuntimeTool, ToolRequest,
};
use trpg_runtime::{
    ActorRole, AuthorityMode, CommandEnvelope, FormalWritePath, PrincipalScope, Visibility,
    VisibilityLabel,
};

fn decision(decision_id: &str, request: ToolRequest) -> RuntimeDecision {
    RuntimeDecision::new(decision_id, "B014 capability decision", request).unwrap()
}

fn command(payload: RuntimeDecision) -> CommandEnvelope<RuntimeDecision> {
    trpg_test_support::governed_command(payload, ActorRole::Workflow, AuthorityMode::AiKp)
}

#[test]
fn capability_layer_impl_preserves_governed_decision_event_contract() {
    assert_eq!(
        trpg_test_support::normalized_prompt_id("trpg-runtime", "capability_layer_impl"),
        "CODEX-0386-03-RUNTIME-ORCHESTRATION-027bb089fe"
    );
    trpg_test_support::assert_normalized_product_module("trpg-runtime", "capability_layer_impl");

    let request = ToolRequest::formal(
        RuntimeAgent::AiKeeperOrchestrator,
        RuntimeTool::RequestSkillCheck,
    );
    assert!(
        capability_layer_impl::approve_capability_layer_impl_tool(&AuthorityMode::AiKp, &request,)
            .unwrap()
            .allowed
    );
    let decision = decision("decision_b014_capability", request);
    let mut command = command(decision.clone());
    command.visibility = Visibility::new(VisibilityLabel::KeeperOnly);
    let contract =
        trpg_test_support::authority_contract("camp_ai_harbor", AuthorityMode::AiKp, 1).unwrap();
    let mut store = common::audited_store();

    let events = capability_layer_impl::commit_capability_layer_impl_decision(
        &mut store, &contract, &command, decision,
    )
    .unwrap();

    assert_eq!(store.events().len(), 2);
    assert_eq!(events[0].event_type, "ToolRequestApproved");
    assert_eq!(events[1].event_type, "DecisionCommitted");
    assert!(store.replay_visible(&PrincipalScope::Public).is_empty());
    assert_eq!(store.replay_visible(&PrincipalScope::Keeper).len(), 2);
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
fn capability_layer_impl_denies_contract_tool_gate_and_direct_agent_write() {
    let contract =
        trpg_test_support::authority_contract("camp_ai_harbor", AuthorityMode::AiKp, 1).unwrap();
    assert_eq!(
        contract.fork(AuthorityMode::HumanKp, 1).unwrap_err().code(),
        "AUTHORITY_CONTRACT_MUTATION"
    );

    let allowed = decision(
        "decision_b014_capability_contract",
        ToolRequest::formal(
            RuntimeAgent::AiKeeperOrchestrator,
            RuntimeTool::RequestSkillCheck,
        ),
    );
    let wrong_contract =
        trpg_test_support::authority_contract("camp_ai_harbor", AuthorityMode::HumanKp, 1).unwrap();
    let mut store = common::audited_store();
    assert_eq!(
        capability_layer_impl::commit_capability_layer_impl_decision(
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

    let denied_request =
        ToolRequest::formal(RuntimeAgent::AtmosphereWriter, RuntimeTool::ChangeScene);
    assert_eq!(
        capability_layer_impl::approve_capability_layer_impl_tool(
            &AuthorityMode::AiKp,
            &denied_request,
        )
        .unwrap_err()
        .code(),
        "AGENT_TOOL_NOT_ALLOWED"
    );
    let denied = decision("decision_b014_capability_tool", denied_request);
    let mut store = common::audited_store();
    assert_eq!(
        capability_layer_impl::commit_capability_layer_impl_decision(
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
        "decision_b014_capability_direct",
        ToolRequest::formal(
            RuntimeAgent::AiKeeperOrchestrator,
            RuntimeTool::RequestSkillCheck,
        ),
    );
    let mut direct_command = command(direct.clone());
    direct_command.write_path = FormalWritePath::DirectAgent;
    let mut store = common::audited_store();
    assert_eq!(
        capability_layer_impl::commit_capability_layer_impl_decision(
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
