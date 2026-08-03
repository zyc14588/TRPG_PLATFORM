#[tokio::test]
async fn token_budget_deadline_and_cancellation_are_enforced() {
    let mut loop_configuration = configuration();
    loop_configuration.max_tool_loops = 2;
    assert_eq!(
        loop_configuration.validate().unwrap_err().code(),
        "AGENT_JOB_CONFIGURATION_INVALID"
    );

    let mut exhausted_job = job(ProviderType::Cloud, "AI_KP");
    exhausted_job.state = WorkflowState::AgentRunning;
    exhausted_job.attempt = configuration().max_attempts;
    let exhausted_repository = Arc::new(MemoryRepository::new(exhausted_job));
    let exhausted_provider = Arc::new(MockProvider::new(ProviderType::Cloud, valid_decision()));
    let exhausted_outcome = worker(
        exhausted_repository,
        Arc::clone(&exhausted_provider),
        Arc::new(CountingToolPort::default()),
        Arc::new(CountingDecisionPort::default()),
        None,
        configuration(),
    )
    .run_once(NOW)
    .await
    .unwrap();
    assert_eq!(
        exhausted_outcome,
        AgentJobOutcome::TerminalFailure {
            job_id: "job_cloud".to_owned(),
            error_code: "AGENT_JOB_ATTEMPT_LIMIT_EXCEEDED",
        }
    );
    assert_eq!(exhausted_provider.calls.load(Ordering::SeqCst), 0);

    let budget_repository = Arc::new(MemoryRepository::new(job(ProviderType::Cloud, "AI_KP")));
    let mut budget_provider = MockProvider::new(ProviderType::Cloud, valid_decision());
    budget_provider.usage.output_tokens = 2_000;
    let budget_outcome = worker(
        budget_repository,
        Arc::new(budget_provider),
        Arc::new(CountingToolPort::default()),
        Arc::new(CountingDecisionPort::default()),
        None,
        configuration(),
    )
    .run_once(NOW)
    .await
    .unwrap();
    assert!(matches!(
        budget_outcome,
        AgentJobOutcome::TerminalFailure {
            error_code: "AGENT_JOB_BUDGET_EXCEEDED",
            ..
        }
    ));

    let mut expired_job = job(ProviderType::Cloud, "AI_KP");
    expired_job.deadline_unix_ms = NOW;
    let deadline_outcome = worker(
        Arc::new(MemoryRepository::new(expired_job)),
        Arc::new(MockProvider::new(ProviderType::Cloud, valid_decision())),
        Arc::new(CountingToolPort::default()),
        Arc::new(CountingDecisionPort::default()),
        None,
        configuration(),
    )
    .run_once(NOW)
    .await
    .unwrap();
    assert!(matches!(
        deadline_outcome,
        AgentJobOutcome::TerminalFailure {
            error_code: "AGENT_JOB_DEADLINE_EXCEEDED",
            ..
        }
    ));

    let cancellation_repository =
        Arc::new(MemoryRepository::new(job(ProviderType::Cloud, "AI_KP")));
    cancellation_repository.cancel();
    let cancellation_outcome = worker(
        cancellation_repository,
        Arc::new(MockProvider::new(ProviderType::Cloud, valid_decision())),
        Arc::new(CountingToolPort::default()),
        Arc::new(CountingDecisionPort::default()),
        None,
        configuration(),
    )
    .run_once(NOW)
    .await
    .unwrap();
    assert!(matches!(
        cancellation_outcome,
        AgentJobOutcome::TerminalFailure {
            error_code: "AGENT_JOB_CANCELLED",
            ..
        }
    ));
}
