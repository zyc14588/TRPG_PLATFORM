
impl PostgresDeletionRepository {

    async fn refresh_execution_lease(
        &self,
        job_id: &str,
        subject_id: &str,
    ) -> Result<(), PrivacyError> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|_| PrivacyError::Database)?;
        let job_updated = sqlx::query(
            "UPDATE privacy_deletion_jobs SET \
                 lease_expires_at = statement_timestamp() + make_interval(secs => $3), \
                 updated_at = statement_timestamp() \
             WHERE job_id = $1 AND subject_id = $2 \
               AND status IN ('running', 'verifying') \
               AND lease_expires_at > statement_timestamp()",
        )
        .bind(job_id)
        .bind(subject_id)
        .bind(DELETION_EXECUTION_LEASE_SECONDS)
        .execute(&mut *transaction)
        .await
        .map_err(|_| PrivacyError::Database)?
        .rows_affected();
        if job_updated != 1 {
            return Err(PrivacyError::ExecutionLeaseExpired);
        }
        let fence_updated = sqlx::query(
            "UPDATE privacy_subject_deletion_fences SET \
                 lease_expires_at = statement_timestamp() + make_interval(secs => $3), \
                 updated_at = statement_timestamp() \
             WHERE subject_id = $2 AND job_id = $1 AND status = 'running' \
               AND lease_expires_at > statement_timestamp()",
        )
        .bind(job_id)
        .bind(subject_id)
        .bind(DELETION_EXECUTION_LEASE_SECONDS)
        .execute(&mut *transaction)
        .await
        .map_err(|_| PrivacyError::Database)?
        .rows_affected();
        if fence_updated != 1 {
            return Err(PrivacyError::ExecutionLeaseExpired);
        }
        transaction
            .commit()
            .await
            .map_err(|_| PrivacyError::Database)
    }

    async fn reclaim_expired_job(&self, job_id: &str) -> Result<bool, PrivacyError> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|_| PrivacyError::Database)?;
        let row = sqlx::query(
            "SELECT subject_id, status, \
                    COALESCE(lease_expires_at <= statement_timestamp(), false) AS expired \
               FROM privacy_deletion_jobs WHERE job_id = $1 FOR UPDATE",
        )
        .bind(job_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|_| PrivacyError::Database)?
        .ok_or(PrivacyError::JobNotFound)?;
        let status: String = row.get("status");
        let expired: bool = row.get("expired");
        if !matches!(status.as_str(), "running" | "verifying") || !expired {
            transaction
                .commit()
                .await
                .map_err(|_| PrivacyError::Database)?;
            return Ok(false);
        }
        let subject_id: String = row.get("subject_id");
        sqlx::query(
            "UPDATE privacy_subject_deletion_fences SET status = 'failed', \
                    lease_expires_at = NULL, updated_at = statement_timestamp() \
             WHERE subject_id = $1 AND job_id = $2 AND status = 'running'",
        )
        .bind(&subject_id)
        .bind(job_id)
        .execute(&mut *transaction)
        .await
        .map_err(|_| PrivacyError::Database)?;
        let affected = sqlx::query(
            "UPDATE privacy_deletion_jobs SET status = 'failed', failure_code = $2, \
                    lease_expires_at = NULL, \
                    lease_recovery_count = lease_recovery_count + 1, \
                    last_lease_expired_at = statement_timestamp(), \
                    updated_at = statement_timestamp() \
             WHERE job_id = $1 AND status IN ('running', 'verifying') \
               AND lease_expires_at <= statement_timestamp() \
               AND lease_recovery_count < $3",
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
        transaction
            .commit()
            .await
            .map_err(|_| PrivacyError::Database)?;
        Ok(true)
    }

    async fn reclaim_expired_executions(&self, limit: i64) -> Result<u64, PrivacyError> {
        let job_ids = sqlx::query_scalar::<_, String>(
            "SELECT job_id FROM privacy_deletion_jobs \
             WHERE status IN ('running', 'verifying') \
               AND lease_expires_at <= statement_timestamp() \
             ORDER BY lease_expires_at, job_id LIMIT $1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|_| PrivacyError::Database)?;
        let mut reclaimed = 0_u64;
        for job_id in job_ids {
            reclaimed += u64::from(self.reclaim_expired_job(&job_id).await?);
        }
        Ok(reclaimed)
    }

    pub async fn lease_recovery_total(&self) -> Result<u64, PrivacyError> {
        let total: i64 = sqlx::query_scalar(
            "SELECT COALESCE(sum(lease_recovery_count), 0)::bigint \
             FROM privacy_deletion_jobs",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|_| PrivacyError::Database)?;
        u64::try_from(total).map_err(|_| PrivacyError::InvalidPersistedState)
    }

    async fn lease_recovery_exhausted(&self, job_id: &str) -> Result<bool, PrivacyError> {
        sqlx::query_scalar(
            "SELECT status = 'failed' AND failure_code = $2 \
                    AND lease_recovery_count >= $3 \
             FROM privacy_deletion_jobs WHERE job_id = $1",
        )
        .bind(job_id)
        .bind(DELETION_LEASE_EXPIRED_CODE)
        .bind(MAX_DELETION_LEASE_RECOVERIES)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| PrivacyError::Database)?
        .ok_or(PrivacyError::JobNotFound)
    }

    async fn set_target_status(
        &self,
        job_id: &str,
        target: DeletionTarget,
        status: DeletionTargetStatus,
        error_code: Option<&str>,
    ) -> Result<(), PrivacyError> {
        let affected = sqlx::query(
            "UPDATE privacy_deletion_job_targets SET status = $3, error_code = $4, \
             deleted_at = CASE WHEN $3 IN ('deleted', 'verified') THEN now() ELSE deleted_at END, \
             verified_at = CASE WHEN $3 = 'verified' THEN now() ELSE verified_at END \
             WHERE job_id = $1 AND target = $2 \
               AND EXISTS (\
                   SELECT 1 FROM privacy_deletion_jobs AS job \
                   JOIN privacy_subject_deletion_fences AS fence \
                     ON fence.job_id = job.job_id AND fence.subject_id = job.subject_id \
                  WHERE job.job_id = $1 AND job.status IN ('running', 'verifying') \
                    AND job.lease_expires_at > statement_timestamp() \
                    AND fence.status = 'running' \
                    AND fence.lease_expires_at > statement_timestamp()\
               )",
        )
        .bind(job_id)
        .bind(target.as_str())
        .bind(status.as_str())
        .bind(error_code)
        .execute(&self.pool)
        .await
        .map_err(|_| PrivacyError::Database)?
        .rows_affected();
        if affected == 1 {
            Ok(())
        } else {
            Err(PrivacyError::InvalidPersistedState)
        }
    }

    async fn target_progress_cursor(
        &self,
        job_id: &str,
        target: DeletionTarget,
    ) -> Result<u64, PrivacyError> {
        let cursor: i64 = sqlx::query_scalar(
            "SELECT progress_cursor FROM privacy_deletion_job_targets \
             WHERE job_id = $1 AND target = $2",
        )
        .bind(job_id)
        .bind(target.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| PrivacyError::Database)?
        .ok_or(PrivacyError::InvalidPersistedState)?;
        u64::try_from(cursor)
            .ok()
            .filter(|cursor| *cursor > 0)
            .ok_or(PrivacyError::InvalidPersistedState)
    }

    async fn advance_target_progress_cursor(
        &self,
        job_id: &str,
        target: DeletionTarget,
        expected_cursor: u64,
        next_cursor: u64,
    ) -> Result<(), PrivacyError> {
        if expected_cursor == 0 || next_cursor <= expected_cursor {
            return Err(PrivacyError::InvalidPersistedState);
        }
        let expected_cursor =
            i64::try_from(expected_cursor).map_err(|_| PrivacyError::InvalidPersistedState)?;
        let next_cursor =
            i64::try_from(next_cursor).map_err(|_| PrivacyError::InvalidPersistedState)?;
        let affected = sqlx::query(
            "UPDATE privacy_deletion_job_targets AS target_row \
                SET progress_cursor = $4 \
              WHERE target_row.job_id = $1 AND target_row.target = $2 \
                AND target_row.status = 'pending' \
                AND target_row.progress_cursor = $3 \
                AND EXISTS (\
                    SELECT 1 FROM privacy_deletion_jobs AS job \
                    JOIN privacy_subject_deletion_fences AS fence \
                      ON fence.job_id = job.job_id AND fence.subject_id = job.subject_id \
                    WHERE job.job_id = $1 AND job.status = 'running' \
                      AND job.lease_expires_at > statement_timestamp() \
                      AND fence.status = 'running' \
                      AND fence.lease_expires_at > statement_timestamp()\
                )",
        )
        .bind(job_id)
        .bind(target.as_str())
        .bind(expected_cursor)
        .bind(next_cursor)
        .execute(&self.pool)
        .await
        .map_err(|_| PrivacyError::Database)?
        .rows_affected();
        if affected == 1 {
            Ok(())
        } else {
            Err(PrivacyError::InvalidPersistedState)
        }
    }
}
