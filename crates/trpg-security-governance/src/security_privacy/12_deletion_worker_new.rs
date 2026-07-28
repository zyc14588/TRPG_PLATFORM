
impl DeletionWorker {
    pub fn new(
        repository: PostgresDeletionRepository,
        legal_holds: std::sync::Arc<dyn LegalHoldResolver>,
        surfaces: Vec<Box<dyn DeletionSurface>>,
    ) -> Result<Self, PrivacyError> {
        let mut by_target = HashMap::new();
        for surface in surfaces {
            let target = surface.target();
            if by_target.insert(target, surface).is_some() {
                return Err(PrivacyError::InvalidInput);
            }
        }
        Ok(Self {
            repository,
            legal_holds,
            surfaces: by_target,
        })
    }

    pub async fn execute_next(&self, limit: i64) -> Result<Vec<DeletionJob>, PrivacyError> {
        if !(1..=100).contains(&limit) {
            return Err(PrivacyError::InvalidInput);
        }
        self.repository.reclaim_expired_executions(limit).await?;
        let job_ids = sqlx::query_scalar::<_, String>(
            "SELECT job_id FROM privacy_deletion_jobs \
             WHERE evidence_status = 'confirmed' \
               AND (status IN ('requested', 'blocked_legal_hold') \
                    OR (status = 'failed' AND failure_code = $2 \
                        AND lease_recovery_count < $3)) \
             ORDER BY created_at, job_id LIMIT $1",
        )
        .bind(limit)
        .bind(DELETION_LEASE_EXPIRED_CODE)
        .bind(MAX_DELETION_LEASE_RECOVERIES)
        .fetch_all(self.repository.pool())
        .await
        .map_err(|_| PrivacyError::Database)?;
        let mut completed = Vec::with_capacity(job_ids.len());
        let mut first_error = None;
        for job_id in job_ids {
            match self.execute(&job_id).await {
                Ok(job) => completed.push(job),
                Err(PrivacyError::JobAlreadyRunning) => {}
                Err(error) => {
                    if first_error.is_none() {
                        first_error = Some(error);
                    }
                }
            }
        }
        match first_error {
            Some(error) => Err(error),
            None => Ok(completed),
        }
    }

    pub async fn execute(&self, job_id: &str) -> Result<DeletionJob, PrivacyError> {
        self.repository.reclaim_expired_job(job_id).await?;
        let job = self.repository.load(job_id).await?;
        let was_completed = job.status == DeletionJobStatus::Completed;
        if job.evidence_status != DeletionEvidenceStatus::Confirmed
            || job.canonical_event_sequence.is_none()
            || !job
                .canonical_event_integrity_hash
                .as_deref()
                .is_some_and(valid_integrity_hash)
        {
            return Err(PrivacyError::EvidenceUnconfirmed);
        }
        if self.repository.lease_recovery_exhausted(job_id).await? {
            return Err(PrivacyError::LeaseRecoveryExhausted);
        }
        if !was_completed && self.legal_holds.has_active_hold(&job.subject_id).await? {
            self.repository
                .set_status(job_id, DeletionJobStatus::BlockedLegalHold, None)
                .await?;
            return self.repository.load(job_id).await;
        }
        if !was_completed
            && !self
                .repository
                .claim_execution(job_id, &job.subject_id)
                .await?
        {
            return self.repository.load(job_id).await;
        }

        for target in REQUIRED_DELETION_TARGETS {
            if !was_completed {
                self.repository
                    .refresh_execution_lease(job_id, &job.subject_id)
                    .await?;
            }
            let current = self.repository.load(job_id).await?;
            let already_verified = current.targets.iter().any(|record| {
                record.target == target && record.status == DeletionTargetStatus::Verified
            });
            let surface = self
                .surfaces
                .get(&target)
                .ok_or(PrivacyError::MissingSurface(target));
            let surface = match surface {
                Ok(surface) => surface,
                Err(error) => {
                    if !was_completed && !already_verified {
                        let _cleanup_error = self.fail(job_id, target, error.code()).await.err();
                    }
                    return Err(error);
                }
            };
            let mut cursor = self
                .repository
                .target_progress_cursor(job_id, target)
                .await?;
            loop {
                if !was_completed {
                    self.repository
                        .refresh_execution_lease(job_id, &job.subject_id)
                        .await?;
                }
                let progress = match surface.delete_subject_batch(&job.subject_id, cursor).await {
                    Ok(progress) => progress,
                    Err(error) => {
                        if !was_completed && !already_verified {
                            let _cleanup_error =
                                self.fail(job_id, target, error.code()).await.err();
                        }
                        return Err(error);
                    }
                };
                if progress.next_cursor < cursor
                    || (!progress.complete && progress.next_cursor == cursor)
                {
                    let error = PrivacyError::InvalidPersistedState;
                    if !was_completed && !already_verified {
                        let _cleanup_error = self.fail(job_id, target, error.code()).await.err();
                    }
                    return Err(error);
                }
                if progress.next_cursor != cursor {
                    if was_completed {
                        return Err(PrivacyError::InvalidPersistedState);
                    }
                    self.repository
                        .advance_target_progress_cursor(
                            job_id,
                            target,
                            cursor,
                            progress.next_cursor,
                        )
                        .await?;
                    cursor = progress.next_cursor;
                }
                if progress.complete {
                    break;
                }
            }
            if !already_verified {
                self.repository
                    .refresh_execution_lease(job_id, &job.subject_id)
                    .await?;
                self.repository
                    .set_target_status(job_id, target, DeletionTargetStatus::Deleted, None)
                    .await?;
            }
            match surface.verify_absent(&job.subject_id).await {
                Ok(true) => {
                    if !already_verified {
                        self.repository
                            .refresh_execution_lease(job_id, &job.subject_id)
                            .await?;
                        self.repository
                            .set_target_status(job_id, target, DeletionTargetStatus::Verified, None)
                            .await?;
                    }
                }
                Ok(false) => {
                    let error = PrivacyError::VerificationFailed(target);
                    if !was_completed && !already_verified {
                        let _cleanup_error = self.fail(job_id, target, error.code()).await.err();
                    }
                    return Err(error);
                }
                Err(error) => {
                    if !was_completed && !already_verified {
                        let _cleanup_error = self.fail(job_id, target, error.code()).await.err();
                    }
                    return Err(error);
                }
            }
        }

        if was_completed {
            // Terminal rows are evidence, not authority. Re-read the durable
            // targets and return success only after every real surface has
            // just been deleted idempotently and independently proved absent.
            let reverified = self.repository.load(job_id).await?;
            return if reverified.all_targets_verified() {
                Ok(reverified)
            } else {
                Err(PrivacyError::InvalidPersistedState)
            };
        }

        self.repository
            .refresh_execution_lease(job_id, &job.subject_id)
            .await?;
        self.repository
            .set_execution_status(job_id, DeletionJobStatus::Verifying)
            .await?;
        let verified = self.repository.load(job_id).await?;
        if !verified.all_targets_verified() {
            self.repository
                .finish_execution(
                    job_id,
                    &job.subject_id,
                    DeletionJobStatus::Failed,
                    Some("DELETION_VERIFICATION_INCOMPLETE"),
                )
                .await?;
            return Err(PrivacyError::InvalidPersistedState);
        }
        self.repository
            .finish_execution(job_id, &job.subject_id, DeletionJobStatus::Completed, None)
            .await?;
        self.repository.load(job_id).await
    }

    async fn fail(
        &self,
        job_id: &str,
        target: DeletionTarget,
        code: &str,
    ) -> Result<(), PrivacyError> {
        let subject_id = self.repository.load(job_id).await?.subject_id;
        let refresh_error = self
            .repository
            .refresh_execution_lease(job_id, &subject_id)
            .await
            .err();
        let target_error = self
            .repository
            .set_target_status(job_id, target, DeletionTargetStatus::Failed, Some(code))
            .await
            .err();
        let finish_error = self
            .repository
            .finish_execution(job_id, &subject_id, DeletionJobStatus::Failed, Some(code))
            .await
            .err();
        refresh_error
            .or(target_error)
            .or(finish_error)
            .map_or(Ok(()), Err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_queue_message_is_classified_from_its_data_subject() {
        let payload = br#"{"data_subject_id":"victim_subject","payload":{"private":"value"}}"#;
        assert_eq!(
            retained_message_subject_digest(None, payload).unwrap(),
            format!("sha256:{}", sha256_hex(b"victim_subject"))
        );
    }

    #[test]
    fn queue_absence_proof_rejects_unclassified_or_mismatched_messages() {
        assert_eq!(
            retained_message_subject_digest(None, br#"{"payload":"unclassified"}"#),
            Err(PrivacyError::InvalidPersistedState)
        );
        assert_eq!(
            retained_message_subject_digest(
                Some("sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
                br#"{"data_subject_id":"victim_subject"}"#,
            ),
            Err(PrivacyError::InvalidPersistedState)
        );
        assert_eq!(
            retained_message_subject_digest(
                Some("sha256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"),
                br#"{"payload":"unclassified"}"#,
            ),
            Err(PrivacyError::InvalidPersistedState)
        );
    }

    #[test]
    fn service_credentials_require_tls_and_are_removed_from_nats_endpoint() {
        assert!(build_redis_client(
            "redis://localhost:6379",
            Some(b"ca"),
            Some(b"certificate"),
            Some(b"private-key"),
        )
        .is_err());
        let (endpoint, credentials) = nats_endpoint_and_credentials(
            "tls://runtime%20user:secret%40value@nats.example.invalid:4222",
        )
        .unwrap();
        assert_eq!(endpoint.as_str(), "tls://nats.example.invalid:4222");
        assert_eq!(
            credentials,
            Some(("runtime user".to_owned(), "secret@value".to_owned()))
        );
    }

    #[test]
    fn ipv6_loopback_is_the_only_cleartext_ipv6_service_host() {
        assert_eq!(
            validate_secure_service_url("http://[::1]:8080", "http", "https"),
            Ok(())
        );
        assert_eq!(
            validate_secure_service_url("http://[::2]:8080", "http", "https"),
            Err(PrivacyError::InvalidInput)
        );
    }
}
