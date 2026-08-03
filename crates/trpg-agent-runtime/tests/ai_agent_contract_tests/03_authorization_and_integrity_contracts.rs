#[test]
fn denied_formal_authorization_never_invokes_the_tool_executor() {
    let request = ToolRequest::formal(
        AgentKind::AiKeeperOrchestrator,
        AgentTool::RequestSkillCheck,
    );
    let contract = trpg_test_support::authority_contract(
        "campaign_b018_agent_policy_deny",
        AuthorityMode::AiKp,
        1,
    )
    .unwrap();
    let authentication =
        trpg_test_support::ai_keeper_authentication(contract.campaign_id().as_str());
    let decision = AgentDecision::new(
        "decision_b018_agent_policy_deny",
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
    let (mut store, _) = common::audited_store_with_policy_endpoints(
        &contract,
        trpg_test_support::test_canonical_commit_port(),
        trpg_test_support::denied_formal_commit_policy_endpoints(),
    );
    let calls = Arc::new(AtomicU64::new(0));
    let committer = counting_committer(&contract, calls.clone());

    ai_agent::submit_ai_agent_decision(
        &committer,
        &mut store,
        &command,
        &trpg_test_support::workflow_authentication(),
        decision,
        2,
    )
    .expect_err("formal policy denial must fail closed");

    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert!(store.events().is_empty());
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
