
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
