
#[async_trait]
impl DeletionRequestPort for PostgresDeletionRepository {
    async fn request_deletion(
        &self,
        job_id: &str,
        subject_id: &str,
        requested_by: &str,
        retention_policy: &str,
        evidence: &DeletionRequestEvidence,
    ) -> Result<DeletionJob, PrivacyError> {
        self.request(job_id, subject_id, requested_by, retention_policy, evidence)
            .await
    }

    async fn confirm_deletion_evidence(
        &self,
        job_id: &str,
        evidence: &DeletionRequestEvidence,
        canonical_event_sequence: u64,
        canonical_event_integrity_hash: &str,
    ) -> Result<DeletionJob, PrivacyError> {
        self.confirm_evidence(
            job_id,
            evidence,
            canonical_event_sequence,
            canonical_event_integrity_hash,
        )
        .await
    }

    async fn record_confirmed_deletion(
        &self,
        record: ConfirmedDeletionRecord<'_>,
    ) -> Result<DeletionJob, PrivacyError> {
        self.record_confirmed(
            record.job_id,
            record.subject_id,
            record.requested_by,
            record.retention_policy,
            record.evidence,
            record.canonical_event_sequence,
            record.canonical_event_integrity_hash,
        )
        .await
    }
}
