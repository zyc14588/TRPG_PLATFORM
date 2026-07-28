
impl PostgresDeletionRepository {

    async fn claim_execution(&self, job_id: &str, subject_id: &str) -> Result<bool, PrivacyError> {
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
                        lease_expires_at = NULL, updated_at = statement_timestamp() \
                 WHERE subject_id = $1 AND job_id = $2 AND status = 'running'",
            )
            .bind(subject_id)
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
            return Ok(false);
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
            return Ok(false);
        }
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
                 lease_expires_at = statement_timestamp() + make_interval(secs => $2), \
                 updated_at = statement_timestamp() WHERE subject_id = $1",
            )
            .bind(subject_id)
            .bind(DELETION_EXECUTION_LEASE_SECONDS)
            .execute(&mut *transaction)
            .await
            .map_err(|_| PrivacyError::Database)?;
        } else {
            sqlx::query(
                "INSERT INTO privacy_subject_deletion_fences \
                 (subject_id, job_id, status, lease_expires_at) \
                 VALUES ($1, $2, 'running', \
                         statement_timestamp() + make_interval(secs => $3))",
            )
            .bind(subject_id)
            .bind(job_id)
            .bind(DELETION_EXECUTION_LEASE_SECONDS)
            .execute(&mut *transaction)
            .await
            .map_err(|_| PrivacyError::Database)?;
        }
        let affected = sqlx::query(
            "UPDATE privacy_deletion_jobs SET status = 'running', failure_code = NULL, \
             lease_expires_at = statement_timestamp() + make_interval(secs => $2), \
             updated_at = statement_timestamp() WHERE job_id = $1",
        )
        .bind(job_id)
        .bind(DELETION_EXECUTION_LEASE_SECONDS)
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
        Ok(true)
    }

    async fn finish_execution(
        &self,
        job_id: &str,
        subject_id: &str,
        status: DeletionJobStatus,
        failure_code: Option<&str>,
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
                    lease_expires_at = NULL, updated_at = statement_timestamp() \
             WHERE subject_id = $1 AND job_id = $2 AND status = 'running' \
               AND lease_expires_at > statement_timestamp()",
        )
        .bind(subject_id)
        .bind(job_id)
        .bind(fence_status)
        .execute(&mut *transaction)
        .await
        .map_err(|_| PrivacyError::Database)?
        .rows_affected();
        if affected != 1 {
            return Err(PrivacyError::ExecutionLeaseExpired);
        }
        let affected = sqlx::query(
            "UPDATE privacy_deletion_jobs SET status = $2, failure_code = $3, \
                    lease_expires_at = NULL, updated_at = statement_timestamp() \
             WHERE job_id = $1 AND status IN ('running', 'verifying') \
               AND lease_expires_at > statement_timestamp()",
        )
        .bind(job_id)
        .bind(status.as_str())
        .bind(failure_code)
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
}

fn validate_id(value: &str) -> Result<(), PrivacyError> {
    EntityId::new(value)
        .map(|_| ())
        .map_err(|_| PrivacyError::InvalidInput)
}

fn valid_integrity_hash(value: &str) -> bool {
    const PREFIX: &str = "hmac-sha256:";
    value.len() == PREFIX.len() + 64
        && value.starts_with(PREFIX)
        && value[PREFIX.len()..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn db_error(_: sqlx::Error) -> PrivacyError {
    PrivacyError::Database
}

fn deletion_evidence_write_error(error: sqlx::Error) -> PrivacyError {
    match error.as_database_error() {
        Some(database_error)
            if database_error.code().as_deref() == Some("P0001")
                && database_error.message()
                    == "deletion evidence does not match the canonical request event" =>
        {
            PrivacyError::DeletionEvidenceMismatch
        }
        _ => PrivacyError::Database,
    }
}

#[async_trait]
pub trait LegalHoldResolver: Send + Sync {
    async fn has_active_hold(&self, subject_id: &str) -> Result<bool, PrivacyError>;
}

#[derive(Clone)]
pub struct PostgresLegalHoldResolver {
    pool: PgPool,
}

impl PostgresLegalHoldResolver {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn set_hold(
        &self,
        subject_id: &str,
        hold_reference: &str,
        active: bool,
    ) -> Result<(), PrivacyError> {
        validate_id(subject_id)?;
        validate_id(hold_reference)?;
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
        if active {
            let deletion_running: bool = sqlx::query_scalar(
                "SELECT EXISTS (SELECT 1 FROM privacy_subject_deletion_fences \
                                WHERE subject_id = $1 AND status = 'running' \
                                  AND lease_expires_at > statement_timestamp())",
            )
            .bind(subject_id)
            .fetch_one(&mut *transaction)
            .await
            .map_err(|_| PrivacyError::Database)?;
            if deletion_running {
                return Err(PrivacyError::DeletionInProgress);
            }
        }
        sqlx::query(
            "INSERT INTO privacy_legal_holds (subject_id, hold_reference, active) \
             VALUES ($1, $2, $3) ON CONFLICT (subject_id) DO UPDATE SET \
             hold_reference = EXCLUDED.hold_reference, active = EXCLUDED.active, updated_at = now()",
        )
        .bind(subject_id)
        .bind(hold_reference)
        .bind(active)
        .execute(&mut *transaction)
        .await
        .map_err(|_| PrivacyError::Database)?;
        transaction
            .commit()
            .await
            .map_err(|_| PrivacyError::Database)
    }
}

#[async_trait]
impl LegalHoldResolver for PostgresLegalHoldResolver {
    async fn has_active_hold(&self, subject_id: &str) -> Result<bool, PrivacyError> {
        validate_id(subject_id)?;
        sqlx::query_scalar::<_, bool>(
            "SELECT active FROM privacy_legal_holds WHERE subject_id = $1",
        )
        .bind(subject_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| PrivacyError::Database)
        .map(|active| active.unwrap_or(false))
    }
}

#[async_trait]
pub trait DeletionSurface: Send + Sync {
    fn target(&self) -> DeletionTarget;
    async fn delete_subject_batch(
        &self,
        subject_id: &str,
        cursor: u64,
    ) -> Result<DeletionBatchProgress, PrivacyError>;
    async fn verify_absent(&self, subject_id: &str) -> Result<bool, PrivacyError>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeletionBatchProgress {
    pub next_cursor: u64,
    pub complete: bool,
}
