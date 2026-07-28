
impl PostgresCanonicalStore {

    /// Loads a bounded, ordered replay page. Authorization and visibility
    /// filtering remain the responsibility of the production composition
    /// root, which owns this store and the live identity verifier together.
    pub async fn load_replay_page(
        &self,
        campaign_id: &str,
        after_sequence: i64,
        limit: i64,
    ) -> Result<Vec<CanonicalReplayEvent>, CanonicalStoreError> {
        self.verify_integrity().await?;
        load_canonical_replay_page(
            &self.primary,
            &self.payload_cipher,
            campaign_id,
            after_sequence,
            limit,
        )
        .await
    }

    /// Loads one fact-promotion source from the durable canonical store. This
    /// path rejects historical/unverified rows, proves the full primary and
    /// external-witness chains, verifies encryption/HMAC metadata, and returns
    /// an immutable domain evidence capability rather than raw caller fields.
    pub async fn load_committed_fact_evidence(
        &self,
        campaign_id: &str,
        event_sequence: i64,
        target_fact_id: &str,
    ) -> Result<CommittedFactEvidence, CanonicalStoreError> {
        if event_sequence <= 0 {
            return Err(CanonicalStoreError::Validation(
                "positive_event_sequence_required",
            ));
        }
        self.verify_integrity().await?;
        let mut events = self
            .load_replay_page(campaign_id, event_sequence - 1, 1)
            .await?;
        let event = events.pop().ok_or(CanonicalStoreError::Validation(
            "committed_fact_event_missing",
        ))?;
        if event.sequence != event_sequence
            || event.integrity_status != "verified_hmac"
            || event.request_hash_source != "formal_commit"
            || event.event_integrity_hash.is_none()
        {
            return Err(CanonicalStoreError::IntegrityViolation(
                "committed_fact_event_unverified",
            ));
        }
        let protected_source: Value = serde_json::from_str(&event.payload_integrity_source)
            .map_err(|_| {
                CanonicalStoreError::IntegrityViolation("event_payload_integrity_source_invalid")
            })?;
        if protected_source.get("protected_payload").is_none() {
            return Err(CanonicalStoreError::IntegrityViolation(
                "committed_fact_payload_not_encrypted",
            ));
        }
        let payload: CommandAcceptedPayload = serde_json::from_value(event.payload)
            .map_err(|_| CanonicalStoreError::Validation("committed_fact_payload_invalid"))?;
        if payload.target_fact_id != target_fact_id {
            return Err(CanonicalStoreError::IntegrityViolation(
                "committed_fact_target_mismatch",
            ));
        }
        let visibility = Visibility::try_from_parts(
            &event.visibility_label,
            nonempty_subject(&event.visibility_subject),
        )
        .map_err(|_| CanonicalStoreError::Validation("committed_fact_visibility_invalid"))?;
        let provenance_kind = persisted_provenance_kind(&event.provenance_kind)?;
        let provenance = FactProvenance::new(
            provenance_kind,
            event.provenance_reference,
            event.provenance_recorded_by,
        )
        .map_err(|_| CanonicalStoreError::Validation("committed_fact_provenance_invalid"))?;
        let record = PersistedFactEvidenceRecord::seal_verified(
            payload.target_fact_id,
            event.campaign_id,
            u64::try_from(event.sequence).map_err(|_| {
                CanonicalStoreError::IntegrityViolation("committed_fact_sequence_invalid")
            })?,
            event.stream_id,
            u64::try_from(event.stream_version).map_err(|_| {
                CanonicalStoreError::IntegrityViolation("committed_fact_stream_version_invalid")
            })?,
            event.event_type,
            payload.kind,
            payload.fact_source,
            visibility,
            provenance,
            event
                .event_integrity_hash
                .expect("checked verified canonical event integrity hash"),
            event.request_hash,
            self.integrity_key(),
        )
        .map_err(|_| {
            CanonicalStoreError::IntegrityViolation("committed_fact_evidence_seal_failed")
        })?;
        CommittedFactEvidence::load_persisted(&record, self.integrity_key())
            .map_err(|_| CanonicalStoreError::IntegrityViolation("committed_fact_evidence_invalid"))
    }
}
