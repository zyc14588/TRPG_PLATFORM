
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
    let mut store = audited_store(&contract);

    let events = runtime_pending_decision::commit_runtime_pending_decision(
        &mut store,
        &contract,
        &command,
        &trpg_test_support::workflow_authentication(),
        decision,
        2,
    )
    .unwrap();

    assert_eq!(events.len(), 3);
    assert_eq!(events[2].event_type, "DecisionCommitted");
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
        &mut store,
        &contract,
        &command,
        &trpg_test_support::workflow_authentication(),
        decision,
        2,
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
    let mut store = audited_store(&contract);

    runtime_workflow_engine::commit_runtime_workflow_decision(
        &mut store,
        &contract,
        &command,
        &trpg_test_support::workflow_authentication(),
        decision,
        2,
    )
    .unwrap();

    let player = trpg_test_support::player_replay_authorization(&contract);
    let system = trpg_test_support::system_replay_authorization(&contract);
    assert!(
        realtime_room_sync::sync_visible_room_events(&store, &player, 206)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        realtime_room_sync::sync_visible_room_events(&store, &system, 206)
            .unwrap()
            .len(),
        3
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
    command.visibility = Visibility::private_to_player(player_a.clone());
    let contract =
        trpg_test_support::authority_contract("camp_ai_harbor", AuthorityMode::AiKp, 1).unwrap();
    let mut store = audited_store(&contract);

    runtime_workflow_engine::commit_runtime_workflow_decision(
        &mut store,
        &contract,
        &command,
        &trpg_test_support::workflow_authentication(),
        decision,
        2,
    )
    .unwrap();

    let player_a_authorization =
        trpg_test_support::player_replay_authorization_for(&contract, "user_player_a");
    let player_b_authorization =
        trpg_test_support::player_replay_authorization_for(&contract, "user_player_b");
    assert_eq!(
        realtime_runtime_binding::visible_runtime_deltas(&store, &player_a_authorization, 206)
            .unwrap()
            .len(),
        3
    );
    assert!(
        realtime_runtime_binding::visible_runtime_deltas(&store, &player_b_authorization, 206,)
            .unwrap()
            .is_empty()
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
    let canonical = trpg_test_support::test_canonical_commit_port();
    let calls = Arc::new(AtomicU64::new(0));
    let mut store = audited_store_with_canonical_and_executor(
        &contract,
        canonical.clone(),
        Arc::new(CountingRuntimeToolExecutor {
            calls: calls.clone(),
        }),
    );

    assert_eq!(
        runtime_workflow_engine::commit_runtime_workflow_decision(
            &mut store,
            &contract,
            &command,
            &trpg_test_support::workflow_authentication(),
            decision.clone(),
            2,
        )
        .unwrap_err()
        .code(),
        "EXPECTED_VERSION_CONFLICT"
    );

    command.expected_version = 0;
    let first_result = runtime_workflow_engine::commit_runtime_workflow_decision(
        &mut store,
        &contract,
        &command,
        &trpg_test_support::workflow_authentication(),
        decision.clone(),
        2,
    )
    .unwrap();

    let replayed_result = runtime_workflow_engine::commit_runtime_workflow_decision(
        &mut store,
        &contract,
        &command,
        &trpg_test_support::workflow_authentication(),
        decision.clone(),
        2,
    )
    .expect("an exact network retry must return the original formal result");
    assert_eq!(replayed_result, first_result);
    assert_eq!(store.events().len(), 3);
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "an exact retry must not repeat the runtime tool side effect"
    );

    let mut restarted_store = audited_store_with_canonical_and_executor(
        &contract,
        canonical,
        Arc::new(CountingRuntimeToolExecutor {
            calls: calls.clone(),
        }),
    );
    let cold_replayed_result = runtime_workflow_engine::commit_runtime_workflow_decision(
        &mut restarted_store,
        &contract,
        &command,
        &trpg_test_support::workflow_authentication(),
        decision.clone(),
        2,
    )
    .expect("a cold exact retry must resolve the durable runtime result");
    assert_eq!(cold_replayed_result, first_result);
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    command.expected_version = 2;
    assert_eq!(
        runtime_workflow_engine::commit_runtime_workflow_decision(
            &mut store,
            &contract,
            &command,
            &trpg_test_support::workflow_authentication(),
            decision,
            2,
        )
        .unwrap_err()
        .code(),
        "DUPLICATE_COMMAND"
    );
}
