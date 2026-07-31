use std::env;
use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Postgres, Transaction};
use trpg_ruleset_coc7::dice_roll_contract::{
    server_roll_skill_check, DiceAdjustment, SuccessLevel,
};
use trpg_runtime::durable_workflow::{
    AgentJobEnqueueDraft, AgentJobEvidenceDraft, AgentJobSkillCheckDraft,
    AgentJobSkillCheckRollDraft, AgentJobTransitionDraft, DurableAgentJob, DurableWorkflowStore,
    WorkflowState, WorkflowStoreError,
};

fn now_unix_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock must be after the Unix epoch")
        .as_millis() as i64
}

fn digest(prefix: &str, value: &str) -> String {
    format!("{prefix}{:x}", Sha256::digest(value.as_bytes()))
}

fn coc7_skill_check_roll(target: u8) -> Result<AgentJobSkillCheckRollDraft, WorkflowStoreError> {
    let roll = server_roll_skill_check(target, DiceAdjustment::None)
        .map_err(|_| WorkflowStoreError::IntegrityViolation("coc7_skill_check_failed"))?;
    let outcome = roll.outcome();
    let success_level = match outcome.success_level {
        SuccessLevel::Critical => "CRITICAL",
        SuccessLevel::Extreme => "EXTREME",
        SuccessLevel::Hard => "HARD",
        SuccessLevel::Regular => "REGULAR",
        SuccessLevel::Failure => "FAILURE",
        SuccessLevel::Fumble => "FUMBLE",
    };
    Ok(AgentJobSkillCheckRollDraft {
        execution_id: roll.roll_id().to_owned(),
        roll: outcome.roll,
        selected_tens_digit: outcome.selected_tens_digit,
        ones_digit: outcome.ones_digit,
        success_level: success_level.to_owned(),
    })
}

async fn assert_service_connection(pool: &PgPool, service_role: &str) {
    let (current_user, is_member, superuser, createdb, createrole, replication, bypass_rls): (
        String,
        bool,
        bool,
        bool,
        bool,
        bool,
        bool,
    ) = sqlx::query_as(
        r#"
        SELECT current_user,
               pg_has_role(current_user, $1, 'MEMBER'),
               role.rolsuper,
               role.rolcreatedb,
               role.rolcreaterole,
               role.rolreplication,
               role.rolbypassrls
          FROM pg_roles AS role
         WHERE role.rolname = current_user
        "#,
    )
    .bind(service_role)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_ne!(current_user, "postgres");
    assert_ne!(current_user, "trpg_database_owner");
    assert!(is_member, "{current_user} must inherit {service_role}");
    assert!(!superuser);
    assert!(!createdb);
    assert!(!createrole);
    assert!(!replication);
    assert!(!bypass_rls);
}

fn assert_permission_denied(result: Result<sqlx::postgres::PgQueryResult, sqlx::Error>) {
    let error = result.expect_err("least-privilege operation unexpectedly succeeded");
    let sqlx::Error::Database(database_error) = error else {
        panic!("expected PostgreSQL permission denial, got {error}");
    };
    assert_eq!(database_error.code().as_deref(), Some("42501"));
}

struct CanonicalAgentJobFixture<'a> {
    suffix: &'a str,
    campaign_id: &'a str,
    contract_id: &'a str,
    actor_id: &'a str,
    job_id: &'a str,
    stream_id: &'a str,
    payload: &'a str,
}

async fn seed_canonical_agent_job_request(
    transaction: &mut Transaction<'_, Postgres>,
    fixture: &CanonicalAgentJobFixture<'_>,
) -> i64 {
    let CanonicalAgentJobFixture {
        suffix,
        campaign_id,
        contract_id,
        actor_id,
        job_id,
        stream_id,
        payload,
    } = fixture;
    let command_id = format!("ar09-command-{suffix}");
    let commit_id = format!("ar09-commit-{suffix}");
    let event_idempotency_key = format!("ar09-event-idempotency-{suffix}:0000");
    let commit_idempotency_key = format!("ar09-event-idempotency-{suffix}");
    let correlation_id = format!("ar09-correlation-{suffix}");
    let causation_id = format!("ar09-causation-{suffix}");
    let trace_id = format!("ar09-trace-{suffix}");
    let request_hash = digest("sha256:", &format!("request-{suffix}"));
    let event_hash = digest("hmac-sha256:", &format!("event-{suffix}"));
    let audit_hash = digest("hmac-sha256:", &format!("audit-{suffix}"));
    let witness_hash = digest("hmac-sha256:", &format!("witness-{suffix}"));
    let batch_hash = digest("sha256:", &format!("batch-{suffix}"));
    let ciphertext = vec![1_u8; 16];
    let nonce = vec![2_u8; 12];

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
            data_subject_id, projection_targets, event_integrity_version
        ) VALUES (
            'AgentJobRequested', $1, $2, 0, 'ai_kp', 1, 'party_visible',
            'system_fixture', $3, $4, $5, $6, $7::jsonb,
            $8, 1, $4, 'agent_job', $9, $10, $4, 'not_applicable', $11,
            $12, $13, 1, 'canonical_commit', $14, 'formal_commit',
            'verified_hmac', $7, 'system',
            jsonb_build_object('kind', 'workload', 'role', 'agent_worker'),
            $15, 'ar09-test-key', $16, 'not_applicable', '[]'::jsonb, 3
        )
        RETURNING sequence
        "#,
    )
    .bind(&command_id)
    .bind(&event_idempotency_key)
    .bind(format!("ar09-provenance-{suffix}"))
    .bind(actor_id)
    .bind(&correlation_id)
    .bind(&causation_id)
    .bind(payload)
    .bind(campaign_id)
    .bind(job_id)
    .bind(contract_id)
    .bind(&trace_id)
    .bind(&event_hash)
    .bind(stream_id)
    .bind(&request_hash)
    .bind(&ciphertext)
    .bind(&nonce)
    .fetch_one(&mut **transaction)
    .await
    .unwrap();

    sqlx::query(
        r#"
        INSERT INTO event_outbox (
            event_id, event_sequence, nats_subject, idempotency_key,
            visibility_label, correlation_id, causation_id, payload_json,
            commit_id, campaign_id, stream_id, event_schema_version,
            idempotency_operation, request_hash, request_hash_source,
            integrity_status, visibility_subject, payload_ciphertext,
            payload_key_reference, payload_nonce, data_subject_id
        ) VALUES (
            $1, $1, 'trpg.events.appended', $2, 'party_visible', $3, $4,
            $5::jsonb, $6, $7, $8, 1, 'canonical_commit', $9,
            'formal_commit', 'verified_hmac', 'not_applicable', $10,
            'ar09-test-key', $11, 'not_applicable'
        )
        "#,
    )
    .bind(input_event_sequence)
    .bind(format!("outbox:{event_idempotency_key}"))
    .bind(&correlation_id)
    .bind(&causation_id)
    .bind(payload)
    .bind(&commit_id)
    .bind(campaign_id)
    .bind(stream_id)
    .bind(&request_hash)
    .bind(&ciphertext)
    .bind(&nonce)
    .execute(&mut **transaction)
    .await
    .unwrap();

    let audit_sequence: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO canonical_audit_log (
            sequence, commit_id, campaign_id, actor_id, actor_origin,
            authentication_reference, resource_type, resource_id, action,
            requested_role, visibility_label, visibility_subject,
            provenance_kind, provenance_reference, provenance_recorded_by,
            decision, openfga_decision_id, openfga_policy_revision,
            opa_decision_id, opa_policy_revision, trace_id, correlation_id,
            causation_id, event_batch_hash, witness_prepare_sequence,
            witness_prepare_hash, integrity_version, integrity_key_id,
            previous_hash, record_hash
        )
        SELECT 0, $1, $2, $3, 'workload', $4, 'agent_job', $5,
               'request_agent_job', 'system', 'party_visible',
               'not_applicable', 'system_fixture', $6, $3, 'PERMIT',
               $7, 'ar09-openfga-v1', $8, 'ar09-opa-v1', $9, $10, $11,
               $12, 1, $13, 3, 'ar09-test-key',
               COALESCE(
                   (SELECT record_hash
                      FROM canonical_audit_log
                     ORDER BY sequence DESC
                     LIMIT 1),
                   'hmac-sha256:' || repeat('0', 64)
               ),
               $14
        RETURNING sequence
        "#,
    )
    .bind(&commit_id)
    .bind(campaign_id)
    .bind(actor_id)
    .bind(format!("ar09-authentication-{suffix}"))
    .bind(job_id)
    .bind(format!("ar09-provenance-{suffix}"))
    .bind(format!("ar09-openfga-{suffix}"))
    .bind(format!("ar09-opa-{suffix}"))
    .bind(&trace_id)
    .bind(&correlation_id)
    .bind(&causation_id)
    .bind(&batch_hash)
    .bind(&witness_hash)
    .bind(&audit_hash)
    .fetch_one(&mut **transaction)
    .await
    .unwrap();

    sqlx::query(
        r#"
        INSERT INTO formal_commits (
            commit_id, campaign_id, idempotency_key, request_hash,
            expected_version, first_event_sequence, last_event_sequence,
            first_stream_version, last_stream_version, audit_sequence,
            witness_prepare_sequence, witness_prepare_hash, stream_id,
            idempotency_operation, status, result_event_sequence,
            response_payload
        ) VALUES (
            $1, $2, $3, $4, 0, $5, $5, 1, 1, $6, 1, $7, $8,
            'canonical_commit', 'committed', $5,
            jsonb_build_object(
                'first_event_sequence', $5::bigint,
                'last_event_sequence', $5::bigint,
                'first_stream_version', 1::bigint,
                'last_stream_version', 1::bigint
            )
        )
        "#,
    )
    .bind(&commit_id)
    .bind(campaign_id)
    .bind(&commit_idempotency_key)
    .bind(&request_hash)
    .bind(input_event_sequence)
    .bind(audit_sequence)
    .bind(&witness_hash)
    .bind(stream_id)
    .execute(&mut **transaction)
    .await
    .unwrap();

    input_event_sequence
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
async fn agent_job_cas_lease_recovery_and_evidence_are_durable() {
    let fixture_database_url = env::var("AR09_AGENT_JOB_FIXTURE_DATABASE_URL")
        .expect("AR09_AGENT_JOB_FIXTURE_DATABASE_URL is required for canonical fixture setup");
    let api_database_url = env::var("AR09_AGENT_JOB_API_DATABASE_URL")
        .expect("AR09_AGENT_JOB_API_DATABASE_URL is required for the API-role gate");
    let worker_database_url = env::var("AR09_AGENT_JOB_WORKER_DATABASE_URL")
        .expect("AR09_AGENT_JOB_WORKER_DATABASE_URL is required for the worker-role gate");
    let canonical_database_url = env::var("AR09_AGENT_JOB_CANONICAL_DATABASE_URL")
        .expect("AR09_AGENT_JOB_CANONICAL_DATABASE_URL is required for the canonical-role gate");
    let now = now_unix_ms();
    let suffix = format!("{}-{}", std::process::id(), now);
    let campaign_id = format!("ar09-campaign-{suffix}");
    let contract_id = format!("ar09-contract-{suffix}");
    let actor_id = format!("ar09-keeper-{suffix}");
    let player_id = format!("ar09-player-{suffix}");
    let character_id = format!("ar09-character-{suffix}");
    let job_id = format!("ar09-job-{suffix}");
    let stream_id = format!("ar09-stream-{suffix}");
    let payload = r#"{"protected_payload":{"kind":"agent_job_test_input"}}"#;

    let fixture_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&fixture_database_url)
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
    .execute(&fixture_pool)
    .await
    .unwrap();
    let mut fixture_transaction = fixture_pool.begin().await.unwrap();
    let input_event_sequence = seed_canonical_agent_job_request(
        &mut fixture_transaction,
        &CanonicalAgentJobFixture {
            suffix: &suffix,
            campaign_id: &campaign_id,
            contract_id: &contract_id,
            actor_id: &actor_id,
            job_id: &job_id,
            stream_id: &stream_id,
            payload,
        },
    )
    .await;
    fixture_transaction.commit().await.unwrap();
    let mut projection_transaction = fixture_pool.begin().await.unwrap();
    sqlx::query("SET LOCAL session_replication_role = 'replica'")
        .execute(&mut *projection_transaction)
        .await
        .unwrap();
    sqlx::query(
        r#"
        INSERT INTO users (
            user_id, login_normalized, password_hash, global_role
        ) VALUES ($1, $2, 'ar09-fixture-hash', 'USER')
        "#,
    )
    .bind(&player_id)
    .bind(format!("ar09-player-{suffix}"))
    .execute(&mut *projection_transaction)
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO campaigns (
            campaign_id, owner_user_id, authority_contract_id, title, state,
            version, created_at, visibility_label, visibility_subject,
            provenance_kind, provenance_reference, provenance_recorded_by,
            last_event_sequence
        ) VALUES (
            $1, $2, $3, 'AR09 durable tool receipt', 'ACTIVE', 1, now(),
            'party_visible', 'not_applicable', 'system_fixture', $4, $2, $5
        )
        "#,
    )
    .bind(&campaign_id)
    .bind(&player_id)
    .bind(&contract_id)
    .bind(format!("ar09-projection-{suffix}"))
    .bind(input_event_sequence)
    .execute(&mut *projection_transaction)
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO characters (
            character_id, campaign_id, owner_user_id, display_name, state,
            current_sheet_version, initial_version_locked, version,
            visibility_label, visibility_subject, provenance_kind,
            provenance_reference, provenance_recorded_by, last_event_sequence
        ) VALUES (
            $1, $2, $3, 'AR09 Investigator', 'APPROVED', 1, TRUE, 1,
            'party_visible', 'not_applicable', 'system_fixture', $4, $3, $5
        )
        "#,
    )
    .bind(&character_id)
    .bind(&campaign_id)
    .bind(&player_id)
    .bind(format!("ar09-character-{suffix}"))
    .bind(input_event_sequence)
    .execute(&mut *projection_transaction)
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO character_sheet_versions (
            sheet_version_id, character_id, version, sheet_json, locked,
            visibility_label, visibility_subject, provenance_kind,
            provenance_reference, provenance_recorded_by, campaign_id,
            last_event_sequence
        ) VALUES (
            $1, $2, 1, '{"skills":{"Library Use":67}}'::jsonb, TRUE,
            'party_visible', 'not_applicable', 'system_fixture', $3, $4, $5, $6
        )
        "#,
    )
    .bind(format!("ar09-sheet-{suffix}"))
    .bind(&character_id)
    .bind(format!("ar09-sheet-{suffix}"))
    .bind(&player_id)
    .bind(&campaign_id)
    .bind(input_event_sequence)
    .execute(&mut *projection_transaction)
    .await
    .unwrap();
    projection_transaction.commit().await.unwrap();

    let api_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&api_database_url)
        .await
        .unwrap();
    let worker_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&worker_database_url)
        .await
        .unwrap();
    let canonical_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&canonical_database_url)
        .await
        .unwrap();
    assert_service_connection(&api_pool, "trpg_api_service").await;
    assert_service_connection(&worker_pool, "trpg_worker_service").await;
    assert_service_connection(&canonical_pool, "trpg_canonical_service").await;
    let canonical_can_insert_events: bool =
        sqlx::query_scalar("SELECT has_table_privilege(current_user, 'event_store', 'INSERT')")
            .fetch_one(&canonical_pool)
            .await
            .unwrap();
    assert!(canonical_can_insert_events);
    assert_permission_denied(
        sqlx::query(
            "UPDATE workflow_instances SET heartbeat_at = heartbeat_at WHERE workflow_id = $1",
        )
        .bind(&job_id)
        .execute(&api_pool)
        .await,
    );
    assert_permission_denied(
        sqlx::query("INSERT INTO agent_job_evidence (job_id) VALUES ($1)")
            .bind(&job_id)
            .execute(&api_pool)
            .await,
    );
    assert_permission_denied(
        sqlx::query("INSERT INTO event_store (event_type) VALUES ('AgentEscapeAttempt')")
            .execute(&worker_pool)
            .await,
    );
    assert_permission_denied(
        sqlx::query(
            "UPDATE workflow_instances SET heartbeat_at = heartbeat_at WHERE workflow_id = $1",
        )
        .bind(&job_id)
        .execute(&canonical_pool)
        .await,
    );

    let api_store = DurableWorkflowStore::connect(&api_database_url)
        .await
        .unwrap();
    let store = DurableWorkflowStore::connect(&worker_database_url)
        .await
        .unwrap();
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
    let enqueued = api_store.enqueue_agent_job(&draft).await.unwrap();
    assert_eq!(enqueued.state, WorkflowState::Requested);
    assert_eq!(api_store.enqueue_agent_job(&draft).await.unwrap(), enqueued);
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
