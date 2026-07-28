
impl PostgresCanonicalStore {

    pub async fn recover(&self) -> Result<RecoveryReport, CanonicalStoreError> {
        // Recovery is itself an append operation. Validate both existing
        // cryptographic chains before resolving an incomplete PREPARED row so
        // a process configured with the wrong key cannot irreversibly append
        // an ABORTED/COMMITTED record to an otherwise valid witness.
        self.verify_cryptographic_chains().await?;
        let rows = sqlx::query(
            r#"
            SELECT p.sequence, p.commit_id, p.primary_request_hash, p.record_hash
              FROM external_audit_witness p
             WHERE p.phase = 'PREPARED'
               AND NOT EXISTS (
                   SELECT 1 FROM external_audit_witness terminal
                    WHERE terminal.commit_id = p.commit_id
                      AND terminal.phase IN ('COMMITTED', 'ABORTED')
               )
             ORDER BY p.sequence
            "#,
        )
        .fetch_all(&self.witness)
        .await
        .map_err(|_| CanonicalStoreError::WitnessWrite {
            operation: "scan_recovery_candidates",
        })?;

        let mut report = RecoveryReport::default();
        for row in rows {
            let commit_id: String = row.get("commit_id");
            let request_hash: String = row.get("primary_request_hash");
            let prepare_sequence: i64 = row.get("sequence");
            let prepare_hash: String = row.get("record_hash");
            if let Some(existing) = self.load_existing_commit(&commit_id, "", "", "").await? {
                if existing.witness_prepare_sequence != prepare_sequence
                    || existing.witness_prepare_hash != prepare_hash
                {
                    return Err(CanonicalStoreError::IntegrityViolation(
                        "primary_witness_prepare_binding_mismatch",
                    ));
                }
                self.finalize_witness(&existing, &request_hash).await?;
                report.finalized += 1;
            } else {
                self.append_witness(
                    &commit_id,
                    WitnessPhase::Aborted,
                    &request_hash,
                    None,
                    None,
                    "primary_transaction_absent_after_recovery",
                )
                .await?;
                report.aborted += 1;
            }
        }
        self.verify_integrity().await?;
        Ok(report)
    }
}
