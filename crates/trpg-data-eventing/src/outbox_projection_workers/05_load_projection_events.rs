
async fn load_projection_events(
    pool: &PgPool,
    campaign_id: &str,
    stream_id: &str,
    after_version: i64,
    limit: i64,
) -> Result<Vec<CanonicalReplayEvent>, EventWorkerError> {
    let rows = sqlx::query_as::<_, ProjectionEventRow>(
        r#"
        SELECT sequence, stream_version, stream_id, event_type,
               event_schema_version, campaign_id, expected_version,
               authority_mode,
               authenticated_actor_id, authenticated_actor_role,
               authenticated_actor_origin, resource_type, resource_id,
               authority_contract_id, authority_owner, command_id,
               idempotency_key, idempotency_operation,
               authority_contract_version, visibility_label,
               visibility_subject, fact_provenance_kind AS provenance_kind,
               fact_provenance_reference AS provenance_reference,
               fact_recorded_by AS provenance_recorded_by, correlation_id,
               causation_id, trace_id, payload_json AS payload,
               recorded_at, event_integrity_hash, request_hash,
               request_hash_source, integrity_status,
               payload_integrity_source
          FROM public.event_store
         WHERE campaign_id = $1
           AND stream_id = $2
           AND stream_version > $3
           AND integrity_status = 'verified_hmac'
           AND request_hash_source = 'formal_commit'
           AND event_integrity_hash IS NOT NULL
           AND payload_json ? 'protected_payload'
         ORDER BY stream_version
         LIMIT $4
        "#,
    )
    .bind(campaign_id)
    .bind(stream_id)
    .bind(after_version)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(|_| EventWorkerError::Database("load_projection_page"))?;
    Ok(rows.into_iter().map(Into::into).collect())
}

fn validate_prepared_page(
    projection_name: &str,
    page: &ProjectionPage,
) -> Result<Vec<String>, EventWorkerError> {
    if page.start.projection_name != projection_name
        || page.target.projection_name != projection_name
        || page.start.campaign_id != page.target.campaign_id
        || page.start.stream_id != page.target.stream_id
        || page.target.version < page.start.version
        || page.target.last_event_sequence < page.start.last_event_sequence
    {
        return Err(EventWorkerError::CheckpointIdentityMismatch);
    }
    let mut hasher = CanonicalProjectionHasher::resume(page.start.projection_hash.clone())?;
    let mut expected_version = page.start.version.saturating_add(1);
    let mut previous_sequence = page.start.last_event_sequence;
    let mut hashes = Vec::with_capacity(page.events.len());
    for event in &page.events {
        if event.campaign_id != page.start.campaign_id
            || event.stream_id != page.start.stream_id
            || event.sequence <= previous_sequence
        {
            return Err(EventWorkerError::CheckpointIdentityMismatch);
        }
        if event.stream_version != expected_version {
            return Err(EventWorkerError::ProjectionStreamGap {
                expected: expected_version,
                actual: event.stream_version,
            });
        }
        hasher.apply(event)?;
        hashes.push(hasher.projection_hash().to_owned());
        previous_sequence = event.sequence;
        expected_version = expected_version.saturating_add(1);
    }
    let Some(last) = page.events.last() else {
        return Err(EventWorkerError::CheckpointIdentityMismatch);
    };
    if page.target.version != last.stream_version
        || page.target.last_event_sequence != last.sequence
        || page.target.projection_hash != hasher.projection_hash()
    {
        return Err(EventWorkerError::CheckpointIdentityMismatch);
    }
    Ok(hashes)
}

fn validate_stream_scope(campaign_id: &str, stream_id: &str) -> Result<(), EventWorkerError> {
    if campaign_id.trim().is_empty()
        || stream_id.trim().is_empty()
        || campaign_id.len() > 256
        || stream_id.len() > 256
    {
        Err(EventWorkerError::Configuration("invalid_stream_scope"))
    } else {
        Ok(())
    }
}

fn valid_worker_id(worker_id: &str) -> bool {
    !worker_id.trim().is_empty()
        && worker_id.len() <= 128
        && worker_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

static CLAIM_TOKEN_SEQUENCE: AtomicU64 = AtomicU64::new(1);

fn next_claim_token_prefix(worker_id: &str) -> Result<String, EventWorkerError> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| EventWorkerError::Configuration("system_clock_before_unix_epoch"))?
        .as_nanos();
    let sequence = CLAIM_TOKEN_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let mut digest = Sha256::new();
    digest.update(b"trpg-outbox-claim-token-v1");
    digest.update((worker_id.len() as u64).to_be_bytes());
    digest.update(worker_id.as_bytes());
    digest.update(timestamp.to_be_bytes());
    digest.update(std::process::id().to_be_bytes());
    digest.update(sequence.to_be_bytes());
    Ok(format!("claim-sha256:{:x}", digest.finalize()))
}

fn duration_milliseconds(duration: Duration) -> Option<i64> {
    i64::try_from(duration.as_millis()).ok()
}

fn require_owned_claim(rows_affected: u64) -> Result<(), EventWorkerError> {
    if rows_affected == 1 {
        Ok(())
    } else {
        Err(EventWorkerError::ClaimLost)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exponential_backoff_is_bounded() {
        let policy = OutboxLeasePolicy {
            lease_duration: Duration::from_secs(30),
            initial_backoff: Duration::from_secs(2),
            maximum_backoff: Duration::from_secs(10),
            maximum_attempts: 5,
        };
        assert_eq!(policy.backoff_for_attempt(1), Duration::from_secs(2));
        assert_eq!(policy.backoff_for_attempt(2), Duration::from_secs(4));
        assert_eq!(policy.backoff_for_attempt(3), Duration::from_secs(8));
        assert_eq!(policy.backoff_for_attempt(4), Duration::from_secs(10));
        assert_eq!(policy.backoff_for_attempt(100), Duration::from_secs(10));
    }

    #[test]
    fn worker_identifiers_cannot_inject_transport_metadata() {
        assert!(valid_worker_id("outbox-worker_01"));
        assert!(!valid_worker_id("outbox worker"));
        assert!(!valid_worker_id("outbox\nworker"));
    }

    #[test]
    fn claim_tokens_are_unique_and_worker_scoped() {
        let first = next_claim_token_prefix("outbox-worker_01").unwrap();
        let second = next_claim_token_prefix("outbox-worker_01").unwrap();
        let peer = next_claim_token_prefix("outbox-worker_02").unwrap();
        assert_ne!(first, second);
        assert_ne!(second, peer);
        assert!(first.starts_with("claim-sha256:"));
    }
}
