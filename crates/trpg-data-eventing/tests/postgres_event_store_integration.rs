mod support;

use std::path::{Path, PathBuf};
use std::process::Command;

use sqlx::{PgPool, Row};
use trpg_data_eventing::event_store_sqlx_outbox_projection::{
    CanonicalStoreError, PostgresCanonicalStore,
};
use trpg_data_eventing::outbox_projection_workers::PostgresProjectionWorker;

use support::{
    assert_database_name, connect_pool, draft, P04PostgresHarness, INTEGRITY_KEY, PAYLOAD_KEY,
};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn postgres_streams_are_isolated_idempotent_atomic_and_restartable() {
    let harness = P04PostgresHarness::reset().await;

    // Same Campaign/Stream and expected version: the advisory transaction
    // lock serializes both writers and exactly one can own stream version 1.
    let first = draft(
        "campaign_concurrent",
        "scene_concurrent",
        "concurrent_first",
        0,
        &["SceneAdvanced"],
    );
    let second = draft(
        "campaign_concurrent",
        "scene_concurrent",
        "concurrent_second",
        0,
        &["SceneAdvanced"],
    );
    let (first_result, second_result) =
        tokio::join!(harness.store.commit(&first), harness.store.commit(&second));
    let conflict = match (first_result, second_result) {
        (Ok(_), Err(error)) | (Err(error), Ok(_)) => error,
        (Ok(_), Ok(_)) => panic!("both same-version writers committed"),
        (Err(first_error), Err(second_error)) => {
            panic!("both same-version writers failed: {first_error}; {second_error}")
        }
    };
    assert!(matches!(
        conflict,
        CanonicalStoreError::VersionConflict {
            expected: 0,
            actual: 1
        }
    ));
    assert_eq!(
        count_for_scope(&harness.primary, "event_store", "campaign_concurrent").await,
        1
    );

    // Equal stream versions in independent streams do not conflict.
    let stream_a = draft(
        "campaign_isolation",
        "scene_isolation_a",
        "isolation_a",
        0,
        &["SceneOpened"],
    );
    let stream_b = draft(
        "campaign_isolation",
        "scene_isolation_b",
        "isolation_b",
        0,
        &["SceneOpened"],
    );
    let (stream_a_result, stream_b_result) = tokio::join!(
        harness.store.commit(&stream_a),
        harness.store.commit(&stream_b)
    );
    assert_eq!(stream_a_result.unwrap().first_stream_version, 1);
    assert_eq!(stream_b_result.unwrap().first_stream_version, 1);

    // Idempotency is resolved before optimistic concurrency. After another
    // commit advances the stream, retrying the byte-equivalent original
    // request returns its first durable result rather than a version error.
    let original = draft(
        "campaign_idempotency",
        "scene_idempotency",
        "idempotent_original",
        0,
        &["ClueRecorded"],
    );
    let original_receipt = harness.store.commit(&original).await.unwrap();
    harness
        .store
        .commit(&draft(
            "campaign_idempotency",
            "scene_idempotency",
            "idempotent_advance",
            1,
            &["ClockAdvanced"],
        ))
        .await
        .unwrap();
    let retry_receipt = harness.store.commit(&original).await.unwrap();
    assert_eq!(retry_receipt, original_receipt);
    assert_eq!(
        count_for_scope(&harness.primary, "event_store", "campaign_idempotency").await,
        2
    );
    let mut conflicting_retry = original.clone();
    conflicting_retry.events[0].payload_json = r#"{"changed":true}"#.to_owned();
    assert_eq!(
        harness.store.commit(&conflicting_retry).await.unwrap_err(),
        CanonicalStoreError::IdempotencyConflict
    );

    // A database failure on the second event proves the formal approval and
    // decision, their outbox rows, audit, and commit marker share one SQLx
    // transaction. Retrying after removing the fault yields one decision.
    sqlx::raw_sql(
        r#"
        CREATE OR REPLACE FUNCTION reject_decision_midpoint_probe()
        RETURNS trigger LANGUAGE plpgsql AS $$
        BEGIN
            IF NEW.event_type = 'DecisionCommitted' THEN
                RAISE EXCEPTION 'decision midpoint probe rejection';
            END IF;
            RETURN NEW;
        END;
        $$;
        CREATE TRIGGER reject_decision_midpoint_probe
        BEFORE INSERT ON event_store
        FOR EACH ROW EXECUTE FUNCTION reject_decision_midpoint_probe();
        "#,
    )
    .execute(&harness.primary)
    .await
    .unwrap();
    let decision = draft(
        "campaign_atomic_decision",
        "scene_atomic_decision",
        "atomic_decision",
        0,
        &["ToolRequestApproved", "DecisionCommitted"],
    );
    assert!(matches!(
        harness.store.commit(&decision).await,
        Err(CanonicalStoreError::PrimaryWrite { .. })
    ));
    for table in [
        "event_store",
        "event_outbox",
        "formal_commits",
        "canonical_audit_log",
    ] {
        assert_eq!(
            count_for_scope(&harness.primary, table, "campaign_atomic_decision").await,
            0,
            "{table} retained a half-committed decision"
        );
    }
    sqlx::raw_sql(
        r#"
        DROP TRIGGER reject_decision_midpoint_probe ON event_store;
        DROP FUNCTION reject_decision_midpoint_probe();
        "#,
    )
    .execute(&harness.primary)
    .await
    .unwrap();
    let recovered_decision = harness.store.commit(&decision).await.unwrap();
    assert_eq!(recovered_decision.first_stream_version, 1);
    assert_eq!(recovered_decision.last_stream_version, 2);
    let event_types: Vec<String> = sqlx::query(
        "SELECT event_type FROM event_store WHERE campaign_id = $1 ORDER BY stream_version",
    )
    .bind("campaign_atomic_decision")
    .fetch_all(&harness.primary)
    .await
    .unwrap()
    .iter()
    .map(|row| row.get("event_type"))
    .collect();
    assert_eq!(
        event_types,
        vec!["ToolRequestApproved", "DecisionCommitted"]
    );

    // Bounded replay survives process/store reconstruction and does not use a
    // global event count as the stream cursor.
    for version in 0..5 {
        harness
            .store
            .commit(&draft(
                "campaign_restart_replay",
                "scene_restart_replay",
                &format!("restart_replay_{version}"),
                version,
                &["ReplayProbeRecorded"],
            ))
            .await
            .unwrap();
    }
    let first_page = harness
        .store
        .load_replay_page("campaign_restart_replay", 0, 2)
        .await
        .unwrap();
    assert_eq!(first_page.len(), 2);
    let restarted = PostgresCanonicalStore::connect(
        &std::env::var("P04_DATABASE_URL").unwrap(),
        &std::env::var("P04_WITNESS_DATABASE_URL").unwrap(),
        "p04-eventing-test-key",
        INTEGRITY_KEY,
        "p05-eventing-payload-key",
        PAYLOAD_KEY,
    )
    .await
    .unwrap();
    restarted.prepare_for_service().await.unwrap();
    let second_page = restarted
        .load_replay_page(
            "campaign_restart_replay",
            first_page.last().unwrap().sequence,
            2,
        )
        .await
        .unwrap();
    let third_page = restarted
        .load_replay_page(
            "campaign_restart_replay",
            second_page.last().unwrap().sequence,
            2,
        )
        .await
        .unwrap();
    assert_eq!(second_page.len(), 2);
    assert_eq!(third_page.len(), 1);
    let versions: Vec<i64> = first_page
        .iter()
        .chain(&second_page)
        .chain(&third_page)
        .map(|event| event.stream_version)
        .collect();
    assert_eq!(versions, vec![1, 2, 3, 4, 5]);

    backup_destroy_restore_preserves_projection_hash(
        &harness.primary,
        "campaign_restart_replay",
        "scene_restart_replay",
    )
    .await;

    // Leave no unresolved PREPARED witness from the losing concurrent write.
    harness.store.recover().await.unwrap();
}

async fn count_for_scope(pool: &PgPool, table: &str, campaign_id: &str) -> i64 {
    assert!(matches!(
        table,
        "event_store" | "event_outbox" | "formal_commits" | "canonical_audit_log"
    ));
    let sql = format!("SELECT count(*) FROM {table} WHERE campaign_id = $1");
    sqlx::query_scalar(&sql)
        .bind(campaign_id)
        .fetch_one(pool)
        .await
        .expect("count canonical rows by campaign")
}

async fn backup_destroy_restore_preserves_projection_hash(
    source: &PgPool,
    campaign_id: &str,
    stream_id: &str,
) {
    let source_url = std::env::var("P04_DATABASE_URL").unwrap();
    let admin_url = std::env::var("P04_ADMIN_DATABASE_URL")
        .expect("P04_ADMIN_DATABASE_URL must name the temporary server's postgres database");
    let recovery_url = std::env::var("P04_RECOVERY_DATABASE_URL")
        .expect("P04_RECOVERY_DATABASE_URL must name p04_eventing_recovery");
    assert_database_name(&admin_url, "postgres");
    assert_database_name(&recovery_url, "p04_eventing_recovery");
    let pg_dump = required_postgres_program("P04_PG_DUMP", "pg_dump");
    let pg_restore = required_postgres_program("P04_PG_RESTORE", "pg_restore");
    let dump_path = std::env::temp_dir().join(format!(
        "trpg-p04-eventing-recovery-{}.dump",
        std::process::id()
    ));

    // Materialize the real downstream table before backup. Computing a hash
    // directly from Event Store here would allow an empty Projection table and
    // checkpoint to masquerade as a successful recovery drill.
    let projection_name = "backup_recovery_projection";
    let source_worker = PostgresProjectionWorker::new(source.clone(), projection_name, 2).unwrap();
    let source_checkpoint = source_worker
        .rebuild_to_tip(campaign_id, stream_id)
        .await
        .unwrap();
    let (source_rows, stored_source_hash) =
        materialized_projection_state(source, projection_name, campaign_id, stream_id).await;
    assert_eq!(source_rows, 5);
    assert_eq!(stored_source_hash, source_checkpoint.projection_hash);

    let dump = Command::new(&pg_dump)
        .args(["--format=custom", "--no-owner", "--no-privileges", "--file"])
        .arg(&dump_path)
        .arg("--dbname")
        .arg(&source_url)
        .output()
        .expect("execute PostgreSQL backup");
    assert!(dump.status.success(), "pg_dump failed");
    assert!(dump_path.metadata().unwrap().len() > 0);

    let admin = connect_pool(&admin_url, 2).await;
    let first_hash = restore_and_hash(
        &admin,
        &recovery_url,
        &pg_restore,
        &dump_path,
        projection_name,
        campaign_id,
        stream_id,
    )
    .await;
    assert_eq!(first_hash, stored_source_hash);

    // Destroy the restored database, then reconstruct it a second time from
    // the same immutable backup. This is the recovery drill, not an
    // application rollback and never invokes a destructive down migration.
    let second_hash = restore_and_hash(
        &admin,
        &recovery_url,
        &pg_restore,
        &dump_path,
        projection_name,
        campaign_id,
        stream_id,
    )
    .await;
    assert_eq!(second_hash, stored_source_hash);
    admin.close().await;
    std::fs::remove_file(&dump_path).expect("remove the P04-owned temporary backup");
}

async fn restore_and_hash(
    admin: &PgPool,
    recovery_url: &str,
    pg_restore: &Path,
    dump_path: &Path,
    projection_name: &str,
    campaign_id: &str,
    stream_id: &str,
) -> String {
    sqlx::query("DROP DATABASE IF EXISTS p04_eventing_recovery WITH (FORCE)")
        .execute(admin)
        .await
        .expect("destroy only the dedicated P04 recovery database");
    sqlx::query("CREATE DATABASE p04_eventing_recovery")
        .execute(admin)
        .await
        .expect("recreate only the dedicated P04 recovery database");
    let restore = Command::new(pg_restore)
        .args([
            "--no-owner",
            "--no-privileges",
            "--exit-on-error",
            "--dbname",
        ])
        .arg(recovery_url)
        .arg(dump_path)
        .output()
        .expect("execute PostgreSQL restore");
    assert!(restore.status.success(), "pg_restore failed");
    let recovery = connect_pool(recovery_url, 5).await;
    let (restored_rows, restored_hash) =
        materialized_projection_state(&recovery, projection_name, campaign_id, stream_id).await;
    assert_eq!(
        restored_rows, 5,
        "backup omitted materialized projection rows"
    );

    // Prove that a restored stale checkpoint cannot hide a destroyed read
    // model: retain the checkpoint, delete only projection rows, and demand a
    // complete deterministic rebuild from the restored Event Store.
    sqlx::query(
        r#"
        DELETE FROM public.canonical_event_projection
         WHERE projection_name = $1 AND campaign_id = $2 AND stream_id = $3
        "#,
    )
    .bind(projection_name)
    .bind(campaign_id)
    .bind(stream_id)
    .execute(&recovery)
    .await
    .unwrap();
    let worker = PostgresProjectionWorker::new(recovery.clone(), projection_name, 2).unwrap();
    let rebuilt = worker.rebuild_to_tip(campaign_id, stream_id).await.unwrap();
    let (rebuilt_rows, rebuilt_hash) =
        materialized_projection_state(&recovery, projection_name, campaign_id, stream_id).await;
    assert_eq!(rebuilt_rows, 5);
    assert_eq!(rebuilt.projection_hash, restored_hash);
    assert_eq!(rebuilt_hash, restored_hash);
    recovery.close().await;
    rebuilt_hash
}

async fn materialized_projection_state(
    pool: &PgPool,
    projection_name: &str,
    campaign_id: &str,
    stream_id: &str,
) -> (i64, String) {
    let rows: i64 = sqlx::query_scalar(
        r#"
        SELECT count(*)
          FROM public.canonical_event_projection
         WHERE projection_name = $1 AND campaign_id = $2 AND stream_id = $3
        "#,
    )
    .bind(projection_name)
    .bind(campaign_id)
    .bind(stream_id)
    .fetch_one(pool)
    .await
    .expect("count materialized projection rows");
    let hash: String = sqlx::query_scalar(
        r#"
        SELECT projection_hash
          FROM public.projection_checkpoint
         WHERE projection_name = $1 AND campaign_id = $2 AND stream_id = $3
        "#,
    )
    .bind(projection_name)
    .bind(campaign_id)
    .bind(stream_id)
    .fetch_one(pool)
    .await
    .expect("load materialized projection checkpoint");
    (rows, hash)
}

fn required_postgres_program(variable: &str, expected_name: &str) -> PathBuf {
    let path = PathBuf::from(std::env::var(variable).unwrap_or_else(|_| {
        panic!("{variable} must name the PostgreSQL {expected_name} executable")
    }));
    assert_eq!(
        path.file_name().and_then(|name| name.to_str()),
        Some(expected_name),
        "unexpected PostgreSQL recovery executable"
    );
    assert!(path.is_file(), "PostgreSQL recovery executable is missing");
    path
}
