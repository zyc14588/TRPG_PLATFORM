mod support;

use std::collections::HashSet;
use std::time::Duration;

use sqlx::Row;
use trpg_data_eventing::outbox_projection_workers::{
    EventWorkerError, OutboxFailureCode, OutboxLeasePolicy, PostgresOutboxLeaseRepository,
};

use support::{draft, P04PostgresHarness};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn postgres_outbox_claims_are_exclusive_leased_backed_off_and_dead_lettered() {
    let harness = P04PostgresHarness::reset().await;
    let policy = OutboxLeasePolicy {
        lease_duration: Duration::from_millis(100),
        initial_backoff: Duration::from_millis(250),
        maximum_backoff: Duration::from_secs(1),
        maximum_attempts: 3,
    };
    let worker_a =
        PostgresOutboxLeaseRepository::new(harness.primary.clone(), "p04_outbox_worker_a", policy)
            .unwrap();
    let worker_b =
        PostgresOutboxLeaseRepository::new(harness.primary.clone(), "p04_outbox_worker_b", policy)
            .unwrap();

    for index in 0..4 {
        harness
            .store
            .commit(&draft(
                &format!("campaign_claim_{index}"),
                &format!("scene_claim_{index}"),
                &format!("claim_{index}"),
                0,
                &["OutboxClaimProbe"],
            ))
            .await
            .unwrap();
    }
    let (claims_a, claims_b) = tokio::join!(worker_a.claim_batch(4), worker_b.claim_batch(4));
    let claims_a = claims_a.unwrap();
    let claims_b = claims_b.unwrap();
    let ids_a: HashSet<i64> = claims_a.iter().map(|claim| claim.outbox_id).collect();
    let ids_b: HashSet<i64> = claims_b.iter().map(|claim| claim.outbox_id).collect();
    assert!(ids_a.is_disjoint(&ids_b));
    assert_eq!(ids_a.len() + ids_b.len(), 4);
    for claim in &claims_a {
        claim.validate_for_publish().unwrap();
        worker_a.mark_published(claim).await.unwrap();
    }
    for claim in &claims_b {
        claim.validate_for_publish().unwrap();
        worker_b.mark_published(claim).await.unwrap();
    }

    // A worker crash is represented by an unacknowledged claim. Before lease
    // expiry no peer can claim it; after expiry a peer atomically takes over.
    harness
        .store
        .commit(&draft(
            "campaign_lease_recovery",
            "scene_lease_recovery",
            "lease_recovery",
            0,
            &["OutboxLeaseProbe"],
        ))
        .await
        .unwrap();
    let crashed_claim = worker_a.claim_batch(1).await.unwrap().remove(0);
    assert!(!crashed_claim.claim_token.trim().is_empty());
    assert!(worker_b.claim_batch(1).await.unwrap().is_empty());
    tokio::time::sleep(Duration::from_millis(140)).await;
    let recovered_claim = worker_b.claim_batch(1).await.unwrap().remove(0);
    assert_eq!(recovered_claim.outbox_id, crashed_claim.outbox_id);
    assert_ne!(recovered_claim.claim_token, crashed_claim.claim_token);
    assert_eq!(
        worker_a.mark_published(&crashed_claim).await.unwrap_err(),
        EventWorkerError::ClaimLost
    );
    worker_b.mark_published(&recovered_claim).await.unwrap();

    // Lease expiry removes write authority even before another worker takes
    // over. Reusing the same worker id after a restart must not resurrect the
    // stale task because every claim has an independent fencing token.
    harness
        .store
        .commit(&draft(
            "campaign_expired_lease",
            "scene_expired_lease",
            "expired_lease",
            0,
            &["ExpiredLeaseProbe"],
        ))
        .await
        .unwrap();
    let expired_claim = worker_a.claim_batch(1).await.unwrap().remove(0);
    tokio::time::sleep(Duration::from_millis(140)).await;
    assert_eq!(
        worker_a.mark_published(&expired_claim).await.unwrap_err(),
        EventWorkerError::ClaimLost
    );
    let restarted_worker_a =
        PostgresOutboxLeaseRepository::new(harness.primary.clone(), "p04_outbox_worker_a", policy)
            .unwrap();
    let replacement_claim = restarted_worker_a.claim_batch(1).await.unwrap().remove(0);
    assert_eq!(replacement_claim.outbox_id, expired_claim.outbox_id);
    assert_ne!(replacement_claim.claim_token, expired_claim.claim_token);
    assert_eq!(
        worker_a.mark_published(&expired_claim).await.unwrap_err(),
        EventWorkerError::ClaimLost
    );
    restarted_worker_a
        .mark_published(&replacement_claim)
        .await
        .unwrap();

    // Failure releases ownership but `available_at` prevents a hot retry.
    // Repeated classified failures end in a persistent, observable DLQ row.
    harness
        .store
        .commit(&draft(
            "campaign_delivery_failure",
            "scene_delivery_failure",
            "delivery_failure",
            0,
            &["OutboxFailureProbe"],
        ))
        .await
        .unwrap();
    let mut failed_claim = worker_a.claim_batch(1).await.unwrap().remove(0);
    let first_failure = worker_a
        .mark_failed(&failed_claim, OutboxFailureCode::JetStreamPublishFailed)
        .await
        .unwrap();
    assert_eq!(first_failure.retry_count, 1);
    assert!(!first_failure.dead_lettered);
    assert!(worker_a.claim_batch(1).await.unwrap().is_empty());

    for expected_retry in [2, 3] {
        sqlx::query(
            "UPDATE event_outbox SET available_at = now() - interval '1 second' WHERE outbox_id = $1",
        )
        .bind(failed_claim.outbox_id)
        .execute(&harness.primary)
        .await
        .unwrap();
        failed_claim = worker_a.claim_batch(1).await.unwrap().remove(0);
        let disposition = worker_a
            .mark_failed(&failed_claim, OutboxFailureCode::JetStreamPublishFailed)
            .await
            .unwrap();
        assert_eq!(disposition.retry_count, expected_retry);
        assert_eq!(disposition.dead_lettered, expected_retry == 3);
    }
    assert!(worker_a.claim_batch(1).await.unwrap().is_empty());
    assert_eq!(worker_a.dead_letter_count().await.unwrap(), 1);
    let dead_letter = sqlx::query(
        "SELECT delivery_status, last_error, claimed_at, claim_owner, claim_token, locked_until FROM event_outbox WHERE outbox_id = $1",
    )
    .bind(failed_claim.outbox_id)
    .fetch_one(&harness.primary)
    .await
    .unwrap();
    assert_eq!(
        dead_letter.get::<String, _>("delivery_status"),
        "dead_lettered"
    );
    assert_eq!(
        dead_letter
            .get::<Option<String>, _>("last_error")
            .as_deref(),
        Some("JETSTREAM_PUBLISH_FAILED")
    );
    assert!(dead_letter
        .get::<Option<chrono::DateTime<chrono::Utc>>, _>("claimed_at")
        .is_none());
    assert!(dead_letter
        .get::<Option<String>, _>("claim_owner")
        .is_none());
    assert!(dead_letter
        .get::<Option<String>, _>("claim_token")
        .is_none());
    assert!(dead_letter
        .get::<Option<chrono::DateTime<chrono::Utc>>, _>("locked_until")
        .is_none());

    // A claimed row without a complete lease identity must be rejected. The
    // explicit IS NOT NULL guards avoid PostgreSQL CHECK three-valued logic
    // treating a NULL predicate result as valid.
    harness
        .store
        .commit(&draft(
            "campaign_incomplete_lease",
            "scene_incomplete_lease",
            "incomplete_lease",
            0,
            &["IncompleteLeaseProbe"],
        ))
        .await
        .unwrap();
    let incomplete_lease = sqlx::query(
        r#"
        UPDATE event_outbox
           SET delivery_status = 'claimed',
               claimed_at = now(),
               claim_owner = NULL,
               claim_token = NULL,
               locked_until = NULL
         WHERE campaign_id = 'campaign_incomplete_lease'
        "#,
    )
    .execute(&harness.primary)
    .await
    .expect_err("claimed rows require an owner and lease expiry");
    assert!(incomplete_lease
        .to_string()
        .contains("event_outbox_delivery_state_consistent"));
    let missing_fencing_token = sqlx::query(
        r#"
        UPDATE event_outbox
           SET delivery_status = 'claimed',
               claimed_at = now(),
               claim_owner = 'p04_outbox_worker_a',
               claim_token = NULL,
               locked_until = now() + interval '60 seconds'
         WHERE campaign_id = 'campaign_incomplete_lease'
        "#,
    )
    .execute(&harness.primary)
    .await
    .expect_err("claimed rows require an independent fencing token");
    assert!(missing_fencing_token
        .to_string()
        .contains("event_outbox_delivery_state_consistent"));

    // Negative regression for AUD-068: a second, inconsistent canonical
    // event reference cannot be inserted even when all copied metadata is
    // otherwise valid.
    let source_sequence: i64 = sqlx::query_scalar(
        "SELECT sequence FROM event_store WHERE campaign_id = 'campaign_claim_0'",
    )
    .fetch_one(&harness.primary)
    .await
    .unwrap();
    let other_sequence: i64 = sqlx::query_scalar(
        "SELECT sequence FROM event_store WHERE campaign_id = 'campaign_claim_1'",
    )
    .fetch_one(&harness.primary)
    .await
    .unwrap();
    let wrong_reference = sqlx::query(
        r#"
        INSERT INTO event_outbox (
            event_id, event_sequence, nats_subject, idempotency_key,
            visibility_label, correlation_id, causation_id, payload_json,
            campaign_id, stream_id, event_schema_version,
            idempotency_operation, request_hash, request_hash_source,
            integrity_status
        )
        SELECT $1, $2, nats_subject,
               'wrong_event_reference_probe', visibility_label,
               correlation_id, causation_id, payload_json, campaign_id,
               stream_id, event_schema_version, idempotency_operation,
               request_hash, request_hash_source, integrity_status
          FROM event_outbox
         WHERE event_sequence = $1
        "#,
    )
    .bind(source_sequence)
    .bind(other_sequence)
    .execute(&harness.primary)
    .await
    .expect_err("inconsistent event_id/event_sequence must be rejected");
    let wrong_reference_detail = wrong_reference.to_string();
    assert!(
        wrong_reference_detail.contains("event_outbox_event_reference_consistent")
            || wrong_reference_detail.contains("outbox metadata does not match canonical event"),
        "unexpected cross-event rejection: {wrong_reference_detail}"
    );
}
