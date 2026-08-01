{

    let claim_time = now + 1_000;
    let first_claim = store
        .claim_due_agent_job("worker-a", claim_time, 100)
        .await
        .unwrap()
        .unwrap();
    let running = store
        .transition_agent_job(&transition(
            &first_claim,
            WorkflowState::Claimed,
            WorkflowState::AgentRunning,
            "running",
            claim_time + 1,
        ))
        .await
        .unwrap();
    assert_eq!(running.state, WorkflowState::AgentRunning);

    let recovered = store
        .claim_due_agent_job("worker-b", claim_time + 101, 1_000)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(recovered.job_id, job_id);
    assert_eq!(recovered.state, WorkflowState::Claimed);
    assert_eq!(recovered.resume_state, Some(WorkflowState::AgentRunning));
    assert_eq!(recovered.attempt, 2);
    let recovered_token = recovered.claim_token.clone().unwrap();
    assert!(store
        .heartbeat_agent_job(
            &job_id,
            "worker-b",
            &recovered_token,
            claim_time + 102,
            1_000,
        )
        .await
        .unwrap());

    let mut awaiting = transition(
        &recovered,
        WorkflowState::Claimed,
        WorkflowState::AwaitingTool,
        "awaiting-tool",
        claim_time + 103,
    );
    awaiting.decision_json = Some(r#"{"kind":"narration_only","text":"test"}"#.to_owned());
    let awaiting = store.transition_agent_job(&awaiting).await.unwrap();
    let skill_check = AgentJobSkillCheckDraft {
        job_id: job_id.clone(),
        claim_owner: awaiting.claim_owner.clone().unwrap(),
        claim_token: awaiting.claim_token.clone().unwrap(),
        expected_attempt: awaiting.attempt,
        idempotency_key: format!("{}:tool", draft.idempotency_key),
        character_id: character_id.clone(),
        skill_name: "Library Use".to_owned(),
        adjustment: "NONE".to_owned(),
        now_unix_ms: claim_time + 104,
    };
    let first_tool_result = store
        .execute_agent_job_skill_check(&skill_check, coc7_skill_check_roll)
        .await
        .unwrap();
    let replayed_tool_result = store
        .execute_agent_job_skill_check(&skill_check, |_| {
            panic!("an exact durable tool replay must not generate a second roll")
        })
        .await
        .unwrap();
    assert_eq!(replayed_tool_result, first_tool_result);
    let result: serde_json::Value = serde_json::from_str(&first_tool_result.result_json).unwrap();
    assert_eq!(result["schema_version"], 1);
    assert_eq!(result["random_source"], "SERVER_OS_CSPRNG");
    assert_eq!(result["character_id"], character_id);
    assert_eq!(result["skill_name"], "Library Use");
    assert_eq!(result["target"], 67);
    assert_eq!(result["adjustment"], "NONE");
    assert_eq!(
        result["roll_id"].as_str(),
        Some(first_tool_result.execution_id.as_str())
    );
    assert!(result["roll"]
        .as_u64()
        .is_some_and(|roll| (1..=100).contains(&roll)));
    assert!(result["selected_tens_digit"]
        .as_u64()
        .is_some_and(|digit| digit <= 9));
    assert!(result["ones_digit"]
        .as_u64()
        .is_some_and(|digit| digit <= 9));
    assert!(matches!(
        result["success_level"].as_str(),
        Some("CRITICAL" | "EXTREME" | "HARD" | "REGULAR" | "FAILURE" | "FUMBLE")
    ));
    let receipt_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM agent_job_tool_receipts WHERE job_id = $1")
            .bind(&job_id)
            .fetch_one(&worker_pool)
            .await
            .unwrap();
    assert_eq!(receipt_count, 1);
    let mut conflicting_skill_check = skill_check.clone();
    conflicting_skill_check.skill_name = "Spot Hidden".to_owned();
    assert_eq!(
        store
            .execute_agent_job_skill_check(&conflicting_skill_check, |_| {
                panic!("an idempotency conflict must fail before generating a roll")
            })
            .await,
        Err(WorkflowStoreError::IdempotencyConflict)
    );
    assert_permission_denied(
        sqlx::query(
            "UPDATE agent_job_tool_receipts SET result_hash = result_hash WHERE job_id = $1",
        )
        .bind(&job_id)
        .execute(&worker_pool)
        .await,
    );
    assert_permission_denied(
        sqlx::query("DELETE FROM agent_job_tool_receipts WHERE job_id = $1")
            .bind(&job_id)
            .execute(&worker_pool)
            .await,
    );
    assert_permission_denied(
        sqlx::query(
            "INSERT INTO agent_job_tool_receipts (
                job_id, idempotency_key, tool_name, request_json,
                execution_id, result_json, result_hash
             ) VALUES (
                $1, 'forged', 'request_skill_check', '{}'::jsonb,
                'forged', '{}'::jsonb, $2
             )",
        )
        .bind(&job_id)
        .bind(format!("sha256:{}", "0".repeat(64)))
        .execute(&canonical_pool)
        .await,
    );
    let committing = store
        .transition_agent_job(&transition(
            &awaiting,
            WorkflowState::AwaitingTool,
            WorkflowState::Committing,
            "committing",
            claim_time + 105,
        ))
        .await
        .unwrap();
    let mut retryable = transition(
        &committing,
        WorkflowState::Committing,
        WorkflowState::RetryableFailed,
        "retryable-failure",
        claim_time + 106,
    );
    retryable.error_code = Some("TEST_COMMIT_INTERRUPTED".to_owned());
    retryable.next_attempt_at_unix_ms = Some(claim_time + 200);
    let retryable = store.transition_agent_job(&retryable).await.unwrap();
    assert_eq!(retryable.resume_state, Some(WorkflowState::Committing));
    assert!(store
        .claim_due_agent_job("worker-c", claim_time + 199, 1_000)
        .await
        .unwrap()
        .is_none());
    let recovered_commit = store
        .claim_due_agent_job("worker-c", claim_time + 200, 1_000)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        recovered_commit.resume_state,
        Some(WorkflowState::Committing)
    );
    let resumed_committing = store
        .transition_agent_job(&transition(
            &recovered_commit,
            WorkflowState::Claimed,
            WorkflowState::Committing,
            "resume-committing",
            claim_time + 201,
        ))
        .await
        .unwrap();

    let evidence = AgentJobEvidenceDraft {
        job_id: job_id.clone(),
        attempt: recovered_commit.attempt,
        phase: "provider".to_owned(),
        model_id: "ar09-model".to_owned(),
        runtime_version: "ar09-test-runtime".to_owned(),
        prompt_template_hash: format!("sha256:{}", "4".repeat(64)),
        tool_schema_hash: format!("sha256:{}", "5".repeat(64)),
        retrieval_hash: format!("sha256:{}", "6".repeat(64)),
        input_hash: format!("sha256:{}", "7".repeat(64)),
        output_hash: format!("sha256:{}", "8".repeat(64)),
        input_tokens: 42,
        output_tokens: 12,
        latency_ms: 9,
        tool_call_count: 0,
        linked_event_sequences: vec![input_event_sequence],
        visibility_label: "party_visible".to_owned(),
        retention_until_unix_ms: now + 86_400_000,
    };
    store.append_agent_job_evidence(&evidence).await.unwrap();
    store.append_agent_job_evidence(&evidence).await.unwrap();
    let mut conflicting_evidence = evidence.clone();
    conflicting_evidence.output_tokens += 1;
    assert_eq!(
        store.append_agent_job_evidence(&conflicting_evidence).await,
        Err(WorkflowStoreError::IdempotencyConflict)
    );

    let mut complete = transition(
        &resumed_committing,
        WorkflowState::Committing,
        WorkflowState::Completed,
        "completed",
        claim_time + 202,
    );
    complete.linked_event_sequences = Some(vec![input_event_sequence]);
    let completed = store.transition_agent_job(&complete).await.unwrap();
    assert_eq!(completed.state, WorkflowState::Completed);
    assert_eq!(completed.linked_event_sequences, vec![input_event_sequence]);
    assert_eq!(completed.decision_json, None);
    assert_eq!(completed.tool_result_json, None);
    assert_eq!(completed.claim_owner, None);
    assert_eq!(
        store.transition_agent_job(&complete).await.unwrap(),
        completed
    );

    let transition_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM workflow_transitions WHERE workflow_id = $1")
            .bind(&job_id)
            .fetch_one(&worker_pool)
            .await
            .unwrap();
    assert_eq!(transition_count, 9);
}
