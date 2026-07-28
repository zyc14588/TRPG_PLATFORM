
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exhausted_lease_recovery_remains_terminal_and_is_not_requeued() {
    let database_url = std::env::var("P05_DATABASE_URL")
        .expect("P05_DATABASE_URL must point to the P05 PostgreSQL test database");
    let witness_url = std::env::var("P05_WITNESS_DATABASE_URL")
        .expect("P05_WITNESS_DATABASE_URL must point to an independent P05 witness database");
    let store = PostgresCanonicalStore::connect(
        &database_url,
        &witness_url,
        INTEGRITY_KEY_ID,
        &INTEGRITY_KEY,
        PAYLOAD_KEY_ID,
        &PAYLOAD_KEY,
    )
    .await
    .expect("connect canonical store for exhausted-lease evidence");
    store.prepare_for_service().await.unwrap();
    let pool = PgPoolOptions::new()
        .max_connections(3)
        .connect(&database_url)
        .await
        .expect("connect P05 PostgreSQL");
    let repository = PostgresDeletionRepository::new(pool.clone());
    repository.migrate().await.unwrap();
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let job_id = format!("exhausted_lease_{nonce}");
    let subject_id = format!("exhausted_subject_{nonce}");
    let (event_sequence, event_hash) =
        commit_deletion_request(&store, repository.pool(), nonce, &subject_id, &job_id).await;
    repository
        .record_confirmed(
            &job_id,
            &subject_id,
            "privacy_officer",
            "user_erasure_v1",
            &deletion_evidence(nonce),
            event_sequence,
            &event_hash,
        )
        .await
        .unwrap();

    let legal_holds = PostgresLegalHoldResolver::new(pool.clone());
    legal_holds
        .set_hold(&subject_id, &format!("exhausted_lease_hold_{nonce}"), true)
        .await
        .unwrap();
    let worker =
        DeletionWorker::new(repository.clone(), Arc::new(legal_holds), Vec::new()).unwrap();

    for recovery in 1..=MAX_DELETION_LEASE_RECOVERIES {
        if recovery == 1 {
            sqlx::query(
                "INSERT INTO privacy_subject_deletion_fences \
                 (subject_id, job_id, status, lease_expires_at) \
                 VALUES ($1, $2, 'running', statement_timestamp() + interval '1 second')",
            )
            .bind(&subject_id)
            .bind(&job_id)
            .execute(&pool)
            .await
            .unwrap();
        } else {
            sqlx::query(
                "UPDATE privacy_subject_deletion_fences SET status = 'running', \
                 lease_expires_at = statement_timestamp() + interval '1 second', \
                 updated_at = statement_timestamp() \
                 WHERE subject_id = $1 AND job_id = $2 AND status = 'failed'",
            )
            .bind(&subject_id)
            .bind(&job_id)
            .execute(&pool)
            .await
            .unwrap();
        }
        sqlx::query(
            "UPDATE privacy_deletion_jobs SET status = 'running', failure_code = NULL, \
             lease_expires_at = statement_timestamp() + interval '1 second', \
             updated_at = statement_timestamp() WHERE job_id = $1",
        )
        .bind(&job_id)
        .execute(&pool)
        .await
        .unwrap();
        tokio::time::sleep(Duration::from_millis(1_200)).await;

        if recovery < MAX_DELETION_LEASE_RECOVERIES {
            let blocked = worker.execute(&job_id).await.unwrap();
            assert_eq!(blocked.status, DeletionJobStatus::BlockedLegalHold);
        } else {
            assert_eq!(
                worker.execute(&job_id).await.unwrap_err(),
                PrivacyError::LeaseRecoveryExhausted
            );
        }
    }

    let exhausted: (String, Option<String>, i64, bool, bool) = sqlx::query_as(
        "SELECT status, failure_code, lease_recovery_count, \
                last_lease_expired_at IS NOT NULL, lease_expires_at IS NULL \
         FROM privacy_deletion_jobs WHERE job_id = $1",
    )
    .bind(&job_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        exhausted,
        (
            "failed".to_owned(),
            Some("DELETION_LEASE_EXPIRED".to_owned()),
            MAX_DELETION_LEASE_RECOVERIES,
            true,
            true,
        )
    );
    assert!(
        sqlx::query(
            "UPDATE privacy_deletion_jobs SET status = 'running', failure_code = NULL, \
             lease_expires_at = statement_timestamp() + interval '5 minutes' \
             WHERE job_id = $1",
        )
        .bind(&job_id)
        .execute(&pool)
        .await
        .is_err(),
        "an exhausted recovery row must remain terminal database evidence"
    );
    let eligible: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM privacy_deletion_jobs \
         WHERE job_id = $1 AND status = 'failed' \
           AND failure_code = 'DELETION_LEASE_EXPIRED' \
           AND lease_recovery_count < $2)",
    )
    .bind(&job_id)
    .bind(MAX_DELETION_LEASE_RECOVERIES)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        !eligible,
        "the exhausted job must not be selected for retry"
    );
}

#[tokio::test]
async fn destroyed_subject_key_cannot_retain_key_material_on_insert() {
    let database_url = std::env::var("P05_DATABASE_URL")
        .expect("P05_DATABASE_URL must point to the P05 PostgreSQL test database");
    let repository = PostgresDeletionRepository::connect(&database_url)
        .await
        .expect("connect P05 PostgreSQL");
    repository.migrate().await.unwrap();
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let subject_id = format!("destroyed_key_material_{nonce}");

    assert!(
        sqlx::query(
            "INSERT INTO privacy_subject_keys \
             (subject_id, key_reference, wrapped_key, destroyed_at) \
             VALUES ($1, $2, $3, statement_timestamp())",
        )
        .bind(&subject_id)
        .bind(format!("destroyed_key_reference_{nonce}"))
        .bind(b"forbidden-retained-key-material".as_slice())
        .execute(repository.pool())
        .await
        .is_err(),
        "destroyed subject keys must reject retained wrapped key material on every write path"
    );
}

#[tokio::test]
async fn filesystem_verification_does_not_misreport_io_failures_as_absence() {
    let root = tempfile::Builder::new()
        .prefix("p05-deletion-verification-")
        .tempdir()
        .unwrap();
    let non_directory_root = root.path().join("not-a-directory");
    fs::write(&non_directory_root, b"not-a-directory").unwrap();
    let surface =
        FilesystemDeletionSurface::new(&non_directory_root, DeletionTarget::ObjectStorage).unwrap();

    let error = surface
        .verify_absent("subject_with_unreadable_root")
        .await
        .expect_err("an I/O failure is not proof that subject data is absent");

    assert_eq!(error, PrivacyError::Storage);
    fs::remove_file(non_directory_root).unwrap();
    root.close().unwrap();
}

#[tokio::test]
async fn retained_security_and_privacy_history_rejects_bulk_removal() {
    let database_url = std::env::var("P05_DATABASE_URL")
        .expect("P05_DATABASE_URL must point to the P05 PostgreSQL test database");
    let repository = PostgresDeletionRepository::connect(&database_url)
        .await
        .expect("connect P05 PostgreSQL");
    repository.migrate().await.expect("apply P05 migrations");

    for statement in [
        "TRUNCATE TABLE campaign_group_memberships",
        "TRUNCATE TABLE cloud_egress_audit",
        "TRUNCATE TABLE privacy_deletion_jobs CASCADE",
        "TRUNCATE TABLE privacy_subject_deletion_fences",
        "TRUNCATE TABLE privacy_erased_subjects",
        "TRUNCATE TABLE privacy_subject_keys",
        "TRUNCATE TABLE privacy_legal_holds",
        "DELETE FROM privacy_deletion_jobs WHERE false",
        "DELETE FROM privacy_deletion_job_targets WHERE false",
        "DELETE FROM privacy_subject_deletion_fences WHERE false",
        "DELETE FROM privacy_erased_subjects WHERE false",
        "DELETE FROM privacy_subject_keys WHERE false",
        "DELETE FROM privacy_legal_holds WHERE false",
    ] {
        assert!(
            sqlx::query(statement)
                .execute(repository.pool())
                .await
                .is_err(),
            "retained security history unexpectedly accepted: {statement}"
        );
    }
}
