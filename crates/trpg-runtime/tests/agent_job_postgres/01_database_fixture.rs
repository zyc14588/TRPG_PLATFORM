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
