use trpg_runtime::adr_0007_internal_workflow_vs_temporal;
use trpg_runtime::capability_layer;
use trpg_runtime::capability_layer_tool_grant;
use trpg_runtime::capability_tool_grant;
use trpg_runtime::pending_decision;
use trpg_runtime::realtime_room_sync;
use trpg_runtime::realtime_runtime_binding;
use trpg_runtime::runtime_pending_decision;
use trpg_runtime::runtime_state_machines::{
    PendingDecisionStatus, RuntimeAgent, RuntimeDecision, RuntimeError, RuntimeEventPayload,
    RuntimeTool, ToolRequest,
};
use trpg_runtime::runtime_workflow_engine;
use trpg_runtime::saga_transaction::{self, SagaCompensation};
use trpg_runtime::scheduler_service::{self, ScheduledRuntimeTask};
use trpg_runtime::session_runtime;
use trpg_runtime::workflow_engine;
use trpg_runtime::{
    ActorRole, AuthorityMode, CommandEnvelope, EntityId, EventStore, FormalWritePath,
    PrincipalScope, Visibility, VisibilityLabel,
};

const S06_STAGE_FIXTURE: &str =
    include_str!("../../../fixtures/stages/S06_stage_acceptance_fixture.v1.json.md");
const S06_DETAILED_FIXTURE: &str = include_str!(
    "../../../fixtures/stages/detailed/S06_decision_pipeline_commit_expected.current.json.md"
);

fn runtime_command(
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
    command
}

#[test]
fn batch_012_maps_primary_modules_to_current_safe_outputs() {
    for module in [
        "capability_tool_grant",
        "pending_decision",
        "realtime_runtime_binding",
        "runtime_state_machines",
        "saga_transaction",
        "scheduler_service",
        "session_runtime",
        "workflow_engine",
        "adr_0007_internal_workflow_vs_temporal",
        "capability_layer_tool_grant",
        "runtime_workflow_engine",
        "capability_layer",
        "realtime_room_sync",
        "runtime_pending_decision",
    ] {
        trpg_test_support::assert_normalized_product_module("trpg-runtime", module);
    }
}

#[test]
fn s06_stage_fixtures_are_bound_to_runtime_assertions() {
    assert!(S06_STAGE_FIXTURE.contains("\"stage\": \"S06\""));
    assert!(S06_STAGE_FIXTURE.contains("\"p1_findings_allowed\": 0"));
    assert!(S06_STAGE_FIXTURE.contains("docs/reports/stages/S06_TEST_RESULTS.md"));

    assert!(S06_DETAILED_FIXTURE.contains("\"type\": \"ToolRequestApproved\""));
    assert!(S06_DETAILED_FIXTURE.contains("\"type\": \"DecisionCommitted\""));
    assert!(S06_DETAILED_FIXTURE.contains("\"error\": \"AGENT_TOOL_NOT_ALLOWED\""));
    assert!(S06_DETAILED_FIXTURE.contains("\"error\": \"HUMAN_KP_AI_DRAFT_ONLY\""));
    assert!(
        S06_DETAILED_FIXTURE.contains("\"expected_error\": \"AGENT_DIRECT_STATE_WRITE_FORBIDDEN\"")
    );
    assert!(S06_DETAILED_FIXTURE.contains("\"tool_gate_required\""));
    assert!(S06_DETAILED_FIXTURE.contains("\"decision_commit_evented\""));
    assert!(S06_DETAILED_FIXTURE.contains("\"draft_only_human_kp_enforced\""));
}

#[test]
fn primary_prompt_outputs_expose_current_safe_prompt_ids() {
    assert_eq!(
        trpg_test_support::normalized_prompt_id("trpg-runtime", "capability_tool_grant"),
        "CODEX-0032-03-RUNTIME-ORCHESTRATION-20830a72ac"
    );
    assert_eq!(
        trpg_test_support::normalized_prompt_id("trpg-runtime", "pending_decision"),
        "CODEX-0033-03-RUNTIME-ORCHESTRATION-0d6882e9c6"
    );
    assert_eq!(
        trpg_test_support::normalized_prompt_id("trpg-runtime", "realtime_runtime_binding"),
        "CODEX-0034-03-RUNTIME-ORCHESTRATION-20e1521d8e"
    );
    assert_eq!(
        trpg_test_support::normalized_prompt_id(
            "trpg-runtime",
            "adr_0007_internal_workflow_vs_temporal"
        ),
        "CODEX-0335-03-RUNTIME-ORCHESTRATION-0ca4a1c995"
    );
    assert_eq!(
        trpg_test_support::normalized_prompt_id("trpg-runtime", "capability_layer_tool_grant"),
        "CODEX-0338-03-RUNTIME-ORCHESTRATION-d0fdce8770"
    );
    assert_eq!(
        trpg_test_support::normalized_prompt_id("trpg-runtime", "runtime_workflow_engine"),
        "CODEX-0344-03-RUNTIME-ORCHESTRATION-22393092aa"
    );
    assert_eq!(
        trpg_test_support::normalized_prompt_id("trpg-runtime", "capability_layer"),
        "CODEX-0346-03-RUNTIME-ORCHESTRATION-fc8679858e"
    );
    assert_eq!(
        trpg_test_support::normalized_prompt_id("trpg-runtime", "realtime_room_sync"),
        "CODEX-0347-03-RUNTIME-ORCHESTRATION-b0e055d98c"
    );
    assert_eq!(
        trpg_test_support::normalized_prompt_id("trpg-runtime", "runtime_pending_decision"),
        "CODEX-0349-03-RUNTIME-ORCHESTRATION-0b68fe8e4e"
    );
    assert_eq!(
        trpg_test_support::normalized_prompt_id("trpg-runtime", "saga_transaction"),
        "CODEX-0036-03-RUNTIME-ORCHESTRATION-12a9414c48"
    );
    assert_eq!(
        trpg_test_support::normalized_prompt_id("trpg-runtime", "scheduler_service"),
        "CODEX-0037-03-RUNTIME-ORCHESTRATION-c9bd0a0635"
    );
    assert_eq!(
        trpg_test_support::normalized_prompt_id("trpg-runtime", "session_runtime"),
        "CODEX-0038-03-RUNTIME-ORCHESTRATION-ec0e699332"
    );
    assert_eq!(
        trpg_test_support::normalized_prompt_id("trpg-runtime", "workflow_engine"),
        "CODEX-0039-03-RUNTIME-ORCHESTRATION-99d8270e66"
    );
}

#[test]
fn ai_kp_orchestrator_commits_decision_through_tool_and_event_log() {
    let request = ToolRequest::formal(
        RuntimeAgent::AiKeeperOrchestrator,
        RuntimeTool::RequestSkillCheck,
    );
    let decision =
        RuntimeDecision::new("decision_001", "Spot Hidden normal check", request).unwrap();
    let command = trpg_test_support::governed_command(
        decision.clone(),
        ActorRole::Workflow,
        AuthorityMode::AiKp,
    );
    let contract =
        trpg_test_support::authority_contract("camp_ai_harbor", AuthorityMode::AiKp, 1).unwrap();
    let mut store = EventStore::default();

    let events = runtime_workflow_engine::commit_runtime_workflow_decision(
        &mut store, &contract, &command, decision,
    )
    .unwrap();

    assert_eq!(events.len(), 2);
    assert_eq!(events[0].event_type, "ToolRequestApproved");
    assert_eq!(events[1].event_type, "DecisionCommitted");
    match &events[1].payload {
        RuntimeEventPayload::DecisionCommitted {
            linked_records,
            audit_fields,
            ..
        } => {
            assert!(linked_records.contains(&"DecisionRecord"));
            assert!(linked_records.contains(&"DiceRoll"));
            assert!(linked_records.contains(&"GameEvent"));
            assert!(audit_fields.contains(&"context_hash"));
            assert!(audit_fields.contains(&"decision_summary"));
        }
        other => panic!("unexpected payload: {other:?}"),
    }
}

#[test]
fn decision_pipeline_fixture_expected_records_are_asserted() {
    let request = ToolRequest::formal(
        RuntimeAgent::AiKeeperOrchestrator,
        RuntimeTool::RequestSkillCheck,
    );
    let decision =
        RuntimeDecision::new("decision_fixture", "Spot Hidden normal check", request).unwrap();
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
    match &events[1].payload {
        RuntimeEventPayload::DecisionCommitted {
            linked_records,
            audit_fields,
            ..
        } => {
            for record in ["DecisionRecord", "DiceRoll", "GameEvent"] {
                assert!(linked_records.contains(&record));
            }
            for field in [
                "agent_pack_version",
                "prompt_version",
                "model_provider",
                "context_hash",
                "tool_calls",
                "decision_summary",
            ] {
                assert!(audit_fields.contains(&field));
            }
        }
        other => panic!("unexpected payload: {other:?}"),
    }
}

#[test]
fn tool_gate_fixture_error_cases_are_asserted() {
    let atmosphere_request =
        ToolRequest::formal(RuntimeAgent::AtmosphereWriter, RuntimeTool::ChangeScene);
    assert_eq!(
        capability_layer_tool_grant::grant_capability_layer_tool(
            &AuthorityMode::AiKp,
            &atmosphere_request,
        )
        .unwrap_err()
        .code(),
        "AGENT_TOOL_NOT_ALLOWED"
    );

    let copilot_request =
        ToolRequest::formal(RuntimeAgent::KeeperCopilot, RuntimeTool::RequestSkillCheck);
    let grant =
        capability_layer::evaluate_capability_layer(&AuthorityMode::HumanKp, &copilot_request);
    assert!(!grant.allowed);
    assert!(grant.requires_human_confirmation);
    assert!(grant.draft_only);
    assert_eq!(grant.error_code, Some("HUMAN_KP_AI_DRAFT_ONLY"));
}

#[test]
fn human_kp_ai_formal_tool_becomes_draft_only_pending_decision() {
    let request = ToolRequest::formal(RuntimeAgent::KeeperCopilot, RuntimeTool::RequestSkillCheck);
    let decision =
        RuntimeDecision::new("decision_002", "draft only check", request.clone()).unwrap();
    let pending = pending_decision::open_pending_decision(&AuthorityMode::HumanKp, decision);

    assert_eq!(pending.status, PendingDecisionStatus::DraftOnly);
    assert!(pending.grant.requires_human_confirmation);
    assert!(pending.grant.draft_only);
    assert_eq!(
        capability_tool_grant::grant_tool(&AuthorityMode::HumanKp, &request)
            .unwrap_err()
            .code(),
        "HUMAN_KP_AI_DRAFT_ONLY"
    );
}

#[test]
fn runtime_pending_decision_wrapper_opens_and_commits_governed_decisions() {
    let request = ToolRequest::formal(
        RuntimeAgent::AiKeeperOrchestrator,
        RuntimeTool::RequestSkillCheck,
    );
    let decision =
        RuntimeDecision::new("decision_runtime_pending", "commit wrapper", request).unwrap();
    let pending = runtime_pending_decision::open_runtime_pending_decision(
        &AuthorityMode::AiKp,
        decision.clone(),
    );
    assert_eq!(pending.status, PendingDecisionStatus::ReadyToCommit);

    let command = trpg_test_support::governed_command(
        decision.clone(),
        ActorRole::Workflow,
        AuthorityMode::AiKp,
    );
    let contract =
        trpg_test_support::authority_contract("camp_ai_harbor", AuthorityMode::AiKp, 1).unwrap();
    let mut store = EventStore::default();

    let events = runtime_pending_decision::commit_runtime_pending_decision(
        &mut store, &contract, &command, decision,
    )
    .unwrap();

    assert_eq!(events.len(), 2);
    assert_eq!(events[1].event_type, "DecisionCommitted");
}

#[test]
fn non_orchestrator_agent_cannot_request_formal_state_tool() {
    let request = ToolRequest::formal(RuntimeAgent::AtmosphereWriter, RuntimeTool::ChangeScene);

    assert_eq!(
        capability_tool_grant::grant_tool(&AuthorityMode::AiKp, &request)
            .unwrap_err()
            .code(),
        "AGENT_TOOL_NOT_ALLOWED"
    );
}

#[test]
fn direct_agent_state_write_is_rejected_before_event_append() {
    let request = ToolRequest::formal(
        RuntimeAgent::AiKeeperOrchestrator,
        RuntimeTool::RequestSkillCheck,
    );
    let decision = RuntimeDecision::new("decision_003", "bad direct write", request).unwrap();
    let mut command = trpg_test_support::governed_command(
        decision.clone(),
        ActorRole::Workflow,
        AuthorityMode::AiKp,
    );
    command.write_path = FormalWritePath::DirectAgent;
    let contract =
        trpg_test_support::authority_contract("camp_ai_harbor", AuthorityMode::AiKp, 1).unwrap();
    let mut store = EventStore::default();

    let error = runtime_workflow_engine::commit_runtime_workflow_decision(
        &mut store, &contract, &command, decision,
    )
    .unwrap_err();

    assert_eq!(error, RuntimeError::AgentDirectStateWriteForbidden);
    assert_eq!(store.events().len(), 0);
}

#[test]
fn session_workflow_saga_and_scheduler_use_governed_runtime_paths() {
    let contract =
        trpg_test_support::authority_contract("camp_ai_harbor", AuthorityMode::AiKp, 1).unwrap();
    let mut store = EventStore::default();

    let session_event = session_runtime::start_session(
        &mut store,
        &contract,
        &runtime_command("start session", 0, "idem_session"),
        "session_001",
    )
    .unwrap();
    assert_eq!(session_event.event_type, "SessionStarted");

    let workflow_event = workflow_engine::advance_workflow(
        &mut store,
        &contract,
        &runtime_command("advance workflow", 1, "idem_workflow"),
        "workflow_001",
    )
    .unwrap();
    assert_eq!(workflow_event.event_type, "WorkflowAdvanced");

    let saga_event = saga_transaction::compensate_saga(
        &mut store,
        &contract,
        &runtime_command("compensate saga", 2, "idem_saga"),
        SagaCompensation::new("saga_001").unwrap(),
    )
    .unwrap();
    assert_eq!(saga_event.event_type, "SagaCompensated");
    assert_eq!(store.events().len(), 3);

    let due = ScheduledRuntimeTask::new("task_due", 7).unwrap();
    let later = ScheduledRuntimeTask::new("task_later", 9).unwrap();
    assert_eq!(
        scheduler_service::due_tasks(&[due.clone(), later], 7),
        vec![due]
    );
}

#[test]
fn adr_boundary_keeps_external_workflows_out_of_canon() {
    assert!(
        adr_0007_internal_workflow_vs_temporal::INTERNAL_WORKFLOW_BOUNDARY
            .contains("runtime workflow remains internal")
    );
    assert!(
        adr_0007_internal_workflow_vs_temporal::TEMPORAL_ADAPTER_POLICY
            .contains("must not become the event-store canon")
    );
}

#[test]
fn keeper_only_runtime_events_do_not_sync_to_public_room() {
    let request = ToolRequest::formal(
        RuntimeAgent::AiKeeperOrchestrator,
        RuntimeTool::RequestSkillCheck,
    );
    let decision = RuntimeDecision::new("decision_004", "keeper-only check", request).unwrap();
    let mut command = trpg_test_support::governed_command(
        decision.clone(),
        ActorRole::Workflow,
        AuthorityMode::AiKp,
    );
    command.visibility = Visibility::new(VisibilityLabel::KeeperOnly);
    let contract =
        trpg_test_support::authority_contract("camp_ai_harbor", AuthorityMode::AiKp, 1).unwrap();
    let mut store = EventStore::default();

    runtime_workflow_engine::commit_runtime_workflow_decision(
        &mut store, &contract, &command, decision,
    )
    .unwrap();

    assert_eq!(
        realtime_room_sync::sync_visible_room_events(&store, &PrincipalScope::Public).len(),
        0
    );
    assert_eq!(
        realtime_room_sync::sync_visible_room_events(&store, &PrincipalScope::Keeper).len(),
        2
    );
}

#[test]
fn realtime_runtime_binding_respects_private_player_visibility() {
    let request = ToolRequest::formal(
        RuntimeAgent::AiKeeperOrchestrator,
        RuntimeTool::RequestSkillCheck,
    );
    let decision = RuntimeDecision::new("decision_private", "private visibility", request).unwrap();
    let mut command = trpg_test_support::governed_command(
        decision.clone(),
        ActorRole::Workflow,
        AuthorityMode::AiKp,
    );
    let player_a = EntityId::new("user_player_a").unwrap();
    let player_b = EntityId::new("user_player_b").unwrap();
    command.visibility = Visibility::private_to_player(player_a.clone());
    let contract =
        trpg_test_support::authority_contract("camp_ai_harbor", AuthorityMode::AiKp, 1).unwrap();
    let mut store = EventStore::default();

    runtime_workflow_engine::commit_runtime_workflow_decision(
        &mut store, &contract, &command, decision,
    )
    .unwrap();

    assert_eq!(
        realtime_runtime_binding::visible_runtime_deltas(
            &store,
            &PrincipalScope::Player(player_a),
        )
        .len(),
        2
    );
    assert_eq!(
        realtime_runtime_binding::visible_runtime_deltas(
            &store,
            &PrincipalScope::Player(player_b),
        )
        .len(),
        0
    );
    assert_eq!(
        realtime_runtime_binding::visible_runtime_deltas(&store, &PrincipalScope::Public).len(),
        0
    );
}

#[test]
fn expected_version_and_idempotency_are_enforced() {
    let request = ToolRequest::formal(
        RuntimeAgent::AiKeeperOrchestrator,
        RuntimeTool::RequestSkillCheck,
    );
    let decision = RuntimeDecision::new("decision_005", "version guard", request).unwrap();
    let mut command = trpg_test_support::governed_command(
        decision.clone(),
        ActorRole::Workflow,
        AuthorityMode::AiKp,
    );
    command.expected_version = 1;
    let contract =
        trpg_test_support::authority_contract("camp_ai_harbor", AuthorityMode::AiKp, 1).unwrap();
    let mut store = EventStore::default();

    assert_eq!(
        runtime_workflow_engine::commit_runtime_workflow_decision(
            &mut store,
            &contract,
            &command,
            decision.clone(),
        )
        .unwrap_err()
        .code(),
        "EXPECTED_VERSION_CONFLICT"
    );

    command.expected_version = 0;
    runtime_workflow_engine::commit_runtime_workflow_decision(
        &mut store,
        &contract,
        &command,
        decision.clone(),
    )
    .unwrap();
    command.expected_version = 2;
    assert_eq!(
        runtime_workflow_engine::commit_runtime_workflow_decision(
            &mut store, &contract, &command, decision,
        )
        .unwrap_err()
        .code(),
        "DUPLICATE_COMMAND"
    );
}
