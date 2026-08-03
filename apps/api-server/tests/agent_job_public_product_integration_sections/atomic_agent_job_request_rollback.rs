use super::*;

pub(super) fn assert_agent_job_request_rollback(
    application: &ApiApplication,
    runtime: &tokio::runtime::Runtime,
    fixture_pool: &sqlx::PgPool,
    campaign_id: &str,
    token: &str,
    suffix: &str,
) {
    // The canonical event and durable workflow are one database transaction.
    // A pre-existing workflow identity forces the atomic projection to fail;
    // the AgentJobRequested append must roll back with it.
    let conflicting_job_id = format!("job_ar09_atomic_conflict_{suffix}");
    runtime
        .block_on(
            sqlx::query(
                r#"
                INSERT INTO workflow_instances (
                    workflow_id, campaign_id, workflow_type, state, version,
                    input_json
                ) VALUES ($1, $2, 'test_conflict', 'PENDING', 0, '{}')
                "#,
            )
            .bind(&conflicting_job_id)
            .bind(campaign_id)
            .execute(fixture_pool),
        )
        .expect("seed conflicting workflow identity");
    let conflict_request = request(
        &format!("/api/v1/campaigns/{campaign_id}/agent-jobs"),
        token,
        json!({
            "command": {
                "command_id": format!("command_{conflicting_job_id}"),
                "idempotency_key": format!("idempotency_{conflicting_job_id}"),
                "expected_version": 0,
                "correlation_id": format!("correlation_{conflicting_job_id}"),
                "causation_id": format!("causation_{conflicting_job_id}"),
                "trace_id": format!("trace_{conflicting_job_id}")
            },
            "campaign_id": campaign_id,
            "job_id": conflicting_job_id,
            "rag_snapshot_id": format!("rag_{conflicting_job_id}"),
            "input": {"kind": "atomic_conflict_probe"},
            "deadline_unix_ms": i64::try_from(now_unix_ms() + 240_000)
                .expect("conflict deadline fits i64")
        }),
    );
    let conflict = call(application, &conflict_request);
    assert_eq!(
        conflict.status, 409,
        "unexpected atomic conflict: {}",
        conflict.body
    );
    let rolled_back_counts: (i64, i64) = runtime
        .block_on(
            sqlx::query_as(
                r#"
                SELECT
                    (SELECT count(*) FROM event_store
                      WHERE campaign_id = $1 AND stream_id = $2
                        AND event_type = 'AgentJobRequested'),
                    (SELECT count(*) FROM agent_jobs WHERE job_id = $2)
                "#,
            )
            .bind(campaign_id)
            .bind(&conflicting_job_id)
            .fetch_one(fixture_pool),
        )
        .expect("verify atomic Agent Job rollback");
    assert_eq!(rolled_back_counts, (0, 0));
}
