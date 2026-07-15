use std::env;

use sqlx::{PgPool, Row};
use trpg_data_eventing::event_store_sqlx_outbox_projection::{
    AtomicCommitDraft, CanonicalEventDraft, CanonicalStoreError, PolicyAuditDraft,
    PostgresCanonicalStore, RecoveryReport,
};

const KEY: &[u8; 32] = &[0x9c; 32];

fn database_urls() -> Option<(String, String)> {
    let primary = env::var("P02_CANONICAL_DATABASE_URL").ok()?;
    let witness = env::var("P02_CANONICAL_WITNESS_DATABASE_URL").ok()?;
    Some((primary, witness))
}

fn draft(commit_id: &str, expected_version: i64, event_types: &[&str]) -> AtomicCommitDraft {
    AtomicCommitDraft {
        commit_id: commit_id.to_owned(),
        campaign_id: "campaign_atomic_commit".to_owned(),
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

async fn scalar(pool: &PgPool, sql: &str) -> i64 {
    sqlx::query(sql).fetch_one(pool).await.unwrap().get(0)
}

#[tokio::test(flavor = "multi_thread")]
async fn canonical_commit_is_atomic_recoverable_and_externally_witnessed() {
    let Some((primary_url, witness_url)) = database_urls() else {
        eprintln!("skipped: set P02_CANONICAL_DATABASE_URL and P02_CANONICAL_WITNESS_DATABASE_URL for the real PostgreSQL gate");
        return;
    };

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

    let success = store
        .commit(&draft(
            "success",
            0,
            &["CampaignStarted", "InvestigatorJoined"],
        ))
        .await
        .unwrap();
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

    store.verify_integrity().await.unwrap();
}

#[tokio::test]
async fn canonical_and_witness_endpoints_must_be_distinct() {
    let Some((primary_url, _)) = database_urls() else {
        return;
    };
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
