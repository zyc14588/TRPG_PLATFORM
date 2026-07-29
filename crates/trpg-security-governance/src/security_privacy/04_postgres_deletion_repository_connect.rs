
impl PostgresDeletionRepository {
    pub async fn connect(database_url: &str) -> Result<Self, PrivacyError> {
        let options =
            PgConnectOptions::from_str(database_url).map_err(|_| PrivacyError::InvalidInput)?;
        let host = options.get_host();
        let local = matches!(host, "localhost" | "127.0.0.1" | "::1") || host.starts_with('/');
        if !local && !matches!(options.get_ssl_mode(), PgSslMode::VerifyFull) {
            return Err(PrivacyError::InvalidInput);
        }
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect_with(options)
            .await
            .map_err(|_| PrivacyError::Database)?;
        Ok(Self::new(pool))
    }

    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn migrate(&self) -> Result<(), PrivacyError> {
        MIGRATOR
            .run(&self.pool)
            .await
            .map_err(|_| PrivacyError::Database)?;
        Ok(())
    }

    pub async fn check_readiness(&self) -> Result<(), PrivacyError> {
        let ready: bool = sqlx::query_scalar(
            "SELECT to_regclass('public.privacy_deletion_jobs') IS NOT NULL \
                    AND to_regclass('public.privacy_deletion_job_targets') IS NOT NULL \
                    AND to_regclass('public.privacy_subject_deletion_fences') IS NOT NULL \
                    AND to_regclass('public.privacy_deletion_revalidation_runs') IS NOT NULL \
                    AND to_regclass('public.privacy_deletion_revalidation_results') IS NOT NULL \
                    AND to_regprocedure('public.enforce_privacy_deletion_job_evidence()') \
                        IS NOT NULL \
                    AND to_regprocedure(\
                        'public.erase_privacy_database_subject(text,text,text)'\
                    ) IS NOT NULL \
                    AND to_regprocedure(\
                        'public.erase_privacy_rag_subject(text,text,text)'\
                    ) IS NOT NULL \
                    AND to_regprocedure(\
                        'public.begin_privacy_deletion_revalidation(text,text)'\
                    ) IS NOT NULL \
                    AND to_regprocedure(\
                        'public.record_privacy_deletion_revalidation_result(\
                            text,text,text,text,text,text,text,text\
                        )'\
                    ) IS NOT NULL",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|_| PrivacyError::Database)?;
        if ready {
            Ok(())
        } else {
            Err(PrivacyError::InvalidPersistedState)
        }
    }

    pub async fn request(
        &self,
        job_id: &str,
        subject_id: &str,
        requested_by: &str,
        retention_policy: &str,
        evidence: &DeletionRequestEvidence,
    ) -> Result<DeletionJob, PrivacyError> {
        validate_id(job_id)?;
        validate_id(subject_id)?;
        validate_id(requested_by)?;
        if retention_policy.trim().is_empty()
            || retention_policy.len() > 128
            || evidence.event_type()
                != "platform.security_privacy_copyright.data_deletion_requested"
        {
            return Err(PrivacyError::InvalidInput);
        }
        // A side-table job must never precede its canonical request event.
        // Production callers commit first and then call `record_confirmed`.
        Err(PrivacyError::LegacyTwoPhaseDisabled)
    }

    pub async fn confirm_evidence(
        &self,
        job_id: &str,
        evidence: &DeletionRequestEvidence,
        canonical_event_sequence: u64,
        canonical_event_integrity_hash: &str,
    ) -> Result<DeletionJob, PrivacyError> {
        validate_id(job_id)?;
        if canonical_event_sequence == 0 || !valid_integrity_hash(canonical_event_integrity_hash) {
            return Err(PrivacyError::InvalidInput);
        }
        let _ = evidence;
        Err(PrivacyError::LegacyTwoPhaseDisabled)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn record_confirmed(
        &self,
        job_id: &str,
        subject_id: &str,
        requested_by: &str,
        retention_policy: &str,
        evidence: &DeletionRequestEvidence,
        canonical_event_sequence: u64,
        canonical_event_integrity_hash: &str,
    ) -> Result<DeletionJob, PrivacyError> {
        validate_id(job_id)?;
        validate_id(subject_id)?;
        validate_id(requested_by)?;
        if retention_policy.trim().is_empty()
            || retention_policy.len() > 128
            || canonical_event_sequence == 0
            || !valid_integrity_hash(canonical_event_integrity_hash)
        {
            return Err(PrivacyError::InvalidInput);
        }
        let canonical_event_sequence =
            i64::try_from(canonical_event_sequence).map_err(|_| PrivacyError::InvalidInput)?;
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|_| PrivacyError::Database)?;
        sqlx::query(
            "INSERT INTO privacy_deletion_jobs \
             (job_id, campaign_id, subject_id, requested_by, retention_policy, status, evidence_status, \
              command_id, correlation_id, causation_id, canonical_event_type, \
              canonical_event_sequence, canonical_event_integrity_hash) \
             VALUES ($1, $2, $3, $4, $5, 'requested', 'confirmed', $6, $7, $8, $9, $10, $11) \
             ON CONFLICT (job_id) DO NOTHING",
        )
        .bind(job_id)
        .bind(evidence.campaign_id().as_str())
        .bind(subject_id)
        .bind(requested_by)
        .bind(retention_policy.trim())
        .bind(evidence.command_id().as_str())
        .bind(evidence.correlation_id().as_str())
        .bind(evidence.causation_id().as_str())
        .bind(evidence.event_type())
        .bind(canonical_event_sequence)
        .bind(canonical_event_integrity_hash)
        .execute(&mut *transaction)
        .await
        .map_err(deletion_evidence_write_error)?;
        let persisted = sqlx::query(
            "SELECT campaign_id, subject_id, requested_by, retention_policy, command_id, correlation_id, \
                    causation_id, canonical_event_type, evidence_status, \
                    canonical_event_sequence, canonical_event_integrity_hash \
               FROM privacy_deletion_jobs WHERE job_id = $1 FOR UPDATE",
        )
        .bind(job_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(|_| PrivacyError::Database)?;
        if persisted.get::<String, _>("campaign_id") != evidence.campaign_id().as_str()
            || persisted.get::<String, _>("subject_id") != subject_id
            || persisted.get::<String, _>("requested_by") != requested_by
            || persisted.get::<String, _>("retention_policy") != retention_policy.trim()
            || persisted.get::<String, _>("command_id") != evidence.command_id().as_str()
            || persisted.get::<String, _>("correlation_id") != evidence.correlation_id().as_str()
            || persisted.get::<String, _>("causation_id") != evidence.causation_id().as_str()
            || persisted.get::<String, _>("canonical_event_type") != evidence.event_type()
            || persisted.get::<String, _>("evidence_status") != "confirmed"
            || persisted.get::<Option<i64>, _>("canonical_event_sequence")
                != Some(canonical_event_sequence)
            || persisted
                .get::<Option<String>, _>("canonical_event_integrity_hash")
                .as_deref()
                != Some(canonical_event_integrity_hash)
        {
            return Err(PrivacyError::InvalidPersistedState);
        }
        for target in REQUIRED_DELETION_TARGETS {
            sqlx::query(
                "INSERT INTO privacy_deletion_job_targets (job_id, target, status) \
                 VALUES ($1, $2, 'pending') ON CONFLICT (job_id, target) DO NOTHING",
            )
            .bind(job_id)
            .bind(target.as_str())
            .execute(&mut *transaction)
            .await
            .map_err(|_| PrivacyError::Database)?;
        }
        transaction
            .commit()
            .await
            .map_err(|_| PrivacyError::Database)?;
        self.load(job_id).await
    }

    pub async fn load(&self, job_id: &str) -> Result<DeletionJob, PrivacyError> {
        self.load_scoped(job_id, None).await
    }

    pub async fn load_for_campaign(
        &self,
        job_id: &str,
        campaign_id: &EntityId,
    ) -> Result<DeletionJob, PrivacyError> {
        self.load_scoped(job_id, Some(campaign_id)).await
    }

    async fn load_scoped(
        &self,
        job_id: &str,
        campaign_id: Option<&EntityId>,
    ) -> Result<DeletionJob, PrivacyError> {
        validate_id(job_id)?;
        let row = sqlx::query(
            "SELECT job_id, campaign_id, subject_id, requested_by, retention_policy, status, failure_code, \
             evidence_status, canonical_event_sequence, canonical_event_integrity_hash \
             FROM privacy_deletion_jobs \
             WHERE job_id = $1 AND ($2::text IS NULL OR campaign_id = $2)",
        )
        .bind(job_id)
        .bind(campaign_id.map(EntityId::as_str))
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| PrivacyError::Database)?
        .ok_or(PrivacyError::JobNotFound)?;
        let target_rows = sqlx::query(
            "SELECT target, status, error_code FROM privacy_deletion_job_targets \
             WHERE job_id = $1 ORDER BY target",
        )
        .bind(job_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|_| PrivacyError::Database)?;
        let targets = target_rows
            .iter()
            .map(|row| {
                Ok(DeletionTargetRecord {
                    target: DeletionTarget::parse(row.try_get("target").map_err(db_error)?)?,
                    status: DeletionTargetStatus::parse(row.try_get("status").map_err(db_error)?)?,
                    error_code: row.try_get("error_code").map_err(db_error)?,
                })
            })
            .collect::<Result<Vec<_>, PrivacyError>>()?;

        Ok(DeletionJob {
            job_id: row.try_get("job_id").map_err(db_error)?,
            campaign_id: row.try_get("campaign_id").map_err(db_error)?,
            subject_id: row.try_get("subject_id").map_err(db_error)?,
            requested_by: row.try_get("requested_by").map_err(db_error)?,
            retention_policy: row.try_get("retention_policy").map_err(db_error)?,
            status: DeletionJobStatus::parse(row.try_get("status").map_err(db_error)?)?,
            failure_code: row.try_get("failure_code").map_err(db_error)?,
            evidence_status: DeletionEvidenceStatus::parse(
                row.try_get("evidence_status").map_err(db_error)?,
            )?,
            canonical_event_sequence: row
                .try_get::<Option<i64>, _>("canonical_event_sequence")
                .map_err(db_error)?
                .map(|value| u64::try_from(value).map_err(|_| PrivacyError::InvalidPersistedState))
                .transpose()?,
            canonical_event_integrity_hash: row
                .try_get("canonical_event_integrity_hash")
                .map_err(db_error)?,
            targets,
        })
    }

    async fn set_status(
        &self,
        job_id: &str,
        status: DeletionJobStatus,
        failure_code: Option<&str>,
    ) -> Result<(), PrivacyError> {
        let affected = sqlx::query(
            "UPDATE privacy_deletion_jobs SET status = $2, failure_code = $3, \
             updated_at = now() WHERE job_id = $1",
        )
        .bind(job_id)
        .bind(status.as_str())
        .bind(failure_code)
        .execute(&self.pool)
        .await
        .map_err(|_| PrivacyError::Database)?
        .rows_affected();
        if affected == 1 {
            Ok(())
        } else {
            Err(PrivacyError::JobNotFound)
        }
    }

    async fn set_execution_status(
        &self,
        job_id: &str,
        status: DeletionJobStatus,
        claim_token: &str,
    ) -> Result<(), PrivacyError> {
        if status != DeletionJobStatus::Verifying {
            return Err(PrivacyError::InvalidInput);
        }
        let affected = sqlx::query(
            "UPDATE privacy_deletion_jobs AS job \
                SET status = $2, failure_code = NULL, updated_at = statement_timestamp() \
              WHERE job.job_id = $1 AND job.status = 'running' \
                AND job.execution_claim_token = $3 \
                AND job.lease_expires_at > statement_timestamp() \
                AND EXISTS (\
                    SELECT 1 FROM privacy_subject_deletion_fences AS fence \
                     WHERE fence.subject_id = job.subject_id \
                       AND fence.job_id = job.job_id AND fence.status = 'running' \
                       AND fence.execution_claim_token = $3 \
                       AND fence.lease_expires_at > statement_timestamp()\
                )",
        )
        .bind(job_id)
        .bind(status.as_str())
        .bind(claim_token)
        .execute(&self.pool)
        .await
        .map_err(|_| PrivacyError::Database)?
        .rows_affected();
        if affected == 1 {
            Ok(())
        } else {
            Err(PrivacyError::ExecutionLeaseExpired)
        }
    }
}
