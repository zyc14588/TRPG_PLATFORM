impl PostgresCanonicalStore {
    async fn verify_primary_commit_integrity(
        &self,
        row: &sqlx::postgres::PgRow,
        audit_records: &[AuditRecord],
    ) -> Result<(), CanonicalStoreError> {
        let commit_id: String = row.get("commit_id");
        let formal_campaign_id: String = row.get("campaign_id");
        let formal_stream_id: String = row.get("stream_id");
        let formal_idempotency_key: String = row.get("idempotency_key");
        let formal_expected_version: i64 = row.get("expected_version");
        let request_hash: String = row.get("request_hash");
        let first_event_sequence: i64 = row.get("first_event_sequence");
        let last_event_sequence: i64 = row.get("last_event_sequence");
        let first_stream_version: i64 = row.get("first_stream_version");
        let last_stream_version: i64 = row.get("last_stream_version");
        let audit_sequence: i64 = row.get("audit_sequence");
        let prepare_sequence: i64 = row.get("witness_prepare_sequence");
        let prepare_hash: String = row.get("witness_prepare_hash");
        if row.get::<String, _>("status") != "committed"
            || row.get::<String, _>("idempotency_operation") != CANONICAL_IDEMPOTENCY_OPERATION
            || row.get::<i64, _>("result_event_sequence") != last_event_sequence
            || row.get::<i64, _>("response_first_event_sequence") != first_event_sequence
            || row.get::<i64, _>("response_last_event_sequence") != last_event_sequence
            || row.get::<i64, _>("response_first_stream_version") != first_stream_version
            || row.get::<i64, _>("response_last_stream_version") != last_stream_version
        {
            return Err(CanonicalStoreError::IntegrityViolation(
                "formal_commit_receipt_metadata_mismatch",
            ));
        }
        let audit = audit_records
            .iter()
            .find(|record| record.sequence == audit_sequence)
            .ok_or(CanonicalStoreError::IntegrityViolation(
                "formal_commit_audit_record_missing",
            ))?;
        if audit.commit_id != commit_id
            || audit.campaign_id != formal_campaign_id
            || audit.decision != "PERMIT"
            || audit.witness_prepare_sequence != prepare_sequence
            || audit.witness_prepare_hash != prepare_hash
        {
            return Err(CanonicalStoreError::IntegrityViolation(
                "formal_commit_audit_binding_mismatch",
            ));
        }
        let prepared_count: i64 = sqlx::query_scalar(
            r#"
            SELECT count(*) FROM external_audit_witness
             WHERE commit_id = $1 AND phase = 'PREPARED'
               AND sequence = $2 AND record_hash = $3
               AND primary_request_hash = $4
            "#,
        )
        .bind(&commit_id)
        .bind(prepare_sequence)
        .bind(&prepare_hash)
        .bind(&request_hash)
        .fetch_one(&self.witness)
        .await
        .map_err(|_| CanonicalStoreError::WitnessWrite {
            operation: "verify_primary_prepare",
        })?;
        let committed_count: i64 = sqlx::query_scalar(
            r#"
            SELECT count(*) FROM external_audit_witness
             WHERE commit_id = $1 AND phase = 'COMMITTED'
               AND primary_request_hash = $2
               AND primary_first_sequence = $3
               AND primary_last_sequence = $4
            "#,
        )
        .bind(&commit_id)
        .bind(&request_hash)
        .bind(first_event_sequence)
        .bind(last_event_sequence)
        .fetch_one(&self.witness)
        .await
        .map_err(|_| CanonicalStoreError::WitnessWrite {
            operation: "verify_primary_finalize",
        })?;
        if prepared_count != 1 || committed_count != 1 {
            return Err(CanonicalStoreError::IntegrityViolation(
                "primary_commit_missing_external_witness",
            ));
        }

        self.verify_primary_commit_events(PrimaryCommitEventBindings {
            commit_id,
            formal_campaign_id,
            formal_stream_id,
            formal_idempotency_key,
            formal_expected_version,
            request_hash,
            first_event_sequence,
            last_event_sequence,
            first_stream_version,
            last_stream_version,
            audit,
        })
        .await?;
        Ok(())
    }
}
