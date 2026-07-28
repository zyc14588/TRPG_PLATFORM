
#[test]
fn production_runtime_without_a_bound_tool_executor_fails_closed() {
    let environment = RealEnvironment::load();
    let contract =
        trpg_test_support::authority_contract("camp_ai_harbor", AuthorityMode::AiKp, 1).unwrap();
    let (mut application, authentication) =
        production_application(&environment, &contract, "missing-tool-executor");

    let version_before = campaign_version(&application, contract.campaign_id().as_str());
    let suffix = format!("{}_{version_before}", std::process::id());
    let decision = RuntimeDecision::new(
        format!("decision_production_{suffix}"),
        "production composition root canonical commit",
        ToolRequest::formal(
            RuntimeAgent::AiKeeperOrchestrator,
            RuntimeTool::RequestSkillCheck,
        ),
    )
    .unwrap();
    let mut command = trpg_test_support::governed_command_for_contract(
        &contract,
        decision.clone(),
        ActorRole::Workflow,
    );
    command.command_id = EntityId::new(format!("command_production_{suffix}")).unwrap();
    command.idempotency_key = format!("idempotency_production_{suffix}");
    command.expected_version = version_before;

    let custody = Arc::get_mut(application.canonical_custody.as_mut().unwrap()).unwrap();
    let error = runtime::commit_runtime_decision(
        &mut custody.runtime_events,
        &contract,
        &command,
        &authentication,
        decision.clone(),
        2,
    )
    .unwrap_err();
    assert_eq!(error, RuntimeError::AgentToolNotAllowed);
    assert!(custody.runtime_events.events().is_empty());
    assert_eq!(
        replay(&application, contract.campaign_id().as_str()).len() as u64,
        version_before
    );
    verify_integrity(&application);
}
