
#[async_trait]
impl DeletionSurface for NatsQueueDeletionSurface {
    fn target(&self) -> DeletionTarget {
        DeletionTarget::Queue
    }

    async fn delete_subject_batch(
        &self,
        context: &DeletionExecutionContext,
        cursor: u64,
    ) -> Result<DeletionBatchProgress, PrivacyError> {
        let subject_id = context.subject_id();
        validate_id(subject_id)?;
        if cursor == 0 {
            return Err(PrivacyError::InvalidPersistedState);
        }
        if let Some(pool) = &self.canonical_pool {
            let key_destroyed: bool = sqlx::query_scalar(
                "SELECT COALESCE((SELECT wrapped_key IS NULL AND destroyed_at IS NOT NULL \
                                  FROM privacy_subject_keys WHERE subject_id = $1), true)",
            )
            .bind(subject_id)
            .fetch_one(pool)
            .await
            .map_err(|_| PrivacyError::Database)?;
            if !key_destroyed {
                return Err(PrivacyError::InvalidPersistedState);
            }
            sqlx::query(
                "UPDATE event_outbox SET delivery_status = 'dead_lettered', \
                 dead_lettered_at = COALESCE(dead_lettered_at, now()), available_at = now(), \
                 last_error = 'DATA_SUBJECT_CRYPTO_ERASED', claimed_at = NULL, \
                 claim_owner = NULL, claim_token = NULL, locked_until = NULL \
                 WHERE data_subject_id = $1 AND published_at IS NULL \
                   AND dead_lettered_at IS NULL",
            )
            .bind(subject_id)
            .execute(pool)
            .await
            .map_err(|_| PrivacyError::Database)?;
            let stream = self
                .jetstream
                .get_stream(&self.stream_name)
                .await
                .map_err(|_| PrivacyError::Queue)?;
            let (sequences, next_cursor, complete) = self
                .canonical_subject_message_batch(subject_id, cursor, NATS_DELETION_BATCH_SIZE)
                .await?;
            for sequence in sequences {
                if !stream
                    .delete_message(sequence)
                    .await
                    .map_err(|_| PrivacyError::Queue)?
                {
                    return Err(PrivacyError::Queue);
                }
            }
            return Ok(DeletionBatchProgress {
                next_cursor,
                complete,
            });
        }
        if cursor != 1 {
            return Err(PrivacyError::InvalidPersistedState);
        }
        let stream = self
            .jetstream
            .get_stream(&self.stream_name)
            .await
            .map_err(|_| PrivacyError::Queue)?;
        stream
            .purge()
            .filter(self.subject(subject_id))
            .await
            .map_err(|_| PrivacyError::Queue)?;
        DeletionBatchProgress::complete(cursor)
    }

    async fn verify_absent(&self, subject_id: &str) -> Result<bool, PrivacyError> {
        validate_id(subject_id)?;
        if let Some(pool) = &self.canonical_pool {
            let row = sqlx::query(
                "SELECT \
                    NOT EXISTS (SELECT 1 FROM event_outbox \
                                WHERE data_subject_id = $1 AND published_at IS NULL \
                                  AND dead_lettered_at IS NULL) AS no_deliverable_rows, \
                    NOT EXISTS (SELECT 1 FROM privacy_subject_keys \
                                WHERE subject_id = $1 AND wrapped_key IS NOT NULL) \
                        AS key_unavailable, \
                    NOT EXISTS (SELECT 1 FROM event_store \
                                WHERE data_subject_id = $1 \
                                  AND (payload_json ? 'protected_payload') IS NOT TRUE) \
                        AS all_events_protected, \
                    NOT EXISTS (SELECT 1 FROM event_outbox \
                                WHERE data_subject_id = $1 \
                                  AND (payload_json ? 'protected_payload') IS NOT TRUE) \
                        AS all_outbox_protected",
            )
            .bind(subject_id)
            .fetch_one(pool)
            .await
            .map_err(|_| PrivacyError::Database)?;
            return Ok(row.get::<bool, _>("no_deliverable_rows")
                && row.get::<bool, _>("key_unavailable")
                && row.get::<bool, _>("all_events_protected")
                && row.get::<bool, _>("all_outbox_protected")
                && self
                    .canonical_subject_message_batch(subject_id, 1, 1)
                    .await?
                    .0
                    .is_empty());
        }
        let stream = self
            .jetstream
            .get_stream(&self.stream_name)
            .await
            .map_err(|_| PrivacyError::Queue)?;
        match stream
            .get_last_raw_message_by_subject(&self.subject(subject_id))
            .await
        {
            Ok(_) => Ok(false),
            Err(error)
                if error.kind()
                    == async_nats::jetstream::stream::LastRawMessageErrorKind::NoMessageFound =>
            {
                Ok(true)
            }
            Err(_) => Err(PrivacyError::Queue),
        }
    }
}

fn validate_secure_service_url(
    value: &str,
    cleartext_scheme: &str,
    tls_scheme: &str,
) -> Result<(), PrivacyError> {
    let parsed = Url::parse(value).map_err(|_| PrivacyError::InvalidInput)?;
    let host = parsed.host_str().ok_or(PrivacyError::InvalidInput)?;
    let local = matches!(host, "localhost" | "127.0.0.1" | "[::1]" | "::1");
    if parsed.scheme() != tls_scheme && !(local && parsed.scheme() == cleartext_scheme) {
        return Err(PrivacyError::InvalidInput);
    }
    Ok(())
}

pub struct PostgresRecordDeletionSurface {
    pool: PgPool,
    target: DeletionTarget,
}

impl PostgresRecordDeletionSurface {
    pub fn new(pool: PgPool, target: DeletionTarget) -> Result<Self, PrivacyError> {
        if !matches!(target, DeletionTarget::Database | DeletionTarget::RagIndex) {
            return Err(PrivacyError::InvalidInput);
        }
        Ok(Self { pool, target })
    }

    async fn delete_database_subject(
        &self,
        context: &DeletionExecutionContext,
    ) -> Result<(), PrivacyError> {
        sqlx::query(
            "SELECT public.erase_privacy_database_subject($1, $2, $3)",
        )
        .bind(context.job_id())
        .bind(context.subject_id())
        .bind(context.claim_token())
        .execute(&self.pool)
        .await
        .map_err(deletion_surface_write_error)?;
        Ok(())
    }

    async fn verify_database_subject(&self, subject_id: &str) -> Result<bool, PrivacyError> {
        let row = sqlx::query(
            "SELECT \
                EXISTS (SELECT 1 FROM privacy_erased_subjects WHERE subject_id = $1) AS erased, \
                EXISTS (SELECT 1 FROM sessions WHERE user_id = $1) AS has_sessions, \
                EXISTS (SELECT 1 FROM campaign_memberships \
                         WHERE user_id = $1 AND revoked_at IS NULL) AS active_membership, \
                EXISTS (SELECT 1 FROM campaign_group_memberships \
                         WHERE user_id = $1 AND revoked_at IS NULL) AS active_group, \
                EXISTS (SELECT 1 FROM cloud_egress_consents \
                         WHERE subject_id = $1 AND granted = true) AS active_consent, \
                COALESCE((SELECT disabled_at IS NOT NULL \
                          AND login_normalized ~ '^deleted_[0-9a-f]{64}$' \
                          AND password_hash ~ '^DELETED_ACCOUNT_NO_LOGIN_[0-9a-f]{64}$' \
                            FROM users WHERE user_id = $1), true) AS identity_erased",
        )
        .bind(subject_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|_| PrivacyError::Database)?;
        Ok(row.get::<bool, _>("erased")
            && !row.get::<bool, _>("has_sessions")
            && !row.get::<bool, _>("active_membership")
            && !row.get::<bool, _>("active_group")
            && !row.get::<bool, _>("active_consent")
            && row.get::<bool, _>("identity_erased"))
    }
}
