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

    // Completion is idempotent and does not recreate or reclassify data.
    assert_eq!(
        worker.execute(&job_id).await.unwrap().status,
        DeletionJobStatus::Completed
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

    root.close().unwrap();
}
