
impl DeletionWorker {
    async fn revalidate_completed(
        &self,
        job: &DeletionJob,
    ) -> Result<DeletionJob, PrivacyError> {
        let claim = self
            .repository
            .begin_revalidation(&job.job_id, &job.subject_id)
            .await?;
        for target in REQUIRED_DELETION_TARGETS {
            let surface = match self.surfaces.get(&target) {
                Some(surface) => surface,
                None => {
                    let error = PrivacyError::MissingSurface(target);
                    self.record_revalidation_failure(
                        job,
                        &claim,
                        target,
                        error.code(),
                    )
                    .await?;
                    return Err(error);
                }
            };
            match surface.verify_absent(&job.subject_id).await {
                Ok(true) => {}
                Ok(false) => {
                    let error = PrivacyError::VerificationFailed(target);
                    self.record_revalidation_failure(
                        job,
                        &claim,
                        target,
                        error.code(),
                    )
                    .await?;
                    return Err(error);
                }
                Err(error) => {
                    self.record_revalidation_failure(
                        job,
                        &claim,
                        target,
                        error.code(),
                    )
                    .await?;
                    return Err(error);
                }
            }
        }
        let evidence_hash =
            self.revalidation_evidence_hash(job, &claim, "passed", None, None);
        self.repository
            .record_revalidation_result(
                &claim,
                &job.job_id,
                &job.subject_id,
                "passed",
                None,
                None,
                &evidence_hash,
            )
            .await?;
        self.repository.load(&job.job_id).await
    }

    async fn record_revalidation_failure(
        &self,
        job: &DeletionJob,
        claim: &DeletionRevalidationClaim,
        target: DeletionTarget,
        error_code: &str,
    ) -> Result<(), PrivacyError> {
        let evidence_hash = self.revalidation_evidence_hash(
            job,
            claim,
            "failed",
            Some(target),
            Some(error_code),
        );
        self.repository
            .record_revalidation_result(
                claim,
                &job.job_id,
                &job.subject_id,
                "failed",
                Some(target),
                Some(error_code),
                &evidence_hash,
            )
            .await
    }

    fn revalidation_evidence_hash(
        &self,
        job: &DeletionJob,
        claim: &DeletionRevalidationClaim,
        result_status: &str,
        target: Option<DeletionTarget>,
        error_code: Option<&str>,
    ) -> String {
        let material = format!(
            "{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}",
            claim.run_id,
            job.job_id,
            job.subject_id,
            job.canonical_event_integrity_hash.as_deref().unwrap_or(""),
            result_status,
            target
                .map(DeletionTarget::as_str)
                .unwrap_or("all_required_targets"),
        ) + "\u{1f}"
            + error_code.unwrap_or("none");
        format!("sha256:{}", sha256_hex(material.as_bytes()))
    }

    async fn fail(
        &self,
        job_id: &str,
        target: DeletionTarget,
        code: &str,
        claim_token: &str,
    ) -> Result<(), PrivacyError> {
        let subject_id = self.repository.load(job_id).await?.subject_id;
        let refresh_error = self
            .repository
            .refresh_execution_lease(job_id, &subject_id, claim_token)
            .await
            .err();
        let target_error = self
            .repository
            .set_target_status(
                job_id,
                target,
                DeletionTargetStatus::Failed,
                Some(code),
                claim_token,
            )
            .await
            .err();
        let finish_error = self
            .repository
            .finish_execution(
                job_id,
                &subject_id,
                DeletionJobStatus::Failed,
                Some(code),
                claim_token,
            )
            .await
            .err();
        refresh_error
            .or(target_error)
            .or(finish_error)
            .map_or(Ok(()), Err)
    }
}
