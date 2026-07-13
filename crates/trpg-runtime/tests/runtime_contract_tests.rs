use trpg_runtime::runtime;
use trpg_runtime::runtime_state_machines::{
    RuntimeAgent, RuntimeDecision, RuntimeError, RuntimeModule, RuntimeTool, ToolRequest,
    RUNTIME_MODULES,
};
use trpg_runtime::{
    ActorRole, AuthorityContract, AuthorityMode, EventStore, FormalWritePath, PrincipalScope,
    TrpgError, Visibility, VisibilityLabel,
};
use trpg_shared_kernel::WireErrorCode;

#[test]
fn runtime_registry_includes_public_runtime_boundaries() {
    for module in [
        RuntimeModule::Saga,
        RuntimeModule::CampaignSessionRuntimeService,
        RuntimeModule::Runtime,
        RuntimeModule::Readme,
    ] {
        assert!(RUNTIME_MODULES.contains(&module));
    }
}

#[test]
fn every_runtime_error_uses_the_canonical_wire_registry() {
    let cases = [
        (
            RuntimeError::Core(TrpgError::ExpectedVersionConflict {
                expected: 1,
                actual: 2,
            }),
            WireErrorCode::ExpectedVersionConflict,
        ),
        (
            RuntimeError::AgentToolNotAllowed,
            WireErrorCode::AgentToolNotAllowed,
        ),
        (
            RuntimeError::HumanKpAiDraftOnly,
            WireErrorCode::HumanKpAiDraftOnly,
        ),
        (
            RuntimeError::AgentDirectStateWriteForbidden,
            WireErrorCode::AgentDirectStateWriteForbidden,
        ),
    ];

    assert_eq!(cases.len(), 4);
    for (error, expected) in cases {
        assert_eq!(error.wire_code(), expected);
        assert_eq!(error.code(), expected.as_str());
        assert_eq!(WireErrorCode::lookup(error.code()).unwrap(), expected);
    }
}

#[test]
fn runtime_boundary_snapshot_preserves_s06_governance_fields() {
    let snapshot = runtime::runtime_boundary_snapshot();

    assert_eq!(snapshot.canon_store, "Event Store");
    assert!(snapshot
        .formal_write_path
        .contains("Command -> Workflow -> Decision"));
    for field in [
        "idempotency_key",
        "expected_version",
        "actor",
        "authority_mode",
        "visibility",
        "fact_provenance",
        "correlation_id",
        "causation_id",
    ] {
        assert!(snapshot.required_command_fields.contains(&field));
    }
}

#[test]
fn runtime_commits_ai_kp_decision_through_evented_pipeline() {
    let request = ToolRequest::formal(
        RuntimeAgent::AiKeeperOrchestrator,
        RuntimeTool::RequestSkillCheck,
    );
    let decision = RuntimeDecision::new("decision_b013", "Spot Hidden check", request).unwrap();
    let command = trpg_test_support::governed_command!(
        decision.clone(),
        ActorRole::Workflow,
        AuthorityMode::AiKp
    );
    let contract = AuthorityContract::new("camp_ai_harbor", AuthorityMode::AiKp, 1).unwrap();
    let mut store = EventStore::default();

    let events =
        runtime::commit_runtime_decision(&mut store, &contract, &command, decision).unwrap();

    assert_eq!(events.len(), 2);
    assert_eq!(events[0].event_type, "ToolRequestApproved");
    assert_eq!(events[1].event_type, "DecisionCommitted");
}

#[test]
fn runtime_replay_does_not_expose_keeper_only_events_to_public() {
    let request = ToolRequest::formal(
        RuntimeAgent::AiKeeperOrchestrator,
        RuntimeTool::RequestSkillCheck,
    );
    let decision = RuntimeDecision::new("decision_keeper_only", "hidden check", request).unwrap();
    let mut command = trpg_test_support::governed_command!(
        decision.clone(),
        ActorRole::Workflow,
        AuthorityMode::AiKp
    );
    command.visibility = Visibility::new(VisibilityLabel::KeeperOnly);
    let contract = AuthorityContract::new("camp_ai_harbor", AuthorityMode::AiKp, 1).unwrap();
    let mut store = EventStore::default();

    runtime::commit_runtime_decision(&mut store, &contract, &command, decision).unwrap();

    assert!(runtime::replay_runtime_for_principal(&store, &PrincipalScope::Public).is_empty());
    assert_eq!(
        runtime::replay_runtime_for_principal(&store, &PrincipalScope::Keeper).len(),
        2
    );
}

#[test]
fn runtime_rejects_agent_direct_write_before_append() {
    let request = ToolRequest::formal(
        RuntimeAgent::AiKeeperOrchestrator,
        RuntimeTool::RequestSkillCheck,
    );
    let decision = RuntimeDecision::new("decision_direct", "bad write", request).unwrap();
    let mut command = trpg_test_support::governed_command!(
        decision.clone(),
        ActorRole::Workflow,
        AuthorityMode::AiKp
    );
    command.write_path = FormalWritePath::DirectAgent;
    let contract = AuthorityContract::new("camp_ai_harbor", AuthorityMode::AiKp, 1).unwrap();
    let mut store = EventStore::default();

    let error =
        runtime::commit_runtime_decision(&mut store, &contract, &command, decision).unwrap_err();

    assert_eq!(error.code(), "AGENT_DIRECT_STATE_WRITE_FORBIDDEN");
    assert!(store.events().is_empty());
}
