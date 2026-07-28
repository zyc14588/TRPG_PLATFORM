
#[test]
fn p04_runtime_rejects_corrupt_batch_receipt_without_partial_local_events() {
    let request = ToolRequest::formal(
        RuntimeAgent::AiKeeperOrchestrator,
        RuntimeTool::RequestSkillCheck,
    );
    let decision = RuntimeDecision::new(
        "decision_p04_corrupt_receipt",
        "corrupt receipt probe",
        request,
    )
    .unwrap();
    let contract = trpg_test_support::authority_contract(
        "campaign_p04_corrupt_receipt",
        AuthorityMode::AiKp,
        1,
    )
    .unwrap();
    let command = trpg_test_support::governed_command_for_contract(
        &contract,
        decision.clone(),
        ActorRole::Workflow,
    );
    let mut store = audited_store_with_canonical(
        &contract,
        trpg_test_support::corrupt_second_event_receipt_port(),
    );

    let error = runtime_workflow_engine::commit_runtime_workflow_decision(
        &mut store,
        &contract,
        &command,
        &trpg_test_support::workflow_authentication(),
        decision,
        2,
    )
    .unwrap_err();

    assert_eq!(error.code(), "AUDIT_INTEGRITY_VIOLATION");
    assert!(store.events().is_empty());
}
