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

fn deletion_surface_write_error(error: sqlx::Error) -> PrivacyError {
    match error.as_database_error().map(|database_error| database_error.message()) {
        Some("canonical authority owner requires a campaign fork before erasure") => {
            PrivacyError::ProtectedCanonicalIdentity
        }
        Some("persisted erasure digest does not match data subject") => {
            PrivacyError::InvalidPersistedState
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
        context: &DeletionExecutionContext,
        cursor: u64,
    ) -> Result<DeletionBatchProgress, PrivacyError>;
    async fn verify_absent(&self, subject_id: &str) -> Result<bool, PrivacyError>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeletionBatchProgress {
    pub next_cursor: u64,
    pub complete: bool,
}
