use std::env;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use sqlx::postgres::PgConnectOptions;
use sqlx::{Connection, PgConnection, PgPool, Row};
use trpg_data_eventing::event_store_sqlx_outbox_projection::{
    AtomicCommitDraft, CanonicalEventDraft, CanonicalStoreError, PolicyAuditDraft,
    PostgresCanonicalCommitPort, PostgresCanonicalStore, RecoveryReport,
};
use trpg_data_eventing::persistence::FormalCommitRecord;
use trpg_domain_core::ddd::FactSource;
use trpg_shared_kernel::{
    CanonicalCommitKey, CanonicalCommitPort, EventActorOriginWire, TrpgError,
};

const KEY: &[u8; 32] = &[0x9c; 32];
const PAYLOAD_KEY: &[u8; 32] = &[0xad; 32];

fn database_urls() -> (String, String) {
    let primary = env::var("P02_CANONICAL_DATABASE_URL")
        .expect("P02_CANONICAL_DATABASE_URL is required for the real PostgreSQL gate");
    let witness = env::var("P02_CANONICAL_WITNESS_DATABASE_URL")
        .expect("P02_CANONICAL_WITNESS_DATABASE_URL is required for the real PostgreSQL gate");
    (primary, witness)
}

#[derive(Debug, PartialEq, Eq)]
struct DatabaseIdentity {
    host: String,
    port: u16,
    database: String,
}

fn database_identity(options: &PgConnectOptions) -> DatabaseIdentity {
    let database = options
        .get_database()
        .filter(|database| !database.trim().is_empty())
        .expect("canonical PostgreSQL URL must name an explicit non-empty database");
    DatabaseIdentity {
        host: options.get_host().to_owned(),
        port: options.get_port(),
        database: database.to_owned(),
    }
}

fn assert_distinct_database_targets(primary_url: &str, witness_url: &str) {
    let primary_identity = database_identity(
        &PgConnectOptions::from_str(primary_url).expect("valid primary canonical PostgreSQL URL"),
    );
    let witness_identity = database_identity(
        &PgConnectOptions::from_str(witness_url).expect("valid witness canonical PostgreSQL URL"),
    );
    assert_ne!(
        primary_identity, witness_identity,
        "canonical primary and witness reset targets must be distinct"
    );
}

async fn reset_dedicated_database(database_url: &str, authorized_database_variable: &str) {
    assert_eq!(
        env::var("P02_CANONICAL_ALLOW_DATABASE_RESET").as_deref(),
        Ok("1"),
        "set P02_CANONICAL_ALLOW_DATABASE_RESET=1 for the dedicated canonical integration databases"
    );
    let options = PgConnectOptions::from_str(database_url).expect("valid canonical PostgreSQL URL");
    let identity = database_identity(&options);
    let authorized_database = env::var(authorized_database_variable).unwrap_or_else(|_| {
        panic!("{authorized_database_variable} must name the dedicated canonical database")
    });
    assert!(
        !authorized_database.trim().is_empty(),
        "{authorized_database_variable} must name a non-empty dedicated canonical database"
    );
    assert!(
        matches!(identity.host.as_str(), "localhost" | "127.0.0.1" | "::1")
            && identity.database == authorized_database,
        "canonical integration test refuses to reset a non-dedicated local database"
    );
    let pool = PgPool::connect_with(options)
        .await
        .expect("connect to dedicated canonical integration database");
    sqlx::raw_sql(
        "DROP SCHEMA IF EXISTS core_domain CASCADE; \
         DROP SCHEMA public CASCADE; \
         CREATE SCHEMA public; \
         GRANT ALL ON SCHEMA public TO public;",
    )
    .execute(&pool)
    .await
    .expect("reset dedicated canonical integration database");
    pool.close().await;
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
        authenticated_actor_role: "workflow".to_owned(),
        authenticated_actor_origin: EventActorOriginWire::Workload {
            role: "workflow_engine".to_owned(),
        },
        authority_mode: "human_kp".to_owned(),
        authority_contract_version: 1,
        authority_contract_id: "authority_campaign_atomic_commit_1".to_owned(),
        authority_owner: "keeper_atomic_commit".to_owned(),
        visibility_label: "party_visible".to_owned(),
        visibility_subject: "not_applicable".to_owned(),
        data_subject_id: "not_applicable".to_owned(),
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
                projection_targets: Vec::new(),
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
    assert_distinct_database_targets(&primary_url, &witness_url);
    reset_dedicated_database(&primary_url, "P02_CANONICAL_RESET_DATABASE").await;
    reset_dedicated_database(&witness_url, "P02_CANONICAL_WITNESS_RESET_DATABASE").await;

    let store = PostgresCanonicalStore::connect(
        &primary_url,
        &witness_url,
        "p02-canonical-test-key",
        KEY,
        "p05-canonical-payload-key",
        PAYLOAD_KEY,
    )
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

    // The synchronous production port resolves committed custody before a
    // caller repeats any non-idempotent external work. A miss also preflights
    // the stream version so stale commands fail before their executor runs.
    let retained_runtime = Arc::new(Mutex::new(
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap(),
    ));
    let canonical_port =
        PostgresCanonicalCommitPort::new(Arc::clone(&retained_runtime), store.clone());
    let exact_receipt = canonical_port
        .load_receipt(&CanonicalCommitKey {
            commit_id: success_draft.commit_id.clone(),
            campaign_id: success_draft.campaign_id.clone(),
            stream_id: success_draft.stream_id.clone(),
            idempotency_key: success_draft.idempotency_key.clone(),
            expected_version: 0,
        })
        .unwrap()
        .expect("committed canonical receipt must be reusable");
    assert_eq!(exact_receipt.first_stream_version, 1);
    assert_eq!(exact_receipt.last_stream_version, 2);
    assert_eq!(exact_receipt.events.len(), 2);
    assert!(canonical_port
        .load_receipt(&CanonicalCommitKey {
            commit_id: "future_commit".to_owned(),
            campaign_id: success_draft.campaign_id.clone(),
            stream_id: success_draft.stream_id.clone(),
            idempotency_key: "future_idempotency".to_owned(),
            expected_version: 2,
        })
        .unwrap()
        .is_none());
    assert_eq!(
        canonical_port.load_receipt(&CanonicalCommitKey {
            commit_id: "stale_commit".to_owned(),
            campaign_id: success_draft.campaign_id.clone(),
            stream_id: success_draft.stream_id.clone(),
            idempotency_key: "stale_idempotency".to_owned(),
            expected_version: 1,
        }),
        Err(TrpgError::ExpectedVersionConflict {
            expected: 1,
            actual: 2,
        })
    );
    drop(canonical_port);
    std::thread::spawn(move || drop(retained_runtime))
        .join()
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
    let audit_context: (String, String) = sqlx::query_as(
        "SELECT correlation_id, causation_id FROM canonical_audit_log WHERE commit_id = 'success'",
    )
    .fetch_one(&primary)
    .await
    .unwrap();
    assert_eq!(
        audit_context,
        (
            success_draft.correlation_id.clone(),
            success_draft.causation_id.clone()
        )
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
    let protected_envelope = serde_json::from_str::<serde_json::Value>(&signed_payload).unwrap();
    assert_eq!(
        protected_envelope["protected_payload"]["algorithm"],
        "AES-256-GCM"
    );
    assert_eq!(
        protected_envelope["protected_payload"]["key_reference"],
        "p05-canonical-payload-key"
    );
    assert!(!signed_payload.contains("\"a\":[true"));
    assert!(!signed_payload.contains("\"z\":1"));
    let stored_payload: String =
        sqlx::query_scalar("SELECT payload_json::text FROM event_store WHERE sequence = $1")
            .bind(success.first_event_sequence)
            .fetch_one(&primary)
            .await
            .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&stored_payload).unwrap(),
        protected_envelope
    );
    let outbox_payload: String =
        sqlx::query_scalar("SELECT payload_json::text FROM event_outbox WHERE event_sequence = $1")
            .bind(success.first_event_sequence)
            .fetch_one(&primary)
            .await
            .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&outbox_payload).unwrap(),
        serde_json::from_str::<serde_json::Value>(&stored_payload).unwrap()
    );
    assert!(!outbox_payload.contains("\"a\":[true"));
    let encrypted_columns = sqlx::query(
        "SELECT payload_ciphertext, payload_key_reference, payload_nonce \
         FROM event_store WHERE sequence = $1",
    )
    .bind(success.first_event_sequence)
    .fetch_one(&primary)
    .await
    .unwrap();
    let event_ciphertext: Vec<u8> = encrypted_columns.get("payload_ciphertext");
    let event_key_reference: String = encrypted_columns.get("payload_key_reference");
    let event_nonce: Vec<u8> = encrypted_columns.get("payload_nonce");
    assert!(event_ciphertext.len() >= 16);
    assert_eq!(event_key_reference, "p05-canonical-payload-key");
    assert_eq!(event_nonce.len(), 12);
    let outbox_encrypted_columns = sqlx::query(
        "SELECT payload_ciphertext, payload_key_reference, payload_nonce, visibility_subject \
         FROM event_outbox WHERE event_sequence = $1",
    )
    .bind(success.first_event_sequence)
    .fetch_one(&primary)
    .await
    .unwrap();
    assert_eq!(
        outbox_encrypted_columns.get::<Vec<u8>, _>("payload_ciphertext"),
        event_ciphertext
    );
    assert_eq!(
        outbox_encrypted_columns.get::<String, _>("payload_key_reference"),
        event_key_reference
    );
    assert_eq!(
        outbox_encrypted_columns.get::<Vec<u8>, _>("payload_nonce"),
        event_nonce
    );
    assert_eq!(
        outbox_encrypted_columns.get::<String, _>("visibility_subject"),
        success_draft.visibility_subject
    );
    let replay = store
        .load_replay_page("campaign_atomic_commit", 0, 10)
        .await
        .unwrap();
    assert_eq!(replay.len(), 2);
    assert_eq!(
        replay[0].payload,
        serde_json::json!({"a": [true, null], "z": 1})
    );

    // Fact promotion consumes the real encrypted, HMAC-verified canonical
    // event and external witness chain. It cannot substitute the process-local
    // EventStore fixture used by domain-only tests.
    let mut fact_draft = draft("persisted_fact_evidence", 0, &["DecisionCommitted"]);
    bind_campaign(&mut fact_draft, "campaign_persisted_fact_evidence");
    fact_draft.events[0].payload_json = serde_json::json!({
        "kind": "RecordDecision",
        "fact_source": "DecisionRecord",
        "target_fact_id": "persisted_fact_001"
    })
    .to_string();
    let fact_commit = store.commit(&fact_draft).await.unwrap();
    let evidence = store
        .load_committed_fact_evidence(
            "campaign_persisted_fact_evidence",
            fact_commit.first_event_sequence,
            "persisted_fact_001",
        )
        .await
        .unwrap();
    assert_eq!(
        evidence.event_sequence(),
        fact_commit.first_event_sequence as u64
    );
    assert_eq!(evidence.source(), FactSource::DecisionRecord);
    assert_eq!(evidence.target_fact_id().as_str(), "persisted_fact_001");
    assert_eq!(
        evidence.stream_id().as_str(),
        "campaign_persisted_fact_evidence"
    );

    // A database-side rejection after the first event proves that events,
    // outbox rows, the audit record, and the formal-commit marker roll back as
    // one primary transaction. The independent PREPARED witness is reconciled
    // to ABORTED rather than being silently erased.
    let event_count_before_rollback = scalar(&primary, "SELECT count(*) FROM event_store").await;
    let outbox_count_before_rollback = scalar(&primary, "SELECT count(*) FROM event_outbox").await;
    let audit_count_before_rollback =
        scalar(&primary, "SELECT count(*) FROM canonical_audit_log").await;
    let formal_count_before_rollback =
        scalar(&primary, "SELECT count(*) FROM formal_commits").await;
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
        event_count_before_rollback
    );
    assert_eq!(
        scalar(&primary, "SELECT count(*) FROM event_outbox").await,
        outbox_count_before_rollback
    );
    assert_eq!(
        scalar(&primary, "SELECT count(*) FROM canonical_audit_log").await,
        audit_count_before_rollback
    );
    assert_eq!(
        scalar(&primary, "SELECT count(*) FROM formal_commits").await,
        formal_count_before_rollback
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
        event_count_before_rollback + 1
    );
    assert_eq!(
        scalar(&primary, "SELECT count(*) FROM event_outbox").await,
        outbox_count_before_rollback + 1
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
        event_count_before_rollback + 1
    );
    assert_eq!(
        scalar(&primary, "SELECT count(*) FROM event_outbox").await,
        outbox_count_before_rollback + 1
    );
    assert_eq!(
        scalar(&primary, "SELECT count(*) FROM canonical_audit_log").await,
        audit_count_before_rollback + 1
    );
    assert_eq!(
        scalar(&primary, "SELECT count(*) FROM formal_commits").await,
        formal_count_before_rollback + 1
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

    // A stale write deliberately leaves an unresolved PREPARED witness. A
    // process carrying the wrong HMAC key must fail before appending an
    // ABORTED recovery record; otherwise one bad deployment permanently
    // poisons the append-only external witness.
    let mut wrong_key_probe = draft("wrong_key_recovery_probe", 1, &["MustNotCommit"]);
    bind_campaign(&mut wrong_key_probe, "campaign_wrong_key_recovery_probe");
    assert!(matches!(
        store.commit(&wrong_key_probe).await,
        Err(CanonicalStoreError::VersionConflict {
            expected: 1,
            actual: 0
        })
    ));
    let witness_rows_before_wrong_key =
        scalar(&witness, "SELECT count(*) FROM external_audit_witness").await;
    let wrong_key_store = PostgresCanonicalStore::connect(
        &primary_url,
        &witness_url,
        "wrong-canonical-integrity-key",
        &[0xee; 32],
        "p05-canonical-payload-key",
        PAYLOAD_KEY,
    )
    .await
    .unwrap();
    assert_eq!(
        wrong_key_store.prepare_for_service().await,
        Err(CanonicalStoreError::IntegrityViolation(
            "external_witness_hmac_mismatch"
        ))
    );
    assert_eq!(
        scalar(&witness, "SELECT count(*) FROM external_audit_witness").await,
        witness_rows_before_wrong_key,
        "wrong-key recovery must not mutate the append-only witness"
    );
    assert_eq!(
        store.recover().await.unwrap(),
        RecoveryReport {
            finalized: 0,
            aborted: 1,
        }
    );

    store.verify_integrity().await.unwrap();
    let audit_integrity_versions: Vec<i32> = sqlx::query_scalar(
        "SELECT DISTINCT integrity_version FROM canonical_audit_log ORDER BY integrity_version",
    )
    .fetch_all(&primary)
    .await
    .unwrap();
    assert_eq!(audit_integrity_versions, vec![3]);

    // Simulate a privileged restore that bypasses ordinary triggers. Version 3
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
    let result = PostgresCanonicalStore::connect(
        &primary_url,
        &primary_url,
        "p02-canonical-test-key",
        KEY,
        "p05-canonical-payload-key",
        PAYLOAD_KEY,
    )
    .await;
    assert!(matches!(
        result,
        Err(CanonicalStoreError::Configuration(
            "independent_witness_endpoint_required"
        ))
    ));
}

#[test]
#[should_panic(expected = "canonical primary and witness reset targets must be distinct")]
fn canonical_reset_rejects_identical_targets_before_connecting() {
    let database_url = "postgresql://local@127.0.0.1:25432/canonical_reset_probe";
    assert_distinct_database_targets(database_url, database_url);
}

#[test]
#[should_panic(expected = "canonical PostgreSQL URL must name an explicit non-empty database")]
fn canonical_reset_rejects_a_missing_database_name() {
    database_identity(&PgConnectOptions::new().host("127.0.0.1").port(25432));
}
