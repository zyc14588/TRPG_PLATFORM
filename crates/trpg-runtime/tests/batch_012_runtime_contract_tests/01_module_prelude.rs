use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

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
    ActorRole, AuthorityContract, AuthorityMode, CommandEnvelope, EntityId, EventStore,
    FormalCommitAudit, FormalCommitAuthorizer, FormalWritePath, RuntimeToolExecutionOutput,
    RuntimeToolExecutor, Visibility, VisibilityLabel,
};
use trpg_security_governance::policy_adapter::{
    HttpPolicyEndpoint, OpenFgaOpaPolicyAdapter, PolicyBackend,
};
use trpg_shared_kernel::CanonicalCommitPort;

static NEXT_AUDIT_ID: AtomicU64 = AtomicU64::new(1);

struct SuccessfulRuntimeToolExecutor;

impl RuntimeToolExecutor for SuccessfulRuntimeToolExecutor {
    fn execute(
        &self,
        decision: &RuntimeDecision,
    ) -> trpg_runtime::runtime_state_machines::RuntimeResult<RuntimeToolExecutionOutput> {
        Ok(RuntimeToolExecutionOutput {
            execution_id: format!("execution_{}", decision.decision_id.as_str()),
            result_hash: "sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"
                .to_owned(),
        })
    }
}

struct CountingRuntimeToolExecutor {
    calls: Arc<AtomicU64>,
}

impl RuntimeToolExecutor for CountingRuntimeToolExecutor {
    fn execute(
        &self,
        decision: &RuntimeDecision,
    ) -> trpg_runtime::runtime_state_machines::RuntimeResult<RuntimeToolExecutionOutput> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        SuccessfulRuntimeToolExecutor.execute(decision)
    }
}

fn audited_store(contract: &AuthorityContract) -> EventStore<RuntimeEventPayload> {
    audited_store_with_canonical(contract, trpg_test_support::test_canonical_commit_port())
}

fn audited_store_with_canonical(
    contract: &AuthorityContract,
    canonical: Arc<dyn CanonicalCommitPort>,
) -> EventStore<RuntimeEventPayload> {
    audited_store_with_canonical_and_executor(
        contract,
        canonical,
        Arc::new(SuccessfulRuntimeToolExecutor),
    )
}

fn audited_store_with_canonical_and_executor(
    contract: &AuthorityContract,
    canonical: Arc<dyn CanonicalCommitPort>,
    executor: Arc<dyn RuntimeToolExecutor>,
) -> EventStore<RuntimeEventPayload> {
    let audit_id = NEXT_AUDIT_ID.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "p02-runtime-batch-audit-{}-{audit_id}.jsonl",
        std::process::id()
    ));
    let audit = FormalCommitAudit::open(path, "runtime-batch-test-v1", &[0x83; 32]).unwrap();
    let endpoints = trpg_test_support::formal_commit_policy_endpoints();
    let policy = OpenFgaOpaPolicyAdapter::new(
        HttpPolicyEndpoint::new(
            endpoints.openfga,
            "/stores/test/check",
            PolicyBackend::OpenFga,
            endpoints.openfga_model,
        )
        .unwrap(),
        HttpPolicyEndpoint::new(
            endpoints.opa,
            "/v1/data/security_governance/decision",
            PolicyBackend::Opa,
            endpoints.opa_revision,
        )
        .unwrap(),
    )
    .unwrap();
    let (identity_verifier, _) = trpg_test_support::formal_commit_identity_for_contract(contract);
    EventStore::with_formal_custody_and_executor(
        FormalCommitAuthorizer::new(identity_verifier, policy, audit),
        canonical,
        executor,
    )
}

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
    let mut store = audited_store(&contract);

    let events = runtime_workflow_engine::commit_runtime_workflow_decision(
        &mut store,
        &contract,
        &command,
        &trpg_test_support::workflow_authentication(),
        decision,
        2,
    )
    .unwrap();

    assert_eq!(events.len(), 3);
    assert_eq!(events[0].event_type, "ToolRequestApproved");
    assert_eq!(events[1].event_type, "ToolExecutionSucceeded");
    assert_eq!(events[2].event_type, "DecisionCommitted");
    match &events[2].payload {
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
    let mut store = audited_store(&contract);

    let events = workflow_engine::commit_workflow_decision(
        &mut store,
        &contract,
        &command,
        &trpg_test_support::workflow_authentication(),
        decision,
        2,
    )
    .unwrap();

    assert_eq!(events[0].event_type, "ToolRequestApproved");
    assert_eq!(events[1].event_type, "ToolExecutionSucceeded");
    assert_eq!(events[2].event_type, "DecisionCommitted");
    match &events[2].payload {
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
