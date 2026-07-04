use trpg_runtime::runtime;
use trpg_runtime::runtime_state_machines::{
    RuntimeAgent, RuntimeDecision, RuntimeModule, RuntimeTool, ToolRequest,
    BATCH_013_PRIMARY_MODULES,
};
use trpg_runtime::{
    ActorRole, AuthorityContract, AuthorityMode, CommandEnvelope, EventStore, FormalWritePath,
    PrincipalScope, Visibility, VisibilityLabel,
};

#[test]
fn runtime_indexes_current_batch_primary_modules() {
    assert_eq!(BATCH_013_PRIMARY_MODULES.len(), 4);
    assert!(BATCH_013_PRIMARY_MODULES.contains(&RuntimeModule::Saga));
    assert!(BATCH_013_PRIMARY_MODULES.contains(&RuntimeModule::CampaignSessionRuntimeService));
    assert!(BATCH_013_PRIMARY_MODULES.contains(&RuntimeModule::Runtime));
    assert!(BATCH_013_PRIMARY_MODULES.contains(&RuntimeModule::Readme));
    assert_eq!(
        runtime::PROMPT_ID,
        "CODEX-0363-03-RUNTIME-ORCHESTRATION-2b19458f57"
    );
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
    let command =
        CommandEnvelope::governed(decision.clone(), ActorRole::Workflow, AuthorityMode::AiKp);
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
    let mut command =
        CommandEnvelope::governed(decision.clone(), ActorRole::Workflow, AuthorityMode::AiKp);
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
    let mut command =
        CommandEnvelope::governed(decision.clone(), ActorRole::Workflow, AuthorityMode::AiKp);
    command.write_path = FormalWritePath::DirectAgent;
    let contract = AuthorityContract::new("camp_ai_harbor", AuthorityMode::AiKp, 1).unwrap();
    let mut store = EventStore::default();

    let error =
        runtime::commit_runtime_decision(&mut store, &contract, &command, decision).unwrap_err();

    assert_eq!(error.code(), "AGENT_DIRECT_STATE_WRITE_FORBIDDEN");
    assert!(store.events().is_empty());
}
