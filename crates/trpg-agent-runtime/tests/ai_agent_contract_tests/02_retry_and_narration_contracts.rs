#[test]
fn ai_keeper_narration_commits_one_canonical_event_without_tool_side_effect() {
    let contract =
        trpg_test_support::authority_contract("campaign_ar09_ai_narration", AuthorityMode::AiKp, 1)
            .unwrap();
    let authentication =
        trpg_test_support::ai_keeper_authentication(contract.campaign_id().as_str());
    let decision = AgentDecision::new(
        "decision_ar09_ai_narration",
        ToolRequest::formal(AgentKind::AiKeeperOrchestrator, AgentTool::NarrationOnly),
        "The lantern throws a long shadow across the archive door.",
        &authentication,
    )
    .unwrap();
    let command = trpg_test_support::governed_command_for_contract(
        &contract,
        decision.clone(),
        ActorRole::Workflow,
    );
    let (mut store, audit) = common::audited_store_with_handle(&contract);
    let calls = Arc::new(AtomicU64::new(0));
    let committer = counting_committer(&contract, calls.clone());

    let events = ai_agent::submit_ai_agent_decision(
        &committer,
        &mut store,
        &command,
        &trpg_test_support::workflow_authentication(),
        decision,
        2,
    )
    .unwrap();

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event_type, "DecisionCommitted");
    assert_eq!(store.events(), events.as_slice());
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let audit_records = audit.verify().unwrap();
    assert_eq!(audit_records.len(), 1);
    assert_eq!(audit_records[0].action, "write_official_state");
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
    let calls = Arc::new(AtomicU64::new(0));
    let committer = counting_committer(&contract, calls.clone());

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
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "an exact retry must not repeat the tool side effect"
    );
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
    let calls = Arc::new(AtomicU64::new(0));
    let first_committer = counting_committer(&contract, calls.clone());

    let first = ai_agent::submit_ai_agent_decision(
        &first_committer,
        &mut first_store,
        &command,
        &trpg_test_support::workflow_authentication(),
        decision.clone(),
        2,
    )
    .unwrap();

    std::thread::sleep(std::time::Duration::from_millis(2));
    let (mut restarted_store, _) = common::audited_store_with_canonical(&contract, canonical);
    let restarted_committer = counting_committer(&contract, calls.clone());
    let replayed = ai_agent::submit_ai_agent_decision(
        &restarted_committer,
        &mut restarted_store,
        &command,
        &trpg_test_support::workflow_authentication(),
        decision,
        2,
    )
    .expect("a cold exact retry must materialize the durable event identities");

    assert_eq!(replayed, first);
    assert_eq!(restarted_store.events(), first.as_slice());
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "a cold retry must resolve the canonical execution result"
    );
}
