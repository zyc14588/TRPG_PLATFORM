impl PostgresCanonicalStore {
    pub async fn verify_integrity(&self) -> Result<(), CanonicalStoreError> {
        let audit_records = self.load_verified_audit_records().await?;
        let primary_commits = sqlx::query(
            r#"
            SELECT commit_id, campaign_id, stream_id, idempotency_key,
                   expected_version, status, idempotency_operation, request_hash,
                   first_event_sequence, last_event_sequence,
                   first_stream_version, last_stream_version, audit_sequence,
                   result_event_sequence, witness_prepare_sequence,
                   witness_prepare_hash,
                   (response_payload->>'first_event_sequence')::bigint
                       AS response_first_event_sequence,
                   (response_payload->>'last_event_sequence')::bigint
                       AS response_last_event_sequence,
                   (response_payload->>'first_stream_version')::bigint
                       AS response_first_stream_version,
                   (response_payload->>'last_stream_version')::bigint
                       AS response_last_stream_version
              FROM formal_commits
             ORDER BY committed_at, commit_id
            "#,
        )
        .fetch_all(&self.primary)
        .await
        .map_err(|_| CanonicalStoreError::PrimaryWrite {
            operation: "verify_primary_commits",
        })?;
        for row in primary_commits {
            self.verify_primary_commit_integrity(&row, &audit_records)
                .await?;
        }
        Ok(())
    }
}
