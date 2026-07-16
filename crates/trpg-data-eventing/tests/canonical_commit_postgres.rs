use std::env;
use std::time::Duration;

use sqlx::{Connection, PgConnection, PgPool, Row};
use trpg_data_eventing::event_store_sqlx_outbox_projection::{
    AtomicCommitDraft, CanonicalEventDraft, CanonicalStoreError, PolicyAuditDraft,
    PostgresCanonicalStore, RecoveryReport,
};
use trpg_data_eventing::persistence::FormalCommitRecord;

const KEY: &[u8; 32] = &[0x9c; 32];

fn database_urls() -> (String, String) {
    let primary = env::var("P02_CANONICAL_DATABASE_URL")
        .expect("P02_CANONICAL_DATABASE_URL is required for the real PostgreSQL gate");
    let witness = env::var("P02_CANONICAL_WITNESS_DATABASE_URL")
        .expect("P02_CANONICAL_WITNESS_DATABASE_URL is required for the real PostgreSQL gate");
    (primary, witness)
}

fn draft(commit_id: &str, expected_version: i64, event_types: &[&str]) -> AtomicCommitDraft {
    AtomicCommitDraft {
        commit_id: commit_id.to_owned(),
        campaign_id: "campaign_atomic_commit".to_owned(),
        stream_id: "campaign_atomic_commit".to_owned(),
        idempotency_key: format!("idempotency_{commit_id}"),
        expected_version,
        command_id: format!("command_{commit_id}"),
        authenticated_actor_id: "workflow_atomic_commit".to_owned(),
        authority_mode: "human_kp".to_owned(),
        authority_contract_version: 1,
        authority_contract_id: "authority_campaign_atomic_commit_1".to_owned(),
        authority_owner: "keeper_atomic_commit".to_owned(),
        visibility_label: "party_visible".to_owned(),
        visibility_subject: "not_applicable".to_owned(),
        provenance_kind: "rules_engine_decision".to_owned(),
        provenance_reference: format!("decision_{commit_id}"),
        provenance_recorded_by: "rules_engine_atomic_commit".to_owned(),
        correlation_id: format!("correlation_{commit_id}"),
        causation_id: format!("causation_{commit_id}"),
        trace_id: format!("trace_{commit_id}"),
        events: event_types
            .iter()
            .enumerate()
            .map(|(index, event_type)| CanonicalEventDraft {
                event_type: (*event_type).to_owned(),
                payload_json: format!(r#"{{"index":{index},"commit":"{commit_id}"}}"#),
            })
            .collect(),
        audit: PolicyAuditDraft {
            actor_id: "keeper_atomic_commit".to_owned(),
            actor_origin: "user_session".to_owned(),
            authentication_reference: "session_atomic_commit".to_owned(),
            resource_type: "campaign".to_owned(),
            resource_id: "campaign_atomic_commit".to_owned(),
            action: "write_official_state".to_owned(),
            requested_role: "human_keeper".to_owned(),
            openfga_decision_id: format!("fga_{commit_id}"),
            openfga_policy_revision: "fga_model_atomic_commit".to_owned(),
            opa_decision_id: format!("opa_{commit_id}"),
            opa_policy_revision: "opa_bundle_atomic_commit".to_owned(),
        },
    }
}

fn bind_campaign(draft: &mut AtomicCommitDraft, campaign_id: &str) {
    draft.campaign_id = campaign_id.to_owned();
    draft.stream_id = campaign_id.to_owned();
    draft.audit.resource_id = campaign_id.to_owned();
    draft.authority_contract_id = format!("authority_{campaign_id}_1");
}

fn bind_stream(draft: &mut AtomicCommitDraft, stream_id: &str) {
    draft.stream_id = stream_id.to_owned();
    draft.audit.resource_type = "scene".to_owned();
    draft.audit.resource_id = stream_id.to_owned();
}

async fn scalar(pool: &PgPool, sql: &str) -> i64 {
    sqlx::query(sql).fetch_one(pool).await.unwrap().get(0)
}

#[tokio::test(flavor = "multi_thread")]
async fn canonical_commit_is_atomic_recoverable_and_externally_witnessed() {
    let (primary_url, witness_url) = database_urls();

    let store =
        PostgresCanonicalStore::connect(&primary_url, &witness_url, "p02-canonical-test-key", KEY)
            .await
            .unwrap();
    let (first_startup, concurrent_startup) =
        tokio::join!(store.prepare_for_service(), store.prepare_for_service());
    first_startup.unwrap();
    concurrent_startup.unwrap();

    let primary = PgPool::connect(&primary_url).await.unwrap();
    let witness = PgPool::connect(&witness_url).await.unwrap();

    let mut success_draft = draft("success", 0, &["CampaignStarted", "InvestigatorJoined"]);
    success_draft.events[0].payload_json = "{ \"z\": 1, \"a\": [true, null] }".to_owned();
    let success = store.commit(&success_draft).await.unwrap();
    assert_eq!(success.first_stream_version, 1);
    assert_eq!(success.last_stream_version, 2);
    assert_eq!(
        scalar(&primary, "SELECT count(*) FROM event_store").await,
        2
    );
    assert_eq!(
        scalar(&primary, "SELECT count(*) FROM event_outbox").await,
        2
    );
    assert_eq!(
        scalar(&primary, "SELECT count(*) FROM canonical_audit_log").await,
        1
    );
    assert_eq!(
        scalar(&primary, "SELECT count(*) FROM formal_commits").await,
        1
    );
    assert_eq!(
        scalar(&witness, "SELECT count(*) FROM external_audit_witness").await,
        2
    );
    let formal_commit: FormalCommitRecord =
        sqlx::query_as("SELECT * FROM formal_commits WHERE commit_id = 'success'")
            .fetch_one(&primary)
            .await
            .expect("lossless SQLx formal-commit mapping");
    let formal_commit_round_trip: FormalCommitRecord =
        serde_json::from_value(serde_json::to_value(&formal_commit).unwrap()).unwrap();
    assert_eq!(formal_commit_round_trip, formal_commit);
    assert_eq!(formal_commit.status, "committed");
    assert_eq!(
        formal_commit.result_event_sequence,
        success.last_event_sequence
    );
    let signed_payload: String =
        sqlx::query_scalar("SELECT payload_integrity_source FROM event_store WHERE sequence = $1")
            .bind(success.first_event_sequence)
            .fetch_one(&primary)
            .await
            .unwrap();
    assert_ne!(signed_payload, success_draft.events[0].payload_json);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&signed_payload).unwrap(),
        serde_json::json!({"a": [true, null], "z": 1})
    );

    // A database-side rejection after the first event proves that events,
    // outbox rows, the audit record, and the formal-commit marker roll back as
    // one primary transaction. The independent PREPARED witness is reconciled
    // to ABORTED rather than being silently erased.
    sqlx::raw_sql(
        r#"
        CREATE OR REPLACE FUNCTION reject_atomicity_probe()
        RETURNS trigger LANGUAGE plpgsql AS $$
        BEGIN
            IF NEW.event_type = 'RejectForAtomicityProbe' THEN
                RAISE EXCEPTION 'atomicity probe rejection';
            END IF;
            RETURN NEW;
        END;
        $$;
        DROP TRIGGER IF EXISTS reject_atomicity_probe ON event_store;
        CREATE TRIGGER reject_atomicity_probe
        BEFORE INSERT ON event_store
        FOR EACH ROW EXECUTE FUNCTION reject_atomicity_probe();
        "#,
    )
    .execute(&primary)
    .await
    .unwrap();

    let failed = store
        .commit(&draft(
            "rollback",
            2,
            &["ClueDiscovered", "RejectForAtomicityProbe"],
        ))
        .await;
    assert!(matches!(
        failed,
        Err(CanonicalStoreError::PrimaryWrite { .. })
    ));

    sqlx::raw_sql(
        r#"
        DROP TRIGGER IF EXISTS reject_atomicity_probe ON event_store;
        DROP FUNCTION IF EXISTS reject_atomicity_probe();
        "#,
    )
    .execute(&primary)
    .await
    .unwrap();

    assert_eq!(
        scalar(&primary, "SELECT count(*) FROM event_store").await,
        2
    );
    assert_eq!(
        scalar(&primary, "SELECT count(*) FROM event_outbox").await,
        2
    );
    assert_eq!(
        scalar(&primary, "SELECT count(*) FROM canonical_audit_log").await,
        1
    );
    assert_eq!(
        scalar(&primary, "SELECT count(*) FROM formal_commits").await,
        1
    );

    let recovered = store.recover().await.unwrap();
    assert_eq!(
        recovered,
        RecoveryReport {
            finalized: 0,
            aborted: 1,
        }
    );
    assert_eq!(
        scalar(
            &witness,
            "SELECT count(*) FROM external_audit_witness WHERE commit_id = 'rollback' AND phase = 'ABORTED'"
        )
        .await,
        1
    );

    // Make only the witness finalization fail. The primary commit remains
    // durable; retrying the same idempotent request repairs the witness without
    // duplicating any event or audit row.
    sqlx::raw_sql(
        r#"
        CREATE OR REPLACE FUNCTION reject_witness_finalize_probe()
        RETURNS trigger LANGUAGE plpgsql AS $$
        BEGIN
            IF NEW.phase = 'COMMITTED' THEN
                RAISE EXCEPTION 'witness finalize probe rejection';
            END IF;
            RETURN NEW;
        END;
        $$;
        DROP TRIGGER IF EXISTS reject_witness_finalize_probe ON external_audit_witness;
        CREATE TRIGGER reject_witness_finalize_probe
        BEFORE INSERT ON external_audit_witness
        FOR EACH ROW EXECUTE FUNCTION reject_witness_finalize_probe();
        "#,
    )
    .execute(&witness)
    .await
    .unwrap();

    let pending_draft = draft("finalize_gap", 2, &["SceneAdvanced"]);
    let pending = store.commit(&pending_draft).await;
    assert!(matches!(
        pending,
        Err(CanonicalStoreError::WitnessFinalizationPending { .. })
    ));
    assert_eq!(
        scalar(&primary, "SELECT count(*) FROM event_store").await,
        3
    );

    sqlx::raw_sql(
        r#"
        DROP TRIGGER IF EXISTS reject_witness_finalize_probe ON external_audit_witness;
        DROP FUNCTION IF EXISTS reject_witness_finalize_probe();
        "#,
    )
    .execute(&witness)
    .await
    .unwrap();

    let retry = store.commit(&pending_draft).await.unwrap();
    assert_eq!(retry.first_event_sequence, retry.last_event_sequence);
    assert_eq!(
        scalar(&primary, "SELECT count(*) FROM event_store").await,
        3
    );
    assert_eq!(
        scalar(&primary, "SELECT count(*) FROM canonical_audit_log").await,
        2
    );
    assert_eq!(
        scalar(&primary, "SELECT count(*) FROM formal_commits").await,
        2
    );

    // The witness tables are append-only, including TRUNCATE protection.
    assert!(
        sqlx::query("DELETE FROM external_audit_witness WHERE commit_id = 'success'")
            .execute(&witness)
            .await
            .is_err()
    );
    assert!(sqlx::query("TRUNCATE external_audit_witness")
        .execute(&witness)
        .await
        .is_err());

    let mut mismatched_audit_scope = draft("mismatched_audit_scope", 0, &["Rejected"]);
    mismatched_audit_scope.campaign_id = "campaign_other".to_owned();
    assert!(matches!(
        store.commit(&mismatched_audit_scope).await,
        Err(CanonicalStoreError::Validation(
            "audit_campaign_resource_mismatch"
        ))
    ));

    // Idempotency and expected version are bound to campaign/stream/operation.
    // Two authorized resources in one campaign are independent streams even
    // when they reuse the same client idempotency key.
    let mut same_campaign_a = draft("same_campaign_a", 0, &["SceneAdvanced"]);
    bind_campaign(&mut same_campaign_a, "campaign_multi_stream");
    bind_stream(&mut same_campaign_a, "scene_alpha");
    same_campaign_a.idempotency_key = "shared_same_campaign_key".to_owned();
    let mut same_campaign_b = draft("same_campaign_b", 0, &["SceneAdvanced"]);
    bind_campaign(&mut same_campaign_b, "campaign_multi_stream");
    bind_stream(&mut same_campaign_b, "scene_beta");
    same_campaign_b.idempotency_key = "shared_same_campaign_key".to_owned();
    let same_campaign_a_result = store.commit(&same_campaign_a).await.unwrap();
    let same_campaign_b_result = store.commit(&same_campaign_b).await.unwrap();
    assert_eq!(same_campaign_a_result.first_stream_version, 1);
    assert_eq!(same_campaign_b_result.first_stream_version, 1);
    let stored_streams: Vec<(String, i64)> = sqlx::query_as(
        "SELECT stream_id, stream_version FROM event_store WHERE campaign_id = $1 ORDER BY stream_id",
    )
    .bind("campaign_multi_stream")
    .fetch_all(&primary)
    .await
    .unwrap();
    assert_eq!(
        stored_streams,
        vec![("scene_alpha".to_owned(), 1), ("scene_beta".to_owned(), 1)]
    );

    let mut mismatched_stream_scope = draft("mismatched_stream_scope", 0, &["Rejected"]);
    bind_campaign(&mut mismatched_stream_scope, "campaign_multi_stream");
    mismatched_stream_scope.stream_id = "scene_ungranted".to_owned();
    mismatched_stream_scope.audit.resource_type = "scene".to_owned();
    mismatched_stream_scope.audit.resource_id = "scene_granted".to_owned();
    assert!(matches!(
        store.commit(&mismatched_stream_scope).await,
        Err(CanonicalStoreError::Validation(
            "stream_audit_resource_mismatch"
        ))
    ));

    // Reusing a key in a different campaign is also valid; changing the
    // request inside one exact scope is rejected without another append.
    let mut scope_a = draft("scope_a", 0, &["CampaignScopedEvent"]);
    bind_campaign(&mut scope_a, "campaign_scope_a");
    scope_a.idempotency_key = "shared_scoped_key".to_owned();
    let mut scope_b = draft("scope_b", 0, &["CampaignScopedEvent"]);
    bind_campaign(&mut scope_b, "campaign_scope_b");
    scope_b.idempotency_key = "shared_scoped_key".to_owned();
    store.commit(&scope_a).await.unwrap();
    store.commit(&scope_b).await.unwrap();

    let mut conflicting = draft("scope_a_conflict", 1, &["DifferentRequest"]);
    bind_campaign(&mut conflicting, "campaign_scope_a");
    conflicting.idempotency_key = "shared_scoped_key".to_owned();
    assert!(matches!(
        store.commit(&conflicting).await,
        Err(CanonicalStoreError::IdempotencyConflict)
    ));
    assert_eq!(
        scalar(
            &primary,
            "SELECT count(*) FROM event_store WHERE campaign_id IN ('campaign_scope_a', 'campaign_scope_b')"
        )
        .await,
        2
    );

    // Global event sequences may interleave across independently locked
    // campaign streams. Hold two advisory barriers until both transactions
    // have allocated their first sequence, then release them together. This
    // deterministically overlaps the ranges without timing-based pg_sleep.
    let mut barrier = PgConnection::connect(&primary_url).await.unwrap();
    sqlx::query("SELECT pg_advisory_lock(90316001), pg_advisory_lock(90316002)")
        .execute(&mut barrier)
        .await
        .unwrap();
    let sequence_before: i64 =
        sqlx::query_scalar("SELECT last_value FROM event_store_sequence_seq")
            .fetch_one(&primary)
            .await
            .unwrap();
    sqlx::raw_sql(
        r#"
        CREATE OR REPLACE FUNCTION block_cross_campaign_probe()
        RETURNS trigger LANGUAGE plpgsql AS $$
        BEGIN
            IF NEW.event_type = 'ConcurrentAFirst' THEN
                PERFORM pg_advisory_xact_lock(90316001);
            ELSIF NEW.event_type = 'ConcurrentBFirst' THEN
                PERFORM pg_advisory_xact_lock(90316002);
            END IF;
            RETURN NEW;
        END;
        $$;
        DROP TRIGGER IF EXISTS block_cross_campaign_probe ON event_store;
        CREATE TRIGGER block_cross_campaign_probe
        BEFORE INSERT ON event_store
        FOR EACH ROW EXECUTE FUNCTION block_cross_campaign_probe();
        "#,
    )
    .execute(&primary)
    .await
    .unwrap();

    let mut concurrent_a = draft(
        "concurrent_a",
        0,
        &["ConcurrentAFirst", "ConcurrentASecond"],
    );
    bind_campaign(&mut concurrent_a, "campaign_concurrent_a");
    let mut concurrent_b = draft(
        "concurrent_b",
        0,
        &["ConcurrentBFirst", "ConcurrentBSecond"],
    );
    bind_campaign(&mut concurrent_b, "campaign_concurrent_b");
    let store_a = store.clone();
    let store_b = store.clone();
    let concurrent_a_task = tokio::spawn(async move { store_a.commit(&concurrent_a).await });
    let concurrent_b_task = tokio::spawn(async move { store_b.commit(&concurrent_b).await });

    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let current: i64 =
                sqlx::query_scalar("SELECT last_value FROM event_store_sequence_seq")
                    .fetch_one(&primary)
                    .await
                    .unwrap();
            if current >= sequence_before + 2 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("both concurrent commits reached their deterministic barrier");
    let unlocked: (bool, bool) =
        sqlx::query_as("SELECT pg_advisory_unlock(90316001), pg_advisory_unlock(90316002)")
            .fetch_one(&mut barrier)
            .await
            .unwrap();
    assert_eq!(unlocked, (true, true));
    let concurrent_a_result = concurrent_a_task.await.unwrap();
    let concurrent_b_result = concurrent_b_task.await.unwrap();

    sqlx::raw_sql(
        r#"
        DROP TRIGGER IF EXISTS block_cross_campaign_probe ON event_store;
        DROP FUNCTION IF EXISTS block_cross_campaign_probe();
        "#,
    )
    .execute(&primary)
    .await
    .unwrap();
    concurrent_a_result.unwrap();
    concurrent_b_result.unwrap();

    let concurrent_a_sequences: Vec<i64> = sqlx::query_scalar(
        "SELECT event_sequence FROM event_outbox WHERE commit_id = 'concurrent_a' ORDER BY event_sequence",
    )
    .fetch_all(&primary)
    .await
    .unwrap();
    let concurrent_b_sequences: Vec<i64> = sqlx::query_scalar(
        "SELECT event_sequence FROM event_outbox WHERE commit_id = 'concurrent_b' ORDER BY event_sequence",
    )
    .fetch_all(&primary)
    .await
    .unwrap();
    assert_eq!(concurrent_a_sequences.len(), 2);
    assert_eq!(concurrent_b_sequences.len(), 2);
    assert!(
        concurrent_a_sequences[0] < concurrent_b_sequences[1]
            && concurrent_b_sequences[0] < concurrent_a_sequences[1],
        "probe must produce overlapping global sequence ranges"
    );

    store.verify_integrity().await.unwrap();
    let audit_integrity_versions: Vec<i32> = sqlx::query_scalar(
        "SELECT DISTINCT integrity_version FROM canonical_audit_log ORDER BY integrity_version",
    )
    .fetch_all(&primary)
    .await
    .unwrap();
    assert_eq!(audit_integrity_versions, vec![2]);

    // Simulate a privileged restore that bypasses ordinary triggers. Version 2
    // binds occurred_at into the HMAC, so timestamp-only tampering is detected
    // even when the database append-only guard is deliberately bypassed.
    let mut audit_tamper = primary.begin().await.unwrap();
    sqlx::query("SET LOCAL session_replication_role = replica")
        .execute(&mut *audit_tamper)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE canonical_audit_log SET occurred_at = occurred_at + interval '1 microsecond' WHERE sequence = (SELECT min(sequence) FROM canonical_audit_log)",
    )
    .execute(&mut *audit_tamper)
    .await
    .unwrap();
    audit_tamper.commit().await.unwrap();
    assert_eq!(
        store.verify_integrity().await,
        Err(CanonicalStoreError::IntegrityViolation(
            "canonical_audit_hmac_mismatch"
        ))
    );
}

#[tokio::test]
async fn canonical_and_witness_endpoints_must_be_distinct() {
    let (primary_url, _) = database_urls();
    let result =
        PostgresCanonicalStore::connect(&primary_url, &primary_url, "p02-canonical-test-key", KEY)
            .await;
    assert!(matches!(
        result,
        Err(CanonicalStoreError::Configuration(
            "independent_witness_endpoint_required"
        ))
    ));
}
