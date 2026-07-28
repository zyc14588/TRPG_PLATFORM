
#[async_trait]
impl DeletionSurface for PostgresRecordDeletionSurface {
    fn target(&self) -> DeletionTarget {
        self.target
    }

    async fn delete_subject_batch(
        &self,
        subject_id: &str,
        cursor: u64,
    ) -> Result<DeletionBatchProgress, PrivacyError> {
        if cursor != 1 {
            return Err(PrivacyError::InvalidPersistedState);
        }
        validate_id(subject_id)?;
        match self.target {
            DeletionTarget::Database => self.delete_database_subject(subject_id).await,
            DeletionTarget::RagIndex => {
                let mut transaction = self
                    .pool
                    .begin()
                    .await
                    .map_err(|_| PrivacyError::Database)?;
                sqlx::query(
                    "DELETE FROM rag_snapshot_chunk WHERE visibility_subject = $1 \
                     OR source_event_sequence IN (SELECT sequence FROM event_store \
                                                   WHERE data_subject_id = $1)",
                )
                .bind(subject_id)
                .execute(&mut *transaction)
                .await
                .map_err(|_| PrivacyError::Database)?;
                sqlx::query(
                    "DELETE FROM privacy_deletion_surface_records \
                     WHERE surface = 'rag_index' AND subject_id = $1",
                )
                .bind(subject_id)
                .execute(&mut *transaction)
                .await
                .map_err(|_| PrivacyError::Database)?;
                transaction
                    .commit()
                    .await
                    .map_err(|_| PrivacyError::Database)
            }
            _ => Err(PrivacyError::InvalidInput),
        }?;
        DeletionBatchProgress::complete(cursor)
    }

    async fn verify_absent(&self, subject_id: &str) -> Result<bool, PrivacyError> {
        validate_id(subject_id)?;
        match self.target {
            DeletionTarget::Database => self.verify_database_subject(subject_id).await,
            DeletionTarget::RagIndex => {
                let count = sqlx::query_scalar::<_, i64>(
                    "SELECT count(*) FROM rag_snapshot_chunk \
                     WHERE visibility_subject = $1 OR source_event_sequence IN \
                     (SELECT sequence FROM event_store WHERE data_subject_id = $1)",
                )
                .bind(subject_id)
                .fetch_one(&self.pool)
                .await
                .map_err(|_| PrivacyError::Database)?;
                Ok(count == 0)
            }
            _ => Err(PrivacyError::InvalidInput),
        }
    }
}

pub struct FilesystemDeletionSurface {
    root: PathBuf,
    target: DeletionTarget,
}

impl FilesystemDeletionSurface {
    pub fn new(root: impl AsRef<Path>, target: DeletionTarget) -> Result<Self, PrivacyError> {
        let root = root.as_ref();
        if !root.is_absolute()
            || root.parent().is_none()
            || !matches!(
                target,
                DeletionTarget::ObjectStorage | DeletionTarget::Export
            )
        {
            return Err(PrivacyError::InvalidInput);
        }
        Ok(Self {
            root: root.to_path_buf(),
            target,
        })
    }

    pub fn subject_path(&self, subject_id: &str) -> Result<PathBuf, PrivacyError> {
        validate_id(subject_id)?;
        Ok(self.root.join(subject_id))
    }
}

#[async_trait]
impl DeletionSurface for FilesystemDeletionSurface {
    fn target(&self) -> DeletionTarget {
        self.target
    }

    async fn delete_subject_batch(
        &self,
        subject_id: &str,
        cursor: u64,
    ) -> Result<DeletionBatchProgress, PrivacyError> {
        if cursor != 1 {
            return Err(PrivacyError::InvalidPersistedState);
        }
        let path = self.subject_path(subject_id)?;
        match tokio::fs::remove_dir_all(path).await {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(PrivacyError::Storage),
        };
        DeletionBatchProgress::complete(cursor)
    }

    async fn verify_absent(&self, subject_id: &str) -> Result<bool, PrivacyError> {
        match tokio::fs::metadata(self.subject_path(subject_id)?).await {
            Ok(_) => Ok(false),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(true),
            Err(_) => Err(PrivacyError::Storage),
        }
    }
}

pub struct BackupKeyDeletionSurface {
    pool: PgPool,
}

impl BackupKeyDeletionSurface {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn put_for_test(
        &self,
        subject_id: &str,
        key_reference: &str,
        wrapped_key: &[u8],
    ) -> Result<(), PrivacyError> {
        validate_id(subject_id)?;
        validate_id(key_reference)?;
        if wrapped_key.is_empty() {
            return Err(PrivacyError::InvalidInput);
        }
        sqlx::query(
            "INSERT INTO privacy_subject_keys (subject_id, key_reference, wrapped_key) \
             VALUES ($1, $2, $3) ON CONFLICT (subject_id) DO UPDATE SET \
             key_reference = EXCLUDED.key_reference, wrapped_key = EXCLUDED.wrapped_key, \
             destroyed_at = NULL",
        )
        .bind(subject_id)
        .bind(key_reference)
        .bind(wrapped_key)
        .execute(&self.pool)
        .await
        .map_err(|_| PrivacyError::Database)?;
        Ok(())
    }
}

#[async_trait]
impl DeletionSurface for BackupKeyDeletionSurface {
    fn target(&self) -> DeletionTarget {
        DeletionTarget::BackupKey
    }

    async fn delete_subject_batch(
        &self,
        subject_id: &str,
        cursor: u64,
    ) -> Result<DeletionBatchProgress, PrivacyError> {
        if cursor != 1 {
            return Err(PrivacyError::InvalidPersistedState);
        }
        validate_id(subject_id)?;
        let affected = sqlx::query(
            "UPDATE privacy_subject_keys SET wrapped_key = NULL, destroyed_at = now() \
             WHERE subject_id = $1 AND wrapped_key IS NOT NULL AND destroyed_at IS NULL",
        )
        .bind(subject_id)
        .execute(&self.pool)
        .await
        .map_err(|_| PrivacyError::Database)?
        .rows_affected();
        if affected == 0 {
            let key_state = sqlx::query(
                "SELECT wrapped_key IS NULL AS material_destroyed, \
                        destroyed_at IS NOT NULL AS destruction_recorded \
                   FROM privacy_subject_keys WHERE subject_id = $1",
            )
            .bind(subject_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|_| PrivacyError::Database)?;
            if let Some(key_state) = key_state {
                return if key_state.get::<bool, _>("material_destroyed")
                    && key_state.get::<bool, _>("destruction_recorded")
                {
                    DeletionBatchProgress::complete(cursor)
                } else {
                    Err(PrivacyError::InvalidPersistedState)
                };
            }
            let protected_events: bool = sqlx::query_scalar(
                "SELECT EXISTS (SELECT 1 FROM event_store WHERE data_subject_id = $1)",
            )
            .bind(subject_id)
            .fetch_one(&self.pool)
            .await
            .map_err(|_| PrivacyError::Database)?;
            if protected_events {
                return Err(PrivacyError::InvalidPersistedState);
            }
        }
        DeletionBatchProgress::complete(cursor)
    }

    async fn verify_absent(&self, subject_id: &str) -> Result<bool, PrivacyError> {
        let material_exists = sqlx::query_scalar::<_, bool>(
            "SELECT wrapped_key IS NOT NULL FROM privacy_subject_keys WHERE subject_id = $1",
        )
        .bind(subject_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| PrivacyError::Database)?;
        let replayable_events: bool = sqlx::query_scalar(
            "SELECT EXISTS (\
                SELECT 1 FROM event_store AS event \
                 JOIN privacy_subject_keys AS subject_key \
                   ON subject_key.subject_id = event.data_subject_id \
                  AND subject_key.key_reference = event.payload_key_reference \
                WHERE event.data_subject_id = $1 \
                  AND subject_key.wrapped_key IS NOT NULL \
                  AND subject_key.destroyed_at IS NULL\
             )",
        )
        .bind(subject_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|_| PrivacyError::Database)?;
        Ok(!material_exists.unwrap_or(false) && !replayable_events)
    }
}

pub struct DeletionWorker {
    repository: PostgresDeletionRepository,
    legal_holds: std::sync::Arc<dyn LegalHoldResolver>,
    surfaces: HashMap<DeletionTarget, Box<dyn DeletionSurface>>,
}
