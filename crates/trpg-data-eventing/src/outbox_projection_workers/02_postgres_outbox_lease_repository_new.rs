
impl PostgresOutboxLeaseRepository {
    pub fn new(
        pool: PgPool,
        worker_id: impl Into<String>,
        policy: OutboxLeasePolicy,
    ) -> Result<Self, EventWorkerError> {
        let worker_id = worker_id.into();
        if !valid_worker_id(&worker_id) {
            return Err(EventWorkerError::Configuration("invalid_worker_id"));
        }
        Ok(Self {
            pool,
            worker_id,
            policy: policy.validate()?,
        })
    }

    pub fn worker_id(&self) -> &str {
        &self.worker_id
    }

    pub fn lease_duration(&self) -> Duration {
        self.policy.lease_duration
    }

    pub async fn claim_batch(&self, limit: i64) -> Result<Vec<OutboxClaim>, EventWorkerError> {
        if !(1..=1_000).contains(&limit) {
            return Err(EventWorkerError::Configuration(
                "outbox_claim_limit_out_of_range",
            ));
        }
        let lease_milliseconds = duration_milliseconds(self.policy.lease_duration).ok_or(
            EventWorkerError::Configuration("outbox_lease_duration_out_of_range"),
        )?;
        let claim_token_prefix = next_claim_token_prefix(&self.worker_id)?;
        sqlx::query_as::<_, OutboxClaim>(
            r#"
            WITH candidates AS (
                SELECT outbox_id
                  FROM public.event_outbox
                 WHERE published_at IS NULL
                   AND dead_lettered_at IS NULL
                   AND integrity_status = 'verified_hmac'
                   AND request_hash_source = 'formal_commit'
                   AND commit_id IS NOT NULL
                   AND EXISTS (
                       SELECT 1
                         FROM public.event_store AS event
                        WHERE event.sequence = event_outbox.event_sequence
                          AND (
                              event.data_subject_id = 'not_applicable'
                              OR EXISTS (
                                      SELECT 1
                                        FROM public.privacy_subject_keys AS subject_key
                                       WHERE subject_key.subject_id = event.data_subject_id
                                         AND subject_key.key_reference = event.payload_key_reference
                                         AND subject_key.wrapped_key IS NOT NULL
                                         AND subject_key.destroyed_at IS NULL
                                  )
                          )
                   )
                   AND available_at <= now()
                   AND (
                       delivery_status IN ('pending', 'retrying')
                       OR delivery_status = 'claimed' AND locked_until <= now()
                   )
                 ORDER BY available_at, outbox_id
                 FOR UPDATE SKIP LOCKED
                 LIMIT $2
            ), claimed AS (
                UPDATE public.event_outbox AS outbox
                   SET delivery_status = 'claimed',
                       claimed_at = now(),
                       claim_owner = $1,
                       locked_until = now() + ($3::bigint * interval '1 millisecond'),
                       claim_token = $4 || ':' || outbox.outbox_id::TEXT
                  FROM candidates
                 WHERE outbox.outbox_id = candidates.outbox_id
                RETURNING outbox.*
            )
            SELECT claimed.outbox_id, claimed.event_sequence,
                   claimed.nats_subject AS subject, claimed.idempotency_key,
                   claimed.visibility_label, claimed.correlation_id,
                   claimed.causation_id, claimed.payload_json,
                   claimed.commit_id, event.event_type,
                   event.event_schema_version, event.campaign_id,
                   event.stream_id, event.stream_version,
                   event.expected_version,
                   event.idempotency_key AS event_idempotency_key,
                   event.idempotency_operation,
                   event.authenticated_actor_id,
                   event.authenticated_actor_role,
                   event.authenticated_actor_origin,
                   event.resource_type, event.resource_id,
                   event.authority_contract_id, event.authority_owner,
                   event.authority_contract_version, event.command_id,
                   event.visibility_subject, event.data_subject_id,
                   event.fact_provenance_kind AS provenance_kind,
                   event.fact_provenance_reference AS provenance_reference,
                   event.fact_recorded_by AS provenance_recorded_by,
                   event.trace_id, event.recorded_at,
                   event.event_integrity_hash, event.request_hash,
                   event.request_hash_source, event.integrity_status,
                   event.payload_integrity_source, claimed.retry_count,
                   claimed.claimed_at, claimed.locked_until,
                   claimed.claim_token
              FROM claimed
              JOIN public.event_store AS event
                ON event.sequence = claimed.event_sequence
             ORDER BY claimed.outbox_id
            "#,
        )
        .bind(&self.worker_id)
        .bind(limit)
        .bind(lease_milliseconds)
        .bind(claim_token_prefix)
        .fetch_all(&self.pool)
        .await
        .map_err(|_| EventWorkerError::Database("claim_outbox_batch"))
    }

    pub async fn mark_published(&self, claim: &OutboxClaim) -> Result<(), EventWorkerError> {
        let result = sqlx::query(
            r#"
            UPDATE public.event_outbox
               SET delivery_status = 'published',
                   published_at = now(),
                   available_at = now(),
                   claimed_at = NULL,
                   claim_owner = NULL,
                   claim_token = NULL,
                   locked_until = NULL,
                   last_error = NULL
             WHERE outbox_id = $1
               AND delivery_status = 'claimed'
               AND claim_owner = $2
               AND claim_token = $3
               AND claimed_at = $4
               AND locked_until = $5
               AND locked_until > now()
               AND published_at IS NULL
               AND dead_lettered_at IS NULL
            "#,
        )
        .bind(claim.outbox_id)
        .bind(&self.worker_id)
        .bind(&claim.claim_token)
        .bind(claim.claimed_at)
        .bind(claim.locked_until)
        .execute(&self.pool)
        .await
        .map_err(|_| EventWorkerError::Database("mark_outbox_published"))?;
        require_owned_claim(result.rows_affected())
    }

    /// Historical or otherwise unverified rows remain auditable in the
    /// outbox but are permanently quarantined before any NATS claim. They can
    /// never enter the shared canonical-events stream.
    pub async fn quarantine_unverified_history(&self) -> Result<u64, EventWorkerError> {
        let result = sqlx::query(
            r#"
            UPDATE public.event_outbox
               SET delivery_status = 'dead_lettered',
                   dead_lettered_at = COALESCE(dead_lettered_at, now()),
                   available_at = now(),
                   last_error = 'UNVERIFIED_HISTORY_QUARANTINED',
                   claimed_at = NULL,
                   claim_owner = NULL,
                   claim_token = NULL,
                   locked_until = NULL
             WHERE published_at IS NULL
               AND dead_lettered_at IS NULL
               AND (
                    integrity_status <> 'verified_hmac'
                    OR request_hash_source <> 'formal_commit'
                    OR commit_id IS NULL
                    OR EXISTS (
                        SELECT 1
                          FROM public.event_store AS event
                         WHERE event.sequence = event_outbox.event_sequence
                           AND event.data_subject_id <> 'not_applicable'
                           AND (
                               NOT EXISTS (
                                   SELECT 1
                                     FROM public.privacy_subject_keys AS subject_key
                                    WHERE subject_key.subject_id = event.data_subject_id
                                      AND subject_key.key_reference = event.payload_key_reference
                                      AND subject_key.wrapped_key IS NOT NULL
                                      AND subject_key.destroyed_at IS NULL
                               )
                           )
                    )
               )
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|_| EventWorkerError::Database("quarantine_unverified_outbox"))?;
        Ok(result.rows_affected())
    }

    pub async fn mark_failed(
        &self,
        claim: &OutboxClaim,
        failure: OutboxFailureCode,
    ) -> Result<OutboxFailureDisposition, EventWorkerError> {
        let next_attempt = claim.retry_count.saturating_add(1);
        let dead_lettered = next_attempt >= self.policy.maximum_attempts;
        let backoff_milliseconds =
            duration_milliseconds(self.policy.backoff_for_attempt(next_attempt)).ok_or(
                EventWorkerError::Configuration("outbox_backoff_out_of_range"),
            )?;
        let row = sqlx::query(
            r#"
            UPDATE public.event_outbox
               SET retry_count = retry_count + 1,
                   last_error = $6,
                   delivery_status = CASE
                       WHEN $7 THEN 'dead_lettered'
                       ELSE 'retrying'
                   END,
                   available_at = CASE
                       WHEN $7 THEN now()
                       ELSE now() + ($8::bigint * interval '1 millisecond')
                   END,
                   dead_lettered_at = CASE WHEN $7 THEN now() ELSE NULL END,
                   claimed_at = NULL,
                   claim_owner = NULL,
                   claim_token = NULL,
                   locked_until = NULL
             WHERE outbox_id = $1
               AND delivery_status = 'claimed'
               AND claim_owner = $2
               AND claim_token = $3
               AND claimed_at = $4
               AND locked_until = $5
               AND locked_until > now()
               AND published_at IS NULL
               AND dead_lettered_at IS NULL
            RETURNING retry_count, delivery_status, available_at
            "#,
        )
        .bind(claim.outbox_id)
        .bind(&self.worker_id)
        .bind(&claim.claim_token)
        .bind(claim.claimed_at)
        .bind(claim.locked_until)
        .bind(failure.as_str())
        .bind(dead_lettered)
        .bind(backoff_milliseconds)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| EventWorkerError::Database("mark_outbox_failed"))?
        .ok_or(EventWorkerError::ClaimLost)?;
        let delivery_status: String = row.get("delivery_status");
        Ok(OutboxFailureDisposition {
            retry_count: row.get("retry_count"),
            dead_lettered: delivery_status == "dead_lettered",
            available_at: row.get("available_at"),
        })
    }

    pub async fn pending_count(&self) -> Result<i64, EventWorkerError> {
        sqlx::query_scalar(
            "SELECT count(*) FROM public.event_outbox WHERE delivery_status IN ('pending', 'claimed', 'retrying')",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|_| EventWorkerError::Database("count_pending_outbox"))
    }

    /// A non-zero value is the persistent alert condition consumed by the
    /// service's metrics/alerting layer; dead letters are never auto-deleted.
    pub async fn dead_letter_count(&self) -> Result<i64, EventWorkerError> {
        sqlx::query_scalar(
            "SELECT count(*) FROM public.event_outbox WHERE delivery_status = 'dead_lettered'",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|_| EventWorkerError::Database("count_dead_letter_outbox"))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, FromRow)]
pub struct ProjectionCheckpointState {
    pub projection_name: String,
    pub campaign_id: String,
    pub stream_id: String,
    pub version: i64,
    pub last_event_sequence: i64,
    pub projection_hash: String,
    pub rebuilt_at: DateTime<Utc>,
}

impl ProjectionCheckpointState {
    fn genesis(projection_name: &str, campaign_id: &str, stream_id: &str) -> Self {
        Self {
            projection_name: projection_name.to_owned(),
            campaign_id: campaign_id.to_owned(),
            stream_id: stream_id.to_owned(),
            version: 0,
            last_event_sequence: 0,
            projection_hash: PROJECTION_HASH_GENESIS.to_owned(),
            rebuilt_at: Utc::now(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProjectionPage {
    start: ProjectionCheckpointState,
    target: ProjectionCheckpointState,
    events: Vec<CanonicalReplayEvent>,
}

impl ProjectionPage {
    pub fn start(&self) -> &ProjectionCheckpointState {
        &self.start
    }

    pub fn target(&self) -> &ProjectionCheckpointState {
        &self.target
    }

    pub fn events(&self) -> &[CanonicalReplayEvent] {
        &self.events
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CheckpointAdvance {
    Advanced(ProjectionCheckpointState),
    AlreadyApplied(ProjectionCheckpointState),
    NoEvents(ProjectionCheckpointState),
}

#[derive(Clone)]
pub struct PostgresProjectionWorker {
    pool: PgPool,
    projection_name: String,
    page_size: i64,
}

impl fmt::Debug for PostgresProjectionWorker {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PostgresProjectionWorker")
            .field("pool", &"[POSTGRESQL POOL]")
            .field("projection_name", &self.projection_name)
            .field("page_size", &self.page_size)
            .finish()
    }
}
