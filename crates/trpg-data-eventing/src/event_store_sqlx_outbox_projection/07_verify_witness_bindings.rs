impl PostgresCanonicalStore {
    async fn load_verified_audit_records(
        &self,
    ) -> Result<Vec<AuditRecord>, CanonicalStoreError> {
    let witness_records = self.load_witness_records().await?;
    verify_witness_chain(&witness_records, self.integrity_key())?;
    let audit_records = self.load_audit_records().await?;
    verify_audit_chain(&audit_records, self.integrity_key())?;

    let commits = sqlx::query(
        r#"
        SELECT commit_id, primary_request_hash, sequence, phase,
               primary_first_sequence, primary_last_sequence, record_hash
          FROM external_audit_witness
         WHERE phase IN ('PREPARED', 'COMMITTED', 'ABORTED')
         ORDER BY sequence
        "#,
    )
    .fetch_all(&self.witness)
    .await
    .map_err(|_| CanonicalStoreError::WitnessWrite {
        operation: "verify_bindings",
    })?;

    for row in commits {
        let phase: String = row.get("phase");
        let commit_id: String = row.get("commit_id");
        let primary = self.load_existing_commit(&commit_id, "", "", "").await?;
        match (phase.as_str(), primary) {
            ("PREPARED", _) => {}
            ("ABORTED", None) => {}
            ("ABORTED", Some(_)) => {
                return Err(CanonicalStoreError::IntegrityViolation(
                    "aborted_witness_has_primary_commit",
                ));
            }
            ("COMMITTED", Some(persisted)) => {
                let request_hash: String = row.get("primary_request_hash");
                let first: Option<i64> = row.get("primary_first_sequence");
                let last: Option<i64> = row.get("primary_last_sequence");
                if first != Some(persisted.first_event_sequence)
                    || last != Some(persisted.last_event_sequence)
                {
                    return Err(CanonicalStoreError::IntegrityViolation(
                        "committed_witness_primary_range_mismatch",
                    ));
                }
                let stored_request: String = sqlx::query_scalar(
                    "SELECT request_hash FROM formal_commits WHERE commit_id = $1",
                )
                .bind(&commit_id)
                .fetch_one(&self.primary)
                .await
                .map_err(|_| CanonicalStoreError::PrimaryWrite {
                    operation: "verify_request_binding",
                })?;
                if request_hash != stored_request {
                    return Err(CanonicalStoreError::IntegrityViolation(
                        "committed_witness_request_mismatch",
                    ));
                }
            }
            ("COMMITTED", None) => {
                return Err(CanonicalStoreError::IntegrityViolation(
                    "committed_witness_missing_primary_commit",
                ));
            }
            _ => {
                return Err(CanonicalStoreError::IntegrityViolation(
                    "unknown_witness_phase",
                ));
            }
        }
    }

        Ok(audit_records)
    }
}
