
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
        if job.status == DeletionJobStatus::Completed {
            return self.revalidate_completed(&job).await;
        }
        if self.legal_holds.has_active_hold(&job.subject_id).await? {
            self.repository
                .set_status(job_id, DeletionJobStatus::BlockedLegalHold, None)
                .await?;
            return self.repository.load(job_id).await;
        }
        let claim_token = match self
            .repository
            .claim_execution(job_id, &job.subject_id)
            .await?
        {
            Some(claim_token) => claim_token,
            None => return self.repository.load(job_id).await,
        };
        let context =
            DeletionExecutionContext::new(job_id, &job.subject_id, claim_token);

        for target in REQUIRED_DELETION_TARGETS {
            self.repository
                .refresh_execution_lease(
                    job_id,
                    &job.subject_id,
                    context.claim_token(),
                )
                .await?;
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
                    if !already_verified {
                        let _cleanup_error = self
                            .fail(
                                job_id,
                                target,
                                error.code(),
                                context.claim_token(),
                            )
                            .await
                            .err();
                    }
                    return Err(error);
                }
            };
            let mut cursor = self
                .repository
                .target_progress_cursor(job_id, target)
                .await?;
            loop {
                self.repository
                    .refresh_execution_lease(
                        job_id,
                        &job.subject_id,
                        context.claim_token(),
                    )
                    .await?;
                let progress = match surface.delete_subject_batch(&context, cursor).await {
                    Ok(progress) => progress,
                    Err(error) => {
                        if !already_verified {
                            let _cleanup_error = self
                                .fail(
                                    job_id,
                                    target,
                                    error.code(),
                                    context.claim_token(),
                                )
                                .await
                                .err();
                        }
                        return Err(error);
                    }
                };
                if progress.next_cursor < cursor
                    || (!progress.complete && progress.next_cursor == cursor)
                {
                    let error = PrivacyError::InvalidPersistedState;
                    if !already_verified {
                        let _cleanup_error = self
                            .fail(
                                job_id,
                                target,
                                error.code(),
                                context.claim_token(),
                            )
                            .await
                            .err();
                    }
                    return Err(error);
                }
                if progress.next_cursor != cursor {
                    self.repository
                        .advance_target_progress_cursor(
                            job_id,
                            target,
                            cursor,
                            progress.next_cursor,
                            context.claim_token(),
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
                    .refresh_execution_lease(
                        job_id,
                        &job.subject_id,
                        context.claim_token(),
                    )
                    .await?;
                self.repository
                    .set_target_status(
                        job_id,
                        target,
                        DeletionTargetStatus::Deleted,
                        None,
                        context.claim_token(),
                    )
                    .await?;
            }
            match surface.verify_absent(&job.subject_id).await {
                Ok(true) => {
                    if !already_verified {
                        self.repository
                            .refresh_execution_lease(
                                job_id,
                                &job.subject_id,
                                context.claim_token(),
                            )
                            .await?;
                        self.repository
                            .set_target_status(
                                job_id,
                                target,
                                DeletionTargetStatus::Verified,
                                None,
                                context.claim_token(),
                            )
                            .await?;
                    }
                }
                Ok(false) => {
                    let error = PrivacyError::VerificationFailed(target);
                    if !already_verified {
                        let _cleanup_error = self
                            .fail(
                                job_id,
                                target,
                                error.code(),
                                context.claim_token(),
                            )
                            .await
                            .err();
                    }
                    return Err(error);
                }
                Err(error) => {
                    if !already_verified {
                        let _cleanup_error = self
                            .fail(
                                job_id,
                                target,
                                error.code(),
                                context.claim_token(),
                            )
                            .await
                            .err();
                    }
                    return Err(error);
                }
            }
        }

        self.repository
            .refresh_execution_lease(
                job_id,
                &job.subject_id,
                context.claim_token(),
            )
            .await?;
        self.repository
            .set_execution_status(
                job_id,
                DeletionJobStatus::Verifying,
                context.claim_token(),
            )
            .await?;
        let verified = self.repository.load(job_id).await?;
        if !verified.all_targets_verified() {
            self.repository
                .finish_execution(
                    job_id,
                    &job.subject_id,
                    DeletionJobStatus::Failed,
                    Some("DELETION_VERIFICATION_INCOMPLETE"),
                    context.claim_token(),
                )
                .await?;
            return Err(PrivacyError::InvalidPersistedState);
        }
        self.repository
            .finish_execution(
                job_id,
                &job.subject_id,
                DeletionJobStatus::Completed,
                None,
                context.claim_token(),
            )
            .await?;
        self.repository.load(job_id).await
    }

}
