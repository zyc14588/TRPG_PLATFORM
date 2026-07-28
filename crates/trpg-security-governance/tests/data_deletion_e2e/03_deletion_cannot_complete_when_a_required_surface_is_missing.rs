
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn deletion_cannot_complete_when_a_required_surface_is_missing() {
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
    .expect("connect canonical store for missing-surface evidence");
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
    let job_id = format!("missing_surface_{nonce}");
    let subject_id = format!("missing_subject_{nonce}");
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
    let second_nonce = nonce + 1;
    let held_job_id = format!("held_after_failed_job_{second_nonce}");
    let held_subject_id = format!("held_after_failed_subject_{second_nonce}");
    let (held_event_sequence, held_event_hash) = commit_deletion_request(
        &store,
        repository.pool(),
        second_nonce,
        &held_subject_id,
        &held_job_id,
    )
    .await;
    repository
        .record_confirmed(
            &held_job_id,
            &held_subject_id,
            "privacy_officer",
            "user_erasure_v1",
            &deletion_evidence(second_nonce),
            held_event_sequence,
            &held_event_hash,
        )
        .await
        .unwrap();
    let legal_holds = PostgresLegalHoldResolver::new(pool);
    legal_holds
        .set_hold(
            &held_subject_id,
            &format!("held_after_failure_{second_nonce}"),
            true,
        )
        .await
        .unwrap();
    let worker =
        DeletionWorker::new(repository.clone(), Arc::new(legal_holds), Vec::new()).unwrap();

    assert_eq!(
        worker.execute_next(100).await.unwrap_err(),
        PrivacyError::MissingSurface(DeletionTarget::Database)
    );
    let failed = repository.load(&job_id).await.unwrap();
    assert_eq!(failed.status, DeletionJobStatus::Failed);
    assert!(!failed.all_targets_verified());
    assert_eq!(
        repository.load(&held_job_id).await.unwrap().status,
        DeletionJobStatus::BlockedLegalHold,
        "a failed job must not prevent later claimed jobs from being processed"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn failure_cleanup_never_masks_the_surface_error_after_lease_loss() {
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
    .expect("connect canonical store for failure-cleanup evidence");
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
    let job_id = format!("lease_loss_surface_{nonce}");
    let subject_id = format!("lease_loss_subject_{nonce}");
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
    let worker = DeletionWorker::new(
        repository.clone(),
        Arc::new(PostgresLegalHoldResolver::new(pool.clone())),
        vec![Box::new(SimulatedLeaseLossSurface { pool })],
    )
    .unwrap();

    assert_eq!(
        worker.execute(&job_id).await.unwrap_err(),
        PrivacyError::Storage,
        "best-effort failure bookkeeping must not replace the originating surface error"
    );
    let failed = repository.load(&job_id).await.unwrap();
    assert_eq!(failed.status, DeletionJobStatus::Failed);
    assert_eq!(failed.failure_code.as_deref(), Some("SIMULATED_LEASE_LOSS"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn expired_deletion_executions_are_reclaimed_and_counted_before_retry() {
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
    .expect("connect canonical store for expired-lease evidence");
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
    let mut expired_jobs = Vec::new();
    for (offset, active_status) in ["running", "verifying"].into_iter().enumerate() {
        let phase_nonce = nonce + offset as u128;
        let job_id = format!("expired_{active_status}_{phase_nonce}");
        let subject_id = format!("expired_subject_{active_status}_{phase_nonce}");
        let (event_sequence, event_hash) =
            commit_deletion_request(&store, repository.pool(), phase_nonce, &subject_id, &job_id)
                .await;
        repository
            .record_confirmed(
                &job_id,
                &subject_id,
                "privacy_officer",
                "user_erasure_v1",
                &deletion_evidence(phase_nonce),
                event_sequence,
                &event_hash,
            )
            .await
            .unwrap();

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
        sqlx::query(
            "UPDATE privacy_deletion_jobs SET status = 'running', \
             lease_expires_at = statement_timestamp() + interval '1 second' \
             WHERE job_id = $1",
        )
        .bind(&job_id)
        .execute(&pool)
        .await
        .unwrap();
        if active_status == "verifying" {
            sqlx::query(
                "UPDATE privacy_deletion_jobs SET status = 'verifying' \
                 WHERE job_id = $1",
            )
            .bind(&job_id)
            .execute(&pool)
            .await
            .unwrap();
        }
        expired_jobs.push((job_id, subject_id));
    }
    tokio::time::sleep(Duration::from_millis(1_200)).await;

    let worker = DeletionWorker::new(
        repository.clone(),
        Arc::new(PostgresLegalHoldResolver::new(pool.clone())),
        Vec::new(),
    )
    .unwrap();
    for (job_id, subject_id) in expired_jobs {
        assert_eq!(
            worker.execute(&job_id).await.unwrap_err(),
            PrivacyError::MissingSurface(DeletionTarget::Database)
        );

        let recovered: (String, i64, bool, bool) = sqlx::query_as(
            "SELECT status, lease_recovery_count, \
                    last_lease_expired_at IS NOT NULL, lease_expires_at IS NULL \
             FROM privacy_deletion_jobs WHERE job_id = $1",
        )
        .bind(&job_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(recovered, ("failed".to_owned(), 1, true, true));
        let fence: (String, bool) = sqlx::query_as(
            "SELECT status, lease_expires_at IS NULL \
             FROM privacy_subject_deletion_fences WHERE subject_id = $1",
        )
        .bind(&subject_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(fence, ("failed".to_owned(), true));
    }
    assert!(repository.lease_recovery_total().await.unwrap() >= 2);
}
