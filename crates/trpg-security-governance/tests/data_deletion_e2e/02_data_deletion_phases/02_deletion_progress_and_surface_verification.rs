{
    let queue_progress_cursor: i64 = sqlx::query_scalar(
        "SELECT progress_cursor FROM privacy_deletion_job_targets \
         WHERE job_id = $1 AND target = 'queue'",
    )
    .bind(&job_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        queue_progress_cursor > 1,
        "queue erasure must persist a resumable cursor across bounded batches"
    );

    // Re-open through a fresh repository handle to prove status is durable,
    // then independently query every backing store instead of trusting Worker
    // return values.
    let reloaded = PostgresDeletionRepository::new(pool.clone())
        .load(&job_id)
        .await
        .unwrap();
    assert_eq!(reloaded.status, DeletionJobStatus::Completed);
    assert!(reloaded.all_targets_verified());
    let remaining_rag_records: i64 =
        sqlx::query_scalar("SELECT count(*) FROM rag_snapshot_chunk WHERE visibility_subject = $1")
            .bind(&subject_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(remaining_rag_records, 0);
    assert!(cache.verify_absent(&subject_id).await.unwrap());
    assert!(queue.verify_absent(&subject_id).await.unwrap());
    assert!(object_storage.verify_absent(&subject_id).await.unwrap());
    assert!(!export_root.join(&subject_id).exists());
    let key_row = sqlx::query(
        "SELECT wrapped_key, destroyed_at IS NOT NULL AS destroyed \
         FROM privacy_subject_keys WHERE subject_id = $1",
    )
    .bind(&subject_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let wrapped_key: Option<Vec<u8>> = key_row.try_get("wrapped_key").unwrap();
    let destroyed: bool = key_row.try_get("destroyed").unwrap();
    assert!(wrapped_key.is_none());
    assert!(destroyed);
    assert!(store
        .load_replay_page(&event_draft.campaign_id, 0, 10)
        .await
        .unwrap()
        .is_empty());
    let erased_identity = sqlx::query(
        "SELECT login_normalized, password_hash, disabled_at IS NOT NULL AS disabled \
         FROM users WHERE user_id = $1",
    )
    .bind(&subject_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(erased_identity
        .get::<String, _>("login_normalized")
        .starts_with("deleted_"));
    assert!(erased_identity
        .get::<String, _>("password_hash")
        .starts_with("DELETED_ACCOUNT_NO_LOGIN_"));
    assert!(erased_identity.get::<bool, _>("disabled"));
    let session_count: i64 = sqlx::query_scalar("SELECT count(*) FROM sessions WHERE user_id = $1")
        .bind(&subject_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(session_count, 0);
    let protected_event: serde_json::Value =
        sqlx::query_scalar("SELECT payload_json FROM event_store WHERE sequence = $1")
            .bind(persisted.first_event_sequence)
            .fetch_one(&pool)
            .await
            .unwrap();
    let protected_wire = serde_json::to_string(&protected_event).unwrap();
    assert!(protected_wire.contains("protected_payload"));
    assert!(!protected_wire.contains("deletion-secret"));

    let completion_updated_at: String = sqlx::query_scalar(
        "SELECT updated_at::text FROM privacy_deletion_jobs WHERE job_id = $1",
    )
    .bind(&job_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    // A clean post-completion revalidation appends a result without mutating
    // the original job or its verified target evidence.
    assert_eq!(
        worker.execute(&job_id).await.unwrap().status,
        DeletionJobStatus::Completed
    );
    let passed_revalidation = sqlx::query(
        "SELECT result.result_status, result.alert_status \
           FROM privacy_deletion_revalidation_results AS result \
           JOIN privacy_deletion_revalidation_runs AS run \
             ON run.run_id = result.run_id \
          WHERE run.job_id = $1 \
          ORDER BY run.started_at DESC, run.run_id DESC LIMIT 1",
    )
    .bind(&job_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        passed_revalidation.get::<String, _>("result_status"),
        "passed"
    );
    assert_eq!(
        passed_revalidation.get::<String, _>("alert_status"),
        "not_required"
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT updated_at::text FROM privacy_deletion_jobs WHERE job_id = $1",
        )
        .bind(&job_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        completion_updated_at
    );

    // Restore a real external surface after completion. Revalidation is
    // deliberately verify-only: it must expose resurgence, append durable
    // failure/alert evidence, and leave the completion proof untouched.
    fs::create_dir_all(export_root.join(&subject_id)).unwrap();
    fs::write(
        export_root.join(&subject_id).join("resurfaced-export.bin"),
        b"resurfaced-protected-export",
    )
    .unwrap();
    assert_eq!(
        worker.execute(&job_id).await.unwrap_err(),
        PrivacyError::VerificationFailed(DeletionTarget::Export)
    );
    assert!(
        export_root.join(&subject_id).exists(),
        "revalidation must not erase resurfaced data before recording failure"
    );
    let failed_revalidation = sqlx::query(
        "SELECT result.result_id, result.result_status, result.failure_target, \
                result.error_code, result.alert_status, result.evidence_hash \
           FROM privacy_deletion_revalidation_results AS result \
           JOIN privacy_deletion_revalidation_runs AS run \
             ON run.run_id = result.run_id \
          WHERE run.job_id = $1 \
          ORDER BY run.started_at DESC, run.run_id DESC LIMIT 1",
    )
    .bind(&job_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        failed_revalidation.get::<String, _>("result_status"),
        "failed"
    );
    assert_eq!(
        failed_revalidation.get::<String, _>("failure_target"),
        "export"
    );
    assert_eq!(
        failed_revalidation.get::<String, _>("error_code"),
        "DELETION_VERIFICATION_FAILED"
    );
    assert_eq!(
        failed_revalidation.get::<String, _>("alert_status"),
        "pending_acknowledgement"
    );
    assert!(failed_revalidation
        .get::<String, _>("evidence_hash")
        .starts_with("sha256:"));
    let result_id = failed_revalidation.get::<String, _>("result_id");
    let immutable_result = sqlx::query(
        "UPDATE privacy_deletion_revalidation_results \
            SET alert_status = 'not_required' WHERE result_id = $1",
    )
    .bind(result_id)
    .execute(&pool)
    .await
    .expect_err("terminal revalidation evidence must be immutable");
    assert_eq!(
        immutable_result
            .as_database_error()
            .and_then(|error| error.code())
            .as_deref(),
        Some("P0001")
    );

    let incomplete_completed_worker =
        DeletionWorker::new(repository.clone(), Arc::new(legal_holds), Vec::new()).unwrap();
    assert_eq!(
        incomplete_completed_worker
            .execute(&job_id)
            .await
            .unwrap_err(),
        PrivacyError::MissingSurface(DeletionTarget::Database)
    );
    let still_completed = repository.load(&job_id).await.unwrap();
    assert_eq!(still_completed.status, DeletionJobStatus::Completed);
    assert!(still_completed.all_targets_verified());
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT updated_at::text FROM privacy_deletion_jobs WHERE job_id = $1",
        )
        .bind(&job_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        completion_updated_at
    );

    root.close().unwrap();
}
