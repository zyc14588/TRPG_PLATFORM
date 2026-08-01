#[tokio::test]
async fn real_agent_job_worker_completes_an_npc_turn_through_all_three_providers() {
    for provider_type in [
        ProviderType::Cloud,
        ProviderType::Ollama,
        ProviderType::LlamaCpp,
    ] {
        let repository = Arc::new(MemoryRepository::new(job(provider_type, "AI_KP")));
        let provider = Arc::new(MockProvider::new(provider_type, valid_decision()));
        let decisions = Arc::new(CountingDecisionPort::default());
        let certification_fixture =
            (provider_type != ProviderType::Cloud).then(|| level4_certification("model_ar09"));
        let certification = certification_fixture
            .as_ref()
            .map(|fixture| fixture.certification.clone());
        let outcome = worker(
            Arc::clone(&repository),
            Arc::clone(&provider),
            Arc::new(CountingToolPort::default()),
            Arc::clone(&decisions),
            certification,
            configuration(),
        )
        .run_once(NOW)
        .await
        .unwrap();
        assert!(matches!(outcome, AgentJobOutcome::Completed { .. }));
        assert_eq!(repository.job().state, WorkflowState::Completed);
        assert_eq!(repository.job().decision_json, None);
        assert_eq!(repository.job().tool_result_json, None);
        assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
        assert_eq!(decisions.authorization_checks.load(Ordering::SeqCst), 1);
        assert_eq!(decisions.canonical_events.load(Ordering::SeqCst), 1);
    }
}

#[tokio::test]
async fn crash_after_tool_and_canonical_commit_recovers_without_duplicates() {
    let mut output = valid_decision();
    output["tool"] = json!({"name":"change_scene","arguments":{}});
    let repository = Arc::new(MemoryRepository::new(job(ProviderType::Cloud, "AI_KP")));
    let provider = Arc::new(MockProvider::new(ProviderType::Cloud, output));
    let tools = Arc::new(CountingToolPort::default());
    let decisions = Arc::new(CountingDecisionPort::crash_after_first_commit());
    let worker = worker(
        Arc::clone(&repository),
        Arc::clone(&provider),
        Arc::clone(&tools),
        Arc::clone(&decisions),
        None,
        configuration(),
    );

    let first = worker.run_once(NOW).await.unwrap();
    assert!(matches!(first, AgentJobOutcome::RetryScheduled { .. }));
    let second = worker.run_once(NOW + 100_000).await.unwrap();
    assert!(matches!(second, AgentJobOutcome::Completed { .. }));
    assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
    assert_eq!(decisions.authorization_checks.load(Ordering::SeqCst), 2);
    assert_eq!(tools.executions.load(Ordering::SeqCst), 1);
    assert_eq!(decisions.canonical_events.load(Ordering::SeqCst), 1);
    assert_eq!(repository.job().linked_event_sequences.len(), 1);
}

#[tokio::test]
async fn human_kp_job_is_draft_only_until_a_separate_approval_event_exists() {
    let repository = Arc::new(MemoryRepository::new(job(ProviderType::Cloud, "HUMAN_KP")));
    let provider = Arc::new(MockProvider::new(ProviderType::Cloud, valid_decision()));
    let decisions = Arc::new(CountingDecisionPort::default());
    let worker = worker(
        Arc::clone(&repository),
        Arc::clone(&provider),
        Arc::new(CountingToolPort::default()),
        Arc::clone(&decisions),
        None,
        configuration(),
    );

    let draft = worker.run_once(NOW).await.unwrap();
    assert!(matches!(
        draft,
        AgentJobOutcome::AwaitingHumanApproval { .. }
    ));
    assert_eq!(decisions.canonical_events.load(Ordering::SeqCst), 0);
    assert_eq!(provider.calls.load(Ordering::SeqCst), 1);

    repository.approve(77);
    let approved = worker.run_once(NOW + 1).await.unwrap();
    assert_eq!(
        approved,
        AgentJobOutcome::Completed {
            job_id: "job_cloud".to_owned(),
            event_sequences: vec![77],
        }
    );
    assert_eq!(decisions.canonical_events.load(Ordering::SeqCst), 0);
    assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
    assert_eq!(repository.job().decision_json, None);
}

#[tokio::test]
async fn local_ai_keeper_without_level4_certification_fails_closed() {
    let repository = Arc::new(MemoryRepository::new(job(ProviderType::Ollama, "AI_KP")));
    let provider = Arc::new(MockProvider::new(ProviderType::Ollama, valid_decision()));
    let outcome = worker(
        Arc::clone(&repository),
        provider,
        Arc::new(CountingToolPort::default()),
        Arc::new(CountingDecisionPort::default()),
        None,
        configuration(),
    )
    .run_once(NOW)
    .await
    .unwrap();
    assert!(matches!(
        outcome,
        AgentJobOutcome::TerminalFailure {
            error_code: "LOCAL_MODEL_LEVEL_4_REQUIRED",
            ..
        }
    ));
    assert_eq!(repository.job().state, WorkflowState::TerminalFailed);
}

#[tokio::test]
async fn invisible_rag_and_unauthorized_tools_and_invalid_output_fail_closed() {
    let unauthorized_repository =
        Arc::new(MemoryRepository::new(job(ProviderType::Cloud, "AI_KP")));
    let unauthorized_provider = Arc::new(MockProvider::new(ProviderType::Cloud, valid_decision()));
    let unauthorized_outcome = worker(
        Arc::clone(&unauthorized_repository),
        Arc::clone(&unauthorized_provider),
        Arc::new(CountingToolPort::default()),
        Arc::new(CountingDecisionPort::deny_authorization()),
        None,
        configuration(),
    )
    .run_once(NOW)
    .await
    .unwrap();
    assert_eq!(
        unauthorized_outcome,
        AgentJobOutcome::TerminalFailure {
            job_id: "job_cloud".to_owned(),
            error_code: "AGENT_EXECUTION_AUTHORIZATION_DENIED",
        }
    );
    assert_eq!(unauthorized_provider.calls.load(Ordering::SeqCst), 0);

    let invisible_repository = Arc::new(MemoryRepository::new(job(ProviderType::Cloud, "AI_KP")));
    let mut invisible = visible_context();
    invisible.chunks[0].visibility_label = "keeper_only".to_owned();
    invisible_repository.set_context(invisible);
    let invisible_outcome = worker(
        Arc::clone(&invisible_repository),
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
        invisible_outcome,
        AgentJobOutcome::TerminalFailure {
            error_code: "RAG_VISIBILITY_SCOPE_VIOLATION",
            ..
        }
    ));

    let unauthorized_repository =
        Arc::new(MemoryRepository::new(job(ProviderType::Cloud, "AI_KP")));
    let unauthorized = json!({
        "kind":"npc_turn",
        "player_visible_text":"No.",
        "tool":{"name":"delete_database","arguments":{}}
    });
    let unauthorized_outcome = worker(
        unauthorized_repository,
        Arc::new(MockProvider::new(ProviderType::Cloud, unauthorized)),
        Arc::new(CountingToolPort::default()),
        Arc::new(CountingDecisionPort::default()),
        None,
        configuration(),
    )
    .run_once(NOW)
    .await
    .unwrap();
    assert!(matches!(
        unauthorized_outcome,
        AgentJobOutcome::TerminalFailure {
            error_code: "AGENT_TOOL_PERMISSION_DENIED",
            ..
        }
    ));

    let invalid_repository = Arc::new(MemoryRepository::new(job(ProviderType::Cloud, "AI_KP")));
    let invalid_outcome = worker(
        invalid_repository,
        Arc::new(MockProvider::new(
            ProviderType::Cloud,
            json!({"kind":"npc_turn"}),
        )),
        Arc::new(CountingToolPort::default()),
        Arc::new(CountingDecisionPort::default()),
        None,
        configuration(),
    )
    .run_once(NOW)
    .await
    .unwrap();
    assert!(matches!(
        invalid_outcome,
        AgentJobOutcome::TerminalFailure {
            error_code: "AGENT_OUTPUT_SCHEMA_INVALID",
            ..
        }
    ));
}
