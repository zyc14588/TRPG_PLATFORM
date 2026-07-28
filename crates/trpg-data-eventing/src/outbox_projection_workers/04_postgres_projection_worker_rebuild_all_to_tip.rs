
impl PostgresProjectionWorker {

    pub async fn rebuild_all_to_tip(
        &self,
    ) -> Result<Vec<ProjectionCheckpointState>, EventWorkerError> {
        let streams = sqlx::query_as::<_, ProjectionStreamScope>(
            r#"
            SELECT DISTINCT campaign_id, stream_id
              FROM public.event_store
             ORDER BY campaign_id, stream_id
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|_| EventWorkerError::Database("load_projection_streams"))?;
        let mut checkpoints = Vec::with_capacity(streams.len());
        for stream in streams {
            checkpoints.push(
                self.rebuild_to_tip(&stream.campaign_id, &stream.stream_id)
                    .await?,
            );
        }
        Ok(checkpoints)
    }

    async fn repair_materialization_if_inconsistent(
        &self,
        campaign_id: &str,
        stream_id: &str,
    ) -> Result<bool, EventWorkerError> {
        validate_stream_scope(campaign_id, stream_id)?;
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|_| EventWorkerError::Database("begin_projection_consistency_check"))?;
        self.lock_stream(&mut transaction, campaign_id, stream_id)
            .await?;

        let checkpoint = sqlx::query_as::<_, ProjectionCheckpointState>(
            r#"
            SELECT projection_name, campaign_id, stream_id, version,
                   last_event_sequence, projection_hash, rebuilt_at
              FROM public.projection_checkpoint
             WHERE projection_name = $1
               AND campaign_id = $2
               AND stream_id = $3
             FOR UPDATE
            "#,
        )
        .bind(&self.projection_name)
        .bind(campaign_id)
        .bind(stream_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|_| EventWorkerError::Database("inspect_projection_checkpoint"))?;
        let summary = sqlx::query_as::<_, ProjectionMaterializationSummary>(
            r#"
            SELECT count(*) AS projected_rows,
                   min(stream_version) AS first_version,
                   max(stream_version) AS last_version,
                   (array_agg(event_sequence ORDER BY stream_version DESC))[1]
                       AS last_event_sequence,
                   (array_agg(projection_hash ORDER BY stream_version DESC))[1]
                       AS last_projection_hash
              FROM public.canonical_event_projection
             WHERE projection_name = $1
               AND campaign_id = $2
               AND stream_id = $3
            "#,
        )
        .bind(&self.projection_name)
        .bind(campaign_id)
        .bind(stream_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(|_| EventWorkerError::Database("inspect_projection_materialization"))?;

        if summary.matches(checkpoint.as_ref()) {
            transaction
                .commit()
                .await
                .map_err(|_| EventWorkerError::Database("commit_projection_consistency_check"))?;
            return Ok(false);
        }

        self.delete_materialization(&mut transaction, campaign_id, stream_id)
            .await?;
        transaction
            .commit()
            .await
            .map_err(|_| EventWorkerError::Database("commit_projection_repair"))?;
        Ok(true)
    }

    async fn lock_stream(
        &self,
        transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        campaign_id: &str,
        stream_id: &str,
    ) -> Result<(), EventWorkerError> {
        sqlx::query(
            "SELECT pg_advisory_xact_lock(hashtextextended($1 || ':' || $2 || ':' || $3, 0))",
        )
        .bind(&self.projection_name)
        .bind(campaign_id)
        .bind(stream_id)
        .execute(&mut **transaction)
        .await
        .map_err(|_| EventWorkerError::Database("lock_projection_repair"))?;
        Ok(())
    }

    async fn delete_materialization(
        &self,
        transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        campaign_id: &str,
        stream_id: &str,
    ) -> Result<(), EventWorkerError> {
        sqlx::query(
            r#"
            DELETE FROM public.canonical_event_projection
             WHERE projection_name = $1
               AND campaign_id = $2
               AND stream_id = $3
            "#,
        )
        .bind(&self.projection_name)
        .bind(campaign_id)
        .bind(stream_id)
        .execute(&mut **transaction)
        .await
        .map_err(|_| EventWorkerError::Database("delete_projection_materialization"))?;
        sqlx::query(
            r#"
            DELETE FROM public.projection_checkpoint
             WHERE projection_name = $1
               AND campaign_id = $2
               AND stream_id = $3
            "#,
        )
        .bind(&self.projection_name)
        .bind(campaign_id)
        .bind(stream_id)
        .execute(&mut **transaction)
        .await
        .map_err(|_| EventWorkerError::Database("delete_projection_checkpoint"))?;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EventWorkerError {
    Configuration(&'static str),
    Database(&'static str),
    ClaimLost,
    InvalidOutboxPayload,
    ProjectionHash(ProjectionHashError),
    ProjectionSerialization,
    ProjectionReadModelConflict,
    ProjectionStreamGap {
        expected: i64,
        actual: i64,
    },
    CheckpointIdentityMismatch,
    CheckpointConflict {
        expected_version: i64,
        actual_version: i64,
    },
}

impl fmt::Display for EventWorkerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Configuration(reason) => write!(formatter, "worker configuration error: {reason}"),
            Self::Database(operation) => write!(formatter, "event worker database failed: {operation}"),
            Self::ClaimLost => formatter.write_str("outbox claim is no longer owned by this worker"),
            Self::InvalidOutboxPayload => formatter.write_str("invalid canonical outbox payload"),
            Self::ProjectionHash(error) => error.fmt(formatter),
            Self::ProjectionSerialization => {
                formatter.write_str("projection event serialization failed")
            }
            Self::ProjectionReadModelConflict => {
                formatter.write_str("projection read model does not match its checkpoint")
            }
            Self::ProjectionStreamGap { expected, actual } => write!(
                formatter,
                "projection stream gap: expected version {expected}, actual version {actual}"
            ),
            Self::CheckpointIdentityMismatch => {
                formatter.write_str("projection page identity does not match worker")
            }
            Self::CheckpointConflict {
                expected_version,
                actual_version,
            } => write!(
                formatter,
                "checkpoint conflict: expected version {expected_version}, actual version {actual_version}"
            ),
        }
    }
}

impl std::error::Error for EventWorkerError {}

impl From<ProjectionHashError> for EventWorkerError {
    fn from(error: ProjectionHashError) -> Self {
        Self::ProjectionHash(error)
    }
}

#[derive(FromRow)]
struct ProjectionStreamScope {
    campaign_id: String,
    stream_id: String,
}

#[derive(FromRow)]
struct ProjectionMaterializationSummary {
    projected_rows: i64,
    first_version: Option<i64>,
    last_version: Option<i64>,
    last_event_sequence: Option<i64>,
    last_projection_hash: Option<String>,
}

impl ProjectionMaterializationSummary {
    fn matches(&self, checkpoint: Option<&ProjectionCheckpointState>) -> bool {
        match checkpoint {
            None => {
                self.projected_rows == 0
                    && self.first_version.is_none()
                    && self.last_version.is_none()
                    && self.last_event_sequence.is_none()
                    && self.last_projection_hash.is_none()
            }
            Some(checkpoint) if checkpoint.version == 0 => {
                checkpoint.last_event_sequence == 0
                    && checkpoint.projection_hash == PROJECTION_HASH_GENESIS
                    && self.projected_rows == 0
                    && self.first_version.is_none()
                    && self.last_version.is_none()
                    && self.last_event_sequence.is_none()
                    && self.last_projection_hash.is_none()
            }
            Some(checkpoint) => {
                self.projected_rows == checkpoint.version
                    && self.first_version == Some(1)
                    && self.last_version == Some(checkpoint.version)
                    && self.last_event_sequence == Some(checkpoint.last_event_sequence)
                    && self.last_projection_hash.as_deref()
                        == Some(checkpoint.projection_hash.as_str())
            }
        }
    }
}

#[derive(FromRow)]
struct ProjectionEventRow {
    sequence: i64,
    stream_version: i64,
    stream_id: String,
    event_type: String,
    event_schema_version: i32,
    campaign_id: String,
    expected_version: i64,
    authority_mode: String,
    authenticated_actor_id: String,
    authenticated_actor_role: String,
    authenticated_actor_origin: Json<EventActorOriginWire>,
    resource_type: String,
    resource_id: String,
    authority_contract_id: String,
    authority_owner: String,
    command_id: String,
    idempotency_key: String,
    idempotency_operation: String,
    authority_contract_version: i64,
    visibility_label: String,
    visibility_subject: String,
    provenance_kind: String,
    provenance_reference: String,
    provenance_recorded_by: String,
    correlation_id: String,
    causation_id: String,
    trace_id: String,
    payload: Value,
    recorded_at: DateTime<Utc>,
    event_integrity_hash: Option<String>,
    request_hash: String,
    request_hash_source: String,
    integrity_status: String,
    payload_integrity_source: String,
}

impl From<ProjectionEventRow> for CanonicalReplayEvent {
    fn from(row: ProjectionEventRow) -> Self {
        Self {
            sequence: row.sequence,
            stream_version: row.stream_version,
            stream_id: row.stream_id,
            event_type: row.event_type,
            event_schema_version: row.event_schema_version,
            campaign_id: row.campaign_id,
            expected_version: row.expected_version,
            authority_mode: row.authority_mode,
            authenticated_actor_id: row.authenticated_actor_id,
            authenticated_actor_role: row.authenticated_actor_role,
            authenticated_actor_origin: row.authenticated_actor_origin.0,
            resource_type: row.resource_type,
            resource_id: row.resource_id,
            authority_contract_id: row.authority_contract_id,
            authority_owner: row.authority_owner,
            command_id: row.command_id,
            idempotency_key: row.idempotency_key,
            idempotency_operation: row.idempotency_operation,
            authority_contract_version: row.authority_contract_version,
            visibility_label: row.visibility_label,
            visibility_subject: row.visibility_subject,
            provenance_kind: row.provenance_kind,
            provenance_reference: row.provenance_reference,
            provenance_recorded_by: row.provenance_recorded_by,
            correlation_id: row.correlation_id,
            causation_id: row.causation_id,
            trace_id: row.trace_id,
            payload: row.payload,
            recorded_at: row.recorded_at,
            event_integrity_hash: row.event_integrity_hash,
            request_hash: row.request_hash,
            request_hash_source: row.request_hash_source,
            integrity_status: row.integrity_status,
            payload_integrity_source: row.payload_integrity_source,
        }
    }
}
