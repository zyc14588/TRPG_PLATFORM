use std::env;
use std::time::{SystemTime, UNIX_EPOCH};

use sqlx::postgres::PgPoolOptions;
use trpg_runtime::durable_workflow::{
    AgentJobEnqueueDraft, AgentJobEvidenceDraft, AgentJobTransitionDraft, DurableAgentJob,
    DurableWorkflowStore, WorkflowState, WorkflowStoreError,
};

fn now_unix_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock must be after the Unix epoch")
        .as_millis() as i64
}

fn transition(
    job: &DurableAgentJob,
    from_state: WorkflowState,
    to_state: WorkflowState,
    suffix: &str,
    now: i64,
) -> AgentJobTransitionDraft {
    AgentJobTransitionDraft {
        job_id: job.job_id.clone(),
        claim_owner: job.claim_owner.clone().unwrap(),
        claim_token: job.claim_token.clone().unwrap(),
        expected_version: job.version,
        from_state,
        to_state,
        idempotency_key: format!("{}-{suffix}", job.job_id),
        correlation_id: format!("{}-correlation", job.job_id),
        causation_id: format!("{}-causation", job.job_id),
        decision_json: None,
        tool_result_json: None,
        linked_event_sequences: None,
        error_code: None,
        next_attempt_at_unix_ms: None,
        now_unix_ms: now,
    }
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires a migrated disposable PostgreSQL database and superuser seed access"]
async fn agent_job_cas_lease_recovery_and_evidence_are_durable() {
    let database_url = env::var("AR09_AGENT_JOB_DATABASE_URL")
        .expect("AR09_AGENT_JOB_DATABASE_URL is required for this real database gate");
    let now = now_unix_ms();
    let suffix = format!("{}-{}", std::process::id(), now);
    let campaign_id = format!("ar09-campaign-{suffix}");
    let contract_id = format!("ar09-contract-{suffix}");
    let actor_id = format!("ar09-keeper-{suffix}");
    let job_id = format!("ar09-job-{suffix}");
    let stream_id = format!("ar09-stream-{suffix}");
    let payload = r#"{"protected_payload":{"kind":"agent_job_test_input"}}"#;

    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&database_url)
        .await
        .unwrap();
    let mut connection = pool.acquire().await.unwrap();
    sqlx::query("SET session_replication_role = replica")
        .execute(&mut *connection)
        .await
        .unwrap();
    sqlx::raw_sql(
        "DELETE FROM agent_job_evidence WHERE job_id LIKE 'ar09-job-%'; \
         DELETE FROM agent_job_approvals WHERE job_id LIKE 'ar09-job-%'; \
         DELETE FROM agent_jobs WHERE job_id LIKE 'ar09-job-%'; \
         DELETE FROM workflow_transitions WHERE workflow_id LIKE 'ar09-job-%'; \
         DELETE FROM workflow_instances WHERE workflow_id LIKE 'ar09-job-%'; \
         DELETE FROM event_store WHERE campaign_id LIKE 'ar09-campaign-%'; \
         DELETE FROM authority_contracts WHERE campaign_id LIKE 'ar09-campaign-%';",
    )
    .execute(&mut *connection)
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO authority_contracts (
            contract_id, campaign_id, authority_mode, authority_owner,
            contract_version, ruleset_version, house_rules_version,
            scenario_version, prompt_version, agent_pack_version,
            tool_schema_version, safety_profile_version,
            ai_provider_snapshot, model_route_snapshot,
            character_sheet_template_version, created_at, locked
        ) VALUES (
            $1, $2, 'AI_KP', $3, 1, 'coc7-v1', 'none-v1',
            'scenario-v1', 'prompt-v1', 'agent-pack-v1',
            'tool-schema-v1', 'safety-v1', 'provider-snapshot-v1',
            'route-snapshot-v1', 'sheet-v1', now(), TRUE
        )
        "#,
    )
    .bind(&contract_id)
    .bind(&campaign_id)
    .bind(&actor_id)
    .execute(&mut *connection)
    .await
    .unwrap();
    let input_event_sequence: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO event_store (
            event_type, command_id, idempotency_key, expected_version,
            authority_mode, authority_contract_version, visibility_label,
            fact_provenance_kind, fact_provenance_reference,
            fact_recorded_by, correlation_id, causation_id, payload_json,
            campaign_id, stream_version, authenticated_actor_id,
            resource_type, resource_id, authority_contract_id,
            authority_owner, visibility_subject, trace_id,
            event_integrity_hash, stream_id, event_schema_version,
            idempotency_operation, request_hash, request_hash_source,
            integrity_status, payload_integrity_source,
            authenticated_actor_role, authenticated_actor_origin,
            payload_ciphertext, payload_key_reference, payload_nonce,
            data_subject_id
        ) VALUES (
            'AgentJobRequested', $1, $2, 0, 'ai_kp', 1, 'party_visible',
            'system_fixture', $3, $4, $5, $6, $7::jsonb,
            $8, 1, $4, 'campaign', $8, $9, $4, 'not_applicable', $10,
            $11, $12, 1, 'agent_job_test_seed', $13, 'formal_commit',
            'verified_hmac', $7, 'system',
            jsonb_build_object('kind', 'workload', 'role', 'agent_worker'),
            decode(repeat('01', 16), 'hex'), 'ar09-test-key',
            decode(repeat('02', 12), 'hex'), 'not_applicable'
        )
        RETURNING sequence
        "#,
    )
    .bind(format!("ar09-command-{suffix}"))
    .bind(format!("ar09-event-idempotency-{suffix}"))
    .bind(format!("ar09-provenance-{suffix}"))
    .bind(&actor_id)
    .bind(format!("ar09-correlation-{suffix}"))
    .bind(format!("ar09-causation-{suffix}"))
    .bind(payload)
    .bind(&campaign_id)
    .bind(&contract_id)
    .bind(format!("ar09-trace-{suffix}"))
    .bind(format!("hmac-sha256:{}", "1".repeat(64)))
    .bind(&stream_id)
    .bind(format!("sha256:{}", "2".repeat(64)))
    .fetch_one(&mut *connection)
    .await
    .unwrap();
    sqlx::query("SET session_replication_role = origin")
        .execute(&mut *connection)
        .await
        .unwrap();
    drop(connection);

    let store = DurableWorkflowStore::connect(&database_url).await.unwrap();
    store.check_agent_job_readiness().await.unwrap();
    let draft = AgentJobEnqueueDraft {
        job_id: job_id.clone(),
        campaign_id: campaign_id.clone(),
        actor_id: actor_id.clone(),
        agent_kind: "ai_keeper_orchestrator".to_owned(),
        authority_contract_id: contract_id,
        authority_mode: "AI_KP".to_owned(),
        authority_contract_version: 1,
        input_event_sequence,
        input_stream_version: 1,
        visibility_scope_json:
            r#"{"allowed_labels":["party_visible"],"subject_id":null,"output_label":"party_visible"}"#
                .to_owned(),
        rag_snapshot_id: format!("ar09-rag-{suffix}"),
        provider_id: "ar09-provider".to_owned(),
        provider_type: "cloud".to_owned(),
        model_id: "ar09-model".to_owned(),
        model_artifact_sha256: format!("sha256:{}", "3".repeat(64)),
        route_authorization_event_id: format!("ar09-route-{suffix}"),
        prompt_template_id: "keeper-turn".to_owned(),
        prompt_template_version: "v1".to_owned(),
        tool_schema_version: "tool-schema-v1".to_owned(),
        idempotency_key: format!("ar09-job-idempotency-{suffix}"),
        deadline_unix_ms: now + 60_000,
    };
    let enqueued = store.enqueue_agent_job(&draft).await.unwrap();
    assert_eq!(enqueued.state, WorkflowState::Requested);
    assert_eq!(store.enqueue_agent_job(&draft).await.unwrap(), enqueued);
    let context = store.load_agent_job_context(&job_id).await.unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&context.input_payload_json).unwrap(),
        serde_json::from_str::<serde_json::Value>(payload).unwrap(),
    );
    assert!(context.chunks.is_empty());

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
    let committing = store
        .transition_agent_job(&transition(
            &awaiting,
            WorkflowState::AwaitingTool,
            WorkflowState::Committing,
            "committing",
            claim_time + 104,
        ))
        .await
        .unwrap();
    let mut retryable = transition(
        &committing,
        WorkflowState::Committing,
        WorkflowState::RetryableFailed,
        "retryable-failure",
        claim_time + 105,
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
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(transition_count, 9);
}
