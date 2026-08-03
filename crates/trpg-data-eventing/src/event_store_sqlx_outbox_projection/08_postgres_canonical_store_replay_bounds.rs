impl PostgresCanonicalStore {
    /// Returns the retained canonical cursor range for one campaign.
    ///
    /// The same eligibility predicates as replay are used so an unverified or
    /// crypto-erased row cannot make a resume token appear valid.
    pub async fn replay_sequence_bounds(
        &self,
        campaign_id: &str,
    ) -> Result<Option<(u64, u64)>, CanonicalStoreError> {
        if campaign_id.trim().is_empty()
            || campaign_id.len() > 128
            || !campaign_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        {
            return Err(CanonicalStoreError::Validation("campaign_id_required"));
        }
        self.verify_integrity().await?;
        let row = sqlx::query(
            r#"
            SELECT min(sequence) AS earliest, max(sequence) AS latest
              FROM event_store
             WHERE campaign_id = $1
               AND integrity_status = 'verified_hmac'
               AND request_hash_source = 'formal_commit'
               AND event_integrity_hash IS NOT NULL
               AND payload_json ? 'protected_payload'
               AND (
                   data_subject_id = 'not_applicable'
                   OR EXISTS (
                       SELECT 1 FROM privacy_subject_keys AS subject_key
                        WHERE subject_key.subject_id = event_store.data_subject_id
                          AND subject_key.key_reference = event_store.payload_key_reference
                          AND subject_key.wrapped_key IS NOT NULL
                          AND subject_key.destroyed_at IS NULL
                   )
               )
            "#,
        )
        .bind(campaign_id)
        .fetch_one(&self.primary)
        .await
        .map_err(|_| CanonicalStoreError::PrimaryWrite {
            operation: "load_replay_bounds",
        })?;
        let earliest = row.try_get::<Option<i64>, _>("earliest").map_err(|_| {
            CanonicalStoreError::IntegrityViolation("replay_bound_sequence_invalid")
        })?;
        let latest = row.try_get::<Option<i64>, _>("latest").map_err(|_| {
            CanonicalStoreError::IntegrityViolation("replay_bound_sequence_invalid")
        })?;
        match (earliest, latest) {
            (None, None) => Ok(None),
            (Some(earliest), Some(latest)) if earliest > 0 && latest >= earliest => Ok(Some((
                u64::try_from(earliest).map_err(|_| {
                    CanonicalStoreError::IntegrityViolation("replay_bound_sequence_invalid")
                })?,
                u64::try_from(latest).map_err(|_| {
                    CanonicalStoreError::IntegrityViolation("replay_bound_sequence_invalid")
                })?,
            ))),
            _ => Err(CanonicalStoreError::IntegrityViolation(
                "replay_bound_sequence_invalid",
            )),
        }
    }
}
