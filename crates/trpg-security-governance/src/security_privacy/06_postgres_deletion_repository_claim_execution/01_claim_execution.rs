
impl PostgresDeletionRepository {

    async fn claim_execution(
        &self,
        job_id: &str,
        subject_id: &str,
    ) -> Result<Option<String>, PrivacyError> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|_| PrivacyError::Database)?;
        sqlx::query(
            "SELECT pg_advisory_xact_lock(hashtextextended('privacy_subject_delete:' || $1, 0))",
        )
        .bind(subject_id)
        .execute(&mut *transaction)
        .await
        .map_err(|_| PrivacyError::Database)?;
        let row = sqlx::query(
            "SELECT subject_id, status, failure_code, evidence_status, lease_recovery_count, \
                    COALESCE(lease_expires_at > statement_timestamp(), false) AS lease_active \
             FROM privacy_deletion_jobs \
             WHERE job_id = $1 FOR UPDATE",
        )
        .bind(job_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|_| PrivacyError::Database)?
        .ok_or(PrivacyError::JobNotFound)?;
        let persisted_subject: String = row.get("subject_id");
        let mut status: String = row.get("status");
        let mut failure_code: Option<String> = row.get("failure_code");
        let mut lease_recovery_count: i64 = row.get("lease_recovery_count");
        let evidence_status: String = row.get("evidence_status");
        if persisted_subject != subject_id || evidence_status != "confirmed" {
            return Err(PrivacyError::EvidenceUnconfirmed);
        }
        if status == "running" || status == "verifying" {
            if row.get::<bool, _>("lease_active") {
                return Err(PrivacyError::JobAlreadyRunning);
            }
            sqlx::query(
                "UPDATE privacy_subject_deletion_fences SET status = 'failed', \
                        lease_expires_at = NULL, execution_claim_token = NULL, \
                        updated_at = statement_timestamp() \
                 WHERE subject_id = $1 AND job_id = $2 AND status = 'running'",
            )
            .bind(subject_id)
            .bind(job_id)
            .execute(&mut *transaction)
            .await
            .map_err(|_| PrivacyError::Database)?;
            let affected = sqlx::query(
                "UPDATE privacy_deletion_jobs SET status = 'failed', failure_code = $2, \
                        lease_expires_at = NULL, execution_claim_token = NULL, \
                        lease_recovery_count = lease_recovery_count + 1, \
                        last_lease_expired_at = statement_timestamp(), \
                        updated_at = statement_timestamp() \
                 WHERE job_id = $1 AND lease_recovery_count < $3",
            )
            .bind(job_id)
            .bind(DELETION_LEASE_EXPIRED_CODE)
            .bind(MAX_DELETION_LEASE_RECOVERIES)
            .execute(&mut *transaction)
            .await
            .map_err(|_| PrivacyError::Database)?
            .rows_affected();
            if affected != 1 {
                return Err(PrivacyError::LeaseRecoveryExhausted);
            }
            status = "failed".to_owned();
            failure_code = Some(DELETION_LEASE_EXPIRED_CODE.to_owned());
            lease_recovery_count += 1;
        }
        if status == "completed" {
            return Ok(None);
        }
        if status == "failed" && failure_code.as_deref() != Some(DELETION_LEASE_EXPIRED_CODE) {
            return Err(PrivacyError::InvalidPersistedState);
        }
        if status == "failed"
            && failure_code.as_deref() == Some(DELETION_LEASE_EXPIRED_CODE)
            && lease_recovery_count >= MAX_DELETION_LEASE_RECOVERIES
        {
            return Err(PrivacyError::LeaseRecoveryExhausted);
        }
        let held: bool = sqlx::query_scalar(
            "SELECT COALESCE((SELECT active FROM privacy_legal_holds \
                              WHERE subject_id = $1), false)",
        )
        .bind(subject_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(|_| PrivacyError::Database)?;
        if held {
            sqlx::query(
                "UPDATE privacy_deletion_jobs SET status = 'blocked_legal_hold', \
                 failure_code = NULL, lease_expires_at = NULL, \
                 updated_at = statement_timestamp() WHERE job_id = $1",
            )
            .bind(job_id)
            .execute(&mut *transaction)
            .await
            .map_err(|_| PrivacyError::Database)?;
            transaction
                .commit()
                .await
                .map_err(|_| PrivacyError::Database)?;
            return Ok(None);
        }
        let claim_token: String = sqlx::query_scalar("SELECT gen_random_uuid()::text")
            .fetch_one(&mut *transaction)
            .await
            .map_err(|_| PrivacyError::Database)?;
        let existing_fence = sqlx::query(
            "SELECT job_id, status FROM privacy_subject_deletion_fences \
             WHERE subject_id = $1 FOR UPDATE",
        )
        .bind(subject_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|_| PrivacyError::Database)?;
        if let Some(fence) = existing_fence {
            let fenced_job: String = fence.get("job_id");
            let fenced_status: String = fence.get("status");
            if fenced_job != job_id || fenced_status == "completed" {
                return Err(PrivacyError::DeletionInProgress);
            }
            sqlx::query(
                "UPDATE privacy_subject_deletion_fences SET status = 'running', \
                 execution_claim_token = $3, \
                 lease_expires_at = statement_timestamp() + make_interval(secs => $2), \
                 updated_at = statement_timestamp() WHERE subject_id = $1",
            )
            .bind(subject_id)
            .bind(DELETION_EXECUTION_LEASE_SECONDS)
            .bind(&claim_token)
            .execute(&mut *transaction)
            .await
            .map_err(|_| PrivacyError::Database)?;
        } else {
            sqlx::query(
                "INSERT INTO privacy_subject_deletion_fences \
                 (subject_id, job_id, status, lease_expires_at, execution_claim_token) \
                 VALUES ($1, $2, 'running', \
                         statement_timestamp() + make_interval(secs => $3), $4)",
            )
            .bind(subject_id)
            .bind(job_id)
            .bind(DELETION_EXECUTION_LEASE_SECONDS)
            .bind(&claim_token)
            .execute(&mut *transaction)
            .await
            .map_err(|_| PrivacyError::Database)?;
        }
        let affected = sqlx::query(
            "UPDATE privacy_deletion_jobs SET status = 'running', failure_code = NULL, \
             execution_claim_token = $3, \
             lease_expires_at = statement_timestamp() + make_interval(secs => $2), \
             updated_at = statement_timestamp() WHERE job_id = $1",
        )
        .bind(job_id)
        .bind(DELETION_EXECUTION_LEASE_SECONDS)
        .bind(&claim_token)
        .execute(&mut *transaction)
        .await
        .map_err(|_| PrivacyError::Database)?
        .rows_affected();
        if affected != 1 {
            return Err(PrivacyError::InvalidPersistedState);
        }
        transaction
            .commit()
            .await
            .map_err(|_| PrivacyError::Database)?;
        Ok(Some(claim_token))
    }

    async fn finish_execution(
        &self,
        job_id: &str,
        subject_id: &str,
        status: DeletionJobStatus,
        failure_code: Option<&str>,
        claim_token: &str,
    ) -> Result<(), PrivacyError> {
        let fence_status = match status {
            DeletionJobStatus::Completed => "completed",
            DeletionJobStatus::Failed => "failed",
            _ => return Err(PrivacyError::InvalidInput),
        };
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|_| PrivacyError::Database)?;
        let affected = sqlx::query(
            "UPDATE privacy_subject_deletion_fences SET status = $3, \
                    lease_expires_at = NULL, execution_claim_token = NULL, \
                    updated_at = statement_timestamp() \
             WHERE subject_id = $1 AND job_id = $2 AND status = 'running' \
               AND execution_claim_token = $4 \
               AND lease_expires_at > statement_timestamp()",
        )
        .bind(subject_id)
        .bind(job_id)
        .bind(fence_status)
        .bind(claim_token)
        .execute(&mut *transaction)
        .await
        .map_err(|_| PrivacyError::Database)?
        .rows_affected();
        if affected != 1 {
            return Err(PrivacyError::ExecutionLeaseExpired);
        }
        let affected = sqlx::query(
            "UPDATE privacy_deletion_jobs SET status = $2, failure_code = $3, \
                    lease_expires_at = NULL, execution_claim_token = NULL, \
                    updated_at = statement_timestamp() \
             WHERE job_id = $1 AND status IN ('running', 'verifying') \
               AND execution_claim_token = $4 \
               AND lease_expires_at > statement_timestamp()",
        )
        .bind(job_id)
        .bind(status.as_str())
        .bind(failure_code)
        .bind(claim_token)
        .execute(&mut *transaction)
        .await
        .map_err(|_| PrivacyError::Database)?
        .rows_affected();
        if affected != 1 {
            return Err(PrivacyError::ExecutionLeaseExpired);
        }
        transaction
            .commit()
            .await
            .map_err(|_| PrivacyError::Database)
    }

    async fn begin_revalidation(
        &self,
        job_id: &str,
        subject_id: &str,
    ) -> Result<DeletionRevalidationClaim, PrivacyError> {
        let row = sqlx::query(
            "SELECT run_id, claim_token \
               FROM public.begin_privacy_deletion_revalidation($1, $2)",
        )
        .bind(job_id)
        .bind(subject_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|_| PrivacyError::Database)?;
        Ok(DeletionRevalidationClaim {
            run_id: row.try_get("run_id").map_err(db_error)?,
            claim_token: row.try_get("claim_token").map_err(db_error)?,
        })
    }

    #[allow(clippy::too_many_arguments)]
    async fn record_revalidation_result(
        &self,
        claim: &DeletionRevalidationClaim,
        job_id: &str,
        subject_id: &str,
        result_status: &str,
        failure_target: Option<DeletionTarget>,
        error_code: Option<&str>,
        evidence_hash: &str,
    ) -> Result<(), PrivacyError> {
        sqlx::query(
            "SELECT public.record_privacy_deletion_revalidation_result(\
                $1, $2, $3, $4, $5, $6, $7, $8\
             )",
        )
        .bind(&claim.run_id)
        .bind(job_id)
        .bind(subject_id)
        .bind(&claim.claim_token)
        .bind(result_status)
        .bind(failure_target.map(DeletionTarget::as_str))
        .bind(error_code)
        .bind(evidence_hash)
        .execute(&self.pool)
        .await
        .map_err(|_| PrivacyError::Database)?;
        Ok(())
    }
}
