{

    let probe_nonce = nonce + 1;
    let probe_subject_id = format!("claim_probe_subject_{probe_nonce}");
    let probe_job_id = format!("claim_probe_job_{probe_nonce}");
    let (probe_event_sequence, probe_event_hash) =
        commit_deletion_request(
            &store,
            &pool,
            probe_nonce,
            &probe_subject_id,
            &probe_job_id,
        )
        .await;
    repository
        .record_confirmed(
            &probe_job_id,
            &probe_subject_id,
            "privacy_officer",
            "user_erasure_v1",
            &deletion_evidence(probe_nonce),
            probe_event_sequence,
            &probe_event_hash,
        )
        .await
        .expect("record the constrained-claim negative fixture");
    let probe_claim_token = format!("ar01-claim-token-{probe_nonce}");
    let mut probe_transaction = pool.begin().await.unwrap();
    sqlx::query(
        "UPDATE privacy_deletion_jobs \
            SET status = 'running', execution_claim_token = $2, \
                lease_expires_at = statement_timestamp() + interval '5 minutes' \
          WHERE job_id = $1",
    )
    .bind(&probe_job_id)
    .bind(&probe_claim_token)
    .execute(&mut *probe_transaction)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO privacy_subject_deletion_fences \
         (subject_id, job_id, status, lease_expires_at, execution_claim_token) \
         VALUES ($1, $2, 'running', \
                 statement_timestamp() + interval '5 minutes', $3)",
    )
    .bind(&probe_subject_id)
    .bind(&probe_job_id)
    .bind(&probe_claim_token)
    .execute(&mut *probe_transaction)
    .await
    .unwrap();
    probe_transaction.commit().await.unwrap();

    let wrong_claim = sqlx::query(
        "SELECT public.erase_privacy_database_subject($1, $2, $3)",
    )
    .bind(&probe_job_id)
    .bind(&probe_subject_id)
    .bind("wrong-claim-token")
    .execute(worker_repository.pool())
    .await
    .expect_err("a wrong deletion claim token must fail closed");
    assert_eq!(
        wrong_claim
            .as_database_error()
            .and_then(|error| error.code())
            .as_deref(),
        Some("42501")
    );

    let non_target = sqlx::query(
        "SELECT public.erase_privacy_database_subject($1, $2, $3)",
    )
    .bind(&probe_job_id)
    .bind(&subject_id)
    .bind(&probe_claim_token)
    .execute(worker_repository.pool())
    .await
    .expect_err("a live claim must not authorize another subject");
    assert_eq!(
        non_target
            .as_database_error()
            .and_then(|error| error.code())
            .as_deref(),
        Some("42501")
    );

    let non_running = sqlx::query(
        "SELECT public.erase_privacy_database_subject($1, $2, $3)",
    )
    .bind(&job_id)
    .bind(&subject_id)
    .bind(&probe_claim_token)
    .execute(worker_repository.pool())
    .await
    .expect_err("a requested job must not authorize erasure");
    assert_eq!(
        non_running
            .as_database_error()
            .and_then(|error| error.code())
            .as_deref(),
        Some("42501")
    );

    let arbitrary_update =
        sqlx::query("UPDATE users SET disabled_at = now() WHERE user_id = $1")
            .bind(&subject_id)
            .execute(worker_repository.pool())
            .await
            .expect_err("worker must not retain direct arbitrary user updates");
    assert_eq!(
        arbitrary_update
            .as_database_error()
            .and_then(|error| error.code())
            .as_deref(),
        Some("42501")
    );

    sqlx::query(
        "INSERT INTO privacy_deletion_surface_records \
         (surface, subject_id, record_key, protected_payload) \
         VALUES ('database', $1, 'legacy-preview', decode('00', 'hex'))",
    )
    .bind(&probe_subject_id)
    .execute(&pool)
    .await
    .unwrap();
    let legacy_cleanup = sqlx::query(
        "DELETE FROM privacy_deletion_surface_records \
         WHERE surface = 'database' AND subject_id = $1",
    )
    .bind(&probe_subject_id)
    .execute(worker_repository.pool())
    .await
    .expect_err("worker must not gain legacy evidence-table DELETE");
    assert_eq!(
        legacy_cleanup
            .as_database_error()
            .and_then(|error| error.code())
            .as_deref(),
        Some("42501")
    );

    sqlx::query(
        "SELECT public.erase_privacy_database_subject($1, $2, $3)",
    )
    .bind(&probe_job_id)
    .bind(&probe_subject_id)
    .bind(&probe_claim_token)
    .execute(worker_repository.pool())
    .await
    .expect("the exact live claim authorizes its fixed-predicate erasure");

    let mut probe_cleanup = pool.begin().await.unwrap();
    let fence_cleanup = sqlx::query(
        "UPDATE privacy_subject_deletion_fences \
            SET status = 'failed', lease_expires_at = NULL, \
                execution_claim_token = NULL, updated_at = statement_timestamp() \
          WHERE subject_id = $1 AND job_id = $2 AND status = 'running' \
            AND execution_claim_token = $3",
    )
    .bind(&probe_subject_id)
    .bind(&probe_job_id)
    .bind(&probe_claim_token)
    .execute(&mut *probe_cleanup)
    .await
    .unwrap()
    .rows_affected();
    assert_eq!(fence_cleanup, 1);
    let job_cleanup = sqlx::query(
        "UPDATE privacy_deletion_jobs \
            SET status = 'failed', failure_code = 'AR01_CLAIM_PROBE_COMPLETE', \
                lease_expires_at = NULL, execution_claim_token = NULL, \
                updated_at = statement_timestamp() \
          WHERE job_id = $1 AND subject_id = $2 AND status = 'running' \
            AND execution_claim_token = $3",
    )
    .bind(&probe_job_id)
    .bind(&probe_subject_id)
    .bind(&probe_claim_token)
    .execute(&mut *probe_cleanup)
    .await
    .unwrap()
    .rows_affected();
    assert_eq!(job_cleanup, 1);
    probe_cleanup.commit().await.unwrap();

    publisher
        .publish_batch()
        .await
        .expect("publish canonical deletion request before execution");

    let blocked = worker.execute(&job_id).await.unwrap();
    assert_eq!(blocked.status, DeletionJobStatus::BlockedLegalHold);
    assert!(!object_storage.verify_absent(&subject_id).await.unwrap());
    let rag_rows_before_release: i64 =
        sqlx::query_scalar("SELECT count(*) FROM rag_snapshot_chunk WHERE visibility_subject = $1")
            .bind(&subject_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(rag_rows_before_release, 1);
    assert!(!cache.verify_absent(&subject_id).await.unwrap());
    assert!(!queue.verify_absent(&subject_id).await.unwrap());

    legal_holds
        .set_hold(&subject_id, &hold_reference, false)
        .await
        .unwrap();
    let completed = worker.execute(&job_id).await.unwrap();
    assert_eq!(completed.status, DeletionJobStatus::Completed);
    assert!(completed.all_targets_verified());
    assert!(completed
        .targets
        .iter()
        .all(|target| target.status == DeletionTargetStatus::Verified));
    include!("../02_deletion_progress_and_surface_verification.rs");
}
