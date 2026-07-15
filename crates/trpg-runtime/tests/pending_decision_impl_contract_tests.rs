mod common;

use trpg_runtime::pending_decision_impl;
use trpg_runtime::runtime_state_machines::{
    PendingDecisionStatus, RuntimeAgent, RuntimeDecision, RuntimeEventPayload, RuntimeTool,
    ToolRequest,
};
use trpg_runtime::{
    ActorRole, AuthorityMode, CommandEnvelope, FormalWritePath, Visibility, VisibilityLabel,
};

fn decision(decision_id: &str, request: ToolRequest) -> RuntimeDecision {
    RuntimeDecision::new(decision_id, "B014 pending decision", request).unwrap()
}

fn command(payload: RuntimeDecision) -> CommandEnvelope<RuntimeDecision> {
    trpg_test_support::governed_command(payload, ActorRole::Workflow, AuthorityMode::AiKp)
}

#[test]
fn pending_decision_impl_preserves_governed_decision_event_contract() {
    assert_eq!(
        trpg_test_support::normalized_prompt_id("trpg-runtime", "pending_decision_impl"),
        "CODEX-0387-03-RUNTIME-ORCHESTRATION-ff36c2cdcf"
    );
    trpg_test_support::assert_normalized_product_module("trpg-runtime", "pending_decision_impl");

    let request = ToolRequest::formal(
        RuntimeAgent::AiKeeperOrchestrator,
        RuntimeTool::RequestSkillCheck,
    );
    let decision = decision("decision_b014_pending", request);
    let pending =
        pending_decision_impl::open_pending_decision_impl(&AuthorityMode::AiKp, decision.clone());
    assert_eq!(pending.status, PendingDecisionStatus::ReadyToCommit);
    let mut command = command(decision.clone());
    command.visibility = Visibility::new(VisibilityLabel::KeeperOnly);
    let contract =
        trpg_test_support::authority_contract("camp_ai_harbor", AuthorityMode::AiKp, 1).unwrap();
    let mut store = common::audited_store(&contract);

    let events = pending_decision_impl::commit_pending_decision_impl(
        &mut store,
        &contract,
        &command,
        &trpg_test_support::workflow_authentication(),
        decision,
        2,
    )
    .unwrap();

    assert_eq!(store.events().len(), 2);
    assert_eq!(events[0].event_type, "ToolRequestApproved");
    assert_eq!(events[1].event_type, "DecisionCommitted");
    let player = trpg_test_support::player_replay_authorization(&contract);
    let system = trpg_test_support::system_replay_authorization(&contract);
    assert!(store.replay_visible(&player, 206).unwrap().is_empty());
    assert_eq!(store.replay_visible(&system, 206).unwrap().len(), 2);
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
            assert!(linked_records.contains(&"GameEvent"));
            assert!(audit_fields.contains(&"prompt_version"));
            assert!(audit_fields.contains(&"model_provider"));
        }
        other => panic!("unexpected payload: {other:?}"),
    }
}

#[test]
fn pending_decision_impl_denies_contract_tool_gate_and_direct_agent_write() {
    let contract =
        trpg_test_support::authority_contract("camp_ai_harbor", AuthorityMode::AiKp, 1).unwrap();
    assert_eq!(
        contract.fork(AuthorityMode::HumanKp, 1).unwrap_err().code(),
        "AUTHORITY_CONTRACT_MUTATION"
    );

    let allowed = decision(
        "decision_b014_pending_contract",
        ToolRequest::formal(
            RuntimeAgent::AiKeeperOrchestrator,
            RuntimeTool::RequestSkillCheck,
        ),
    );
    let wrong_contract =
        trpg_test_support::authority_contract("camp_ai_harbor", AuthorityMode::HumanKp, 1).unwrap();
    let mut store = common::audited_store(&contract);
    assert_eq!(
        pending_decision_impl::commit_pending_decision_impl(
            &mut store,
            &wrong_contract,
            &command(allowed.clone()),
            &trpg_test_support::workflow_authentication(),
            allowed,
            2,
        )
        .unwrap_err()
        .code(),
        "AUTHORITY_VIOLATION"
    );
    assert!(store.events().is_empty());

    let denied = decision(
        "decision_b014_pending_tool",
        ToolRequest::formal(RuntimeAgent::AtmosphereWriter, RuntimeTool::ChangeScene),
    );
    let mut store = common::audited_store(&contract);
    assert_eq!(
        pending_decision_impl::commit_pending_decision_impl(
            &mut store,
            &contract,
            &command(denied.clone()),
            &trpg_test_support::workflow_authentication(),
            denied,
            2,
        )
        .unwrap_err()
        .code(),
        "AGENT_TOOL_NOT_ALLOWED"
    );
    assert!(store.events().is_empty());

    let direct = decision(
        "decision_b014_pending_direct",
        ToolRequest::formal(
            RuntimeAgent::AiKeeperOrchestrator,
            RuntimeTool::RequestSkillCheck,
        ),
    );
    let mut direct_command = command(direct.clone());
    direct_command.write_path = FormalWritePath::DirectAgent;
    let mut store = common::audited_store(&contract);
    assert_eq!(
        pending_decision_impl::commit_pending_decision_impl(
            &mut store,
            &contract,
            &direct_command,
            &trpg_test_support::workflow_authentication(),
            direct,
            2,
        )
        .unwrap_err()
        .code(),
        "AGENT_DIRECT_STATE_WRITE_FORBIDDEN"
    );
    assert!(store.events().is_empty());
}
