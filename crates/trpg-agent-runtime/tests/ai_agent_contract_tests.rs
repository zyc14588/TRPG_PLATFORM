pub mod common;

use std::sync::Arc;

use trpg_agent_runtime::ai_agent;
use trpg_agent_runtime::{
    ActorRole, AgentDecision, AgentDecisionCommitter, AgentEventPayload, AgentKind, AgentTool,
    AgentToolExecutionOutput, AgentToolExecutor, AuthorityMode, CommandEnvelope, ToolRequest,
};

struct SuccessfulToolExecutor;

impl AgentToolExecutor for SuccessfulToolExecutor {
    fn execute(
        &self,
        decision: &AgentDecision,
    ) -> trpg_agent_runtime::agent_runtime::AgentResult<AgentToolExecutionOutput> {
        Ok(AgentToolExecutionOutput {
            execution_id: format!("execution_{}", decision.decision_id.as_str()),
            result_hash: "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                .to_owned(),
        })
    }
}

fn committer(contract: &trpg_agent_runtime::AuthorityContract) -> AgentDecisionCommitter {
    AgentDecisionCommitter::with_tool_executor(
        trpg_test_support::identity_verifier_for_contract(contract),
        Arc::new(SuccessfulToolExecutor),
    )
    .unwrap()
}

fn ai_kp_command(payload: AgentDecision) -> CommandEnvelope<AgentDecision> {
    trpg_test_support::governed_command(payload, ActorRole::Workflow, AuthorityMode::AiKp)
}

#[test]
fn ai_agent_commits_only_through_event_store_with_provenance() {
    assert_eq!(
        trpg_test_support::normalized_prompt_id("trpg-agent-runtime", "ai_agent"),
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
    let contract =
        trpg_test_support::authority_contract("campaign_b018_ai_agent", AuthorityMode::AiKp, 1)
            .unwrap();
    let authentication =
        trpg_test_support::ai_keeper_authentication(contract.campaign_id().as_str());
    let decision = AgentDecision::new(
        "decision_b018_ai_agent",
        request,
        "Spot Hidden",
        &authentication,
    )
    .unwrap();
    let command = trpg_test_support::governed_command_for_contract(
        &contract,
        decision.clone(),
        ActorRole::Workflow,
    );
    let (mut store, audit) = common::audited_store_with_handle(&contract);
    let committer = committer(&contract);

    let mut unaudited_store = trpg_agent_runtime::AgentEventStore::default();
    let unaudited_error = ai_agent::submit_ai_agent_decision(
        &committer,
        &mut unaudited_store,
        &command,
        &trpg_test_support::workflow_authentication(),
        decision.clone(),
        2,
    )
    .unwrap_err();
    assert_eq!(unaudited_error.code(), "AUDIT_INTEGRITY_VIOLATION");
    assert!(unaudited_store.events().is_empty());

    let events = ai_agent::submit_ai_agent_decision(
        &committer,
        &mut store,
        &command,
        &trpg_test_support::workflow_authentication(),
        decision,
        2,
    )
    .unwrap();

    assert_eq!(events.len(), 3);
    assert_eq!(store.events().len(), 3);
    assert_eq!(events[0].event_type, "ToolRequestApproved");
    assert_eq!(events[1].event_type, "ToolExecutionSucceeded");
    assert_eq!(events[2].event_type, "DecisionCommitted");
    assert_eq!(events[2].fact_provenance, command.fact_provenance);
    match &events[2].payload {
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
    let audit_records = audit.verify().unwrap();
    assert_eq!(audit_records.len(), 1);
    assert_eq!(audit_records[0].actor_id, "ai_kp_local_level4");
    assert_eq!(audit_records[0].action, "write_official_state");
    assert_eq!(audit_records[0].requested_role, "ai_keeper_orchestrator");
}

#[test]
fn ai_agent_exact_retry_returns_original_formal_events() {
    let request = ToolRequest::formal(
        AgentKind::AiKeeperOrchestrator,
        AgentTool::RequestSkillCheck,
    );
    let contract =
        trpg_test_support::authority_contract("campaign_b018_ai_agent", AuthorityMode::AiKp, 1)
            .unwrap();
    let authentication =
        trpg_test_support::ai_keeper_authentication(contract.campaign_id().as_str());
    let decision = AgentDecision::new(
        "decision_b018_ai_agent_retry",
        request,
        "Spot Hidden",
        &authentication,
    )
    .unwrap();
    let command = trpg_test_support::governed_command_for_contract(
        &contract,
        decision.clone(),
        ActorRole::Workflow,
    );
    let mut store = common::audited_store(&contract);
    let committer = committer(&contract);

    let first = ai_agent::submit_ai_agent_decision(
        &committer,
        &mut store,
        &command,
        &trpg_test_support::workflow_authentication(),
        decision.clone(),
        2,
    )
    .unwrap();
    let replayed = ai_agent::submit_ai_agent_decision(
        &committer,
        &mut store,
        &command,
        &trpg_test_support::workflow_authentication(),
        decision,
        2,
    )
    .expect("exact retry returns the first formal result");

    assert_eq!(replayed, first);
    assert_eq!(store.events().len(), 3);
}

#[test]
fn cold_retry_returns_canonical_event_identities() {
    let request = ToolRequest::formal(
        AgentKind::AiKeeperOrchestrator,
        AgentTool::RequestSkillCheck,
    );
    let contract = trpg_test_support::authority_contract(
        "campaign_b018_ai_agent_cold_retry",
        AuthorityMode::AiKp,
        1,
    )
    .unwrap();
    let authentication =
        trpg_test_support::ai_keeper_authentication(contract.campaign_id().as_str());
    let decision = AgentDecision::new(
        "decision_b018_ai_agent_cold_retry",
        request,
        "Spot Hidden",
        &authentication,
    )
    .unwrap();
    let command = trpg_test_support::governed_command_for_contract(
        &contract,
        decision.clone(),
        ActorRole::Workflow,
    );
    let canonical = trpg_test_support::test_canonical_commit_port();
    let (mut first_store, _) = common::audited_store_with_canonical(&contract, canonical.clone());
    let committer = committer(&contract);

    let first = ai_agent::submit_ai_agent_decision(
        &committer,
        &mut first_store,
        &command,
        &trpg_test_support::workflow_authentication(),
        decision.clone(),
        2,
    )
    .unwrap();

    std::thread::sleep(std::time::Duration::from_millis(2));
    let (mut restarted_store, _) = common::audited_store_with_canonical(&contract, canonical);
    let replayed = ai_agent::submit_ai_agent_decision(
        &committer,
        &mut restarted_store,
        &command,
        &trpg_test_support::workflow_authentication(),
        decision,
        2,
    )
    .expect("a cold exact retry must materialize the durable event identities");

    assert_eq!(replayed, first);
    assert_eq!(restarted_store.events(), first.as_slice());
}

#[test]
fn corrupt_batch_receipt_is_rejected_without_partial_local_events() {
    let request = ToolRequest::formal(
        AgentKind::AiKeeperOrchestrator,
        AgentTool::RequestSkillCheck,
    );
    let contract = trpg_test_support::authority_contract(
        "campaign_b018_ai_agent_corrupt_receipt",
        AuthorityMode::AiKp,
        1,
    )
    .unwrap();
    let authentication =
        trpg_test_support::ai_keeper_authentication(contract.campaign_id().as_str());
    let decision = AgentDecision::new(
        "decision_b018_ai_agent_corrupt_receipt",
        request,
        "Listen",
        &authentication,
    )
    .unwrap();
    let command = trpg_test_support::governed_command_for_contract(
        &contract,
        decision.clone(),
        ActorRole::Workflow,
    );
    let (mut store, _) = common::audited_store_with_canonical(
        &contract,
        trpg_test_support::corrupt_second_event_receipt_port(),
    );
    let committer = committer(&contract);

    let error = ai_agent::submit_ai_agent_decision(
        &committer,
        &mut store,
        &command,
        &trpg_test_support::workflow_authentication(),
        decision,
        2,
    )
    .unwrap_err();

    assert_eq!(error.code(), "AUDIT_INTEGRITY_VIOLATION");
    assert!(store.events().is_empty());
}

#[test]
fn ai_agent_rejects_authority_contract_mismatch_without_event_write() {
    let request = ToolRequest::formal(
        AgentKind::AiKeeperOrchestrator,
        AgentTool::RequestSkillCheck,
    );
    let authentication = trpg_test_support::ai_keeper_authentication("camp_ai_harbor");
    let decision = AgentDecision::new(
        "decision_b018_ai_agent_mismatch",
        request,
        "Listen",
        &authentication,
    )
    .unwrap();
    let command = ai_kp_command(decision.clone());
    let contract =
        trpg_test_support::authority_contract("campaign_b018_ai_agent", AuthorityMode::HumanKp, 1)
            .unwrap();
    let mut store = common::audited_store(&contract);
    let committer =
        AgentDecisionCommitter::new(trpg_test_support::identity_verifier_for_contract(&contract))
            .unwrap();

    let error = ai_agent::submit_ai_agent_decision(
        &committer,
        &mut store,
        &command,
        &trpg_test_support::workflow_authentication(),
        decision,
        2,
    )
    .unwrap_err();

    assert_eq!(error.code(), "AUTHORITY_VIOLATION");
    assert!(store.events().is_empty());
}
