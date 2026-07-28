
async fn load_committed_events(
    pool: &PgPool,
    payload_cipher: &PayloadCipher,
    persisted: &PersistedCommit,
) -> Result<Vec<CanonicalCommittedEvent>, CanonicalStoreError> {
    let rows = sqlx::query(
        r#"
        SELECT event.sequence, event.stream_version, event.event_type,
               event.payload_json, event.command_id, event.idempotency_key,
               event.recorded_at, event.campaign_id, event.stream_id,
               event.data_subject_id, event.payload_key_reference,
               event.event_integrity_hash
          FROM public.event_store AS event
          JOIN public.event_outbox AS outbox
            ON outbox.event_sequence = event.sequence
         WHERE outbox.commit_id = $1
         ORDER BY event.stream_version
        "#,
    )
    .bind(&persisted.commit_id)
    .fetch_all(pool)
    .await
    .map_err(|_| CanonicalStoreError::PrimaryWrite {
        operation: "load_committed_events",
    })?;
    let expected_count = persisted
        .last_stream_version
        .checked_sub(persisted.first_stream_version)
        .and_then(|delta| delta.checked_add(1))
        .and_then(|count| usize::try_from(count).ok())
        .ok_or(CanonicalStoreError::IntegrityViolation(
            "committed_event_range_invalid",
        ))?;
    if rows.len() != expected_count {
        return Err(CanonicalStoreError::IntegrityViolation(
            "committed_event_count_mismatch",
        ));
    }
    let mut events = Vec::with_capacity(rows.len());
    for row in rows {
        let protected_payload: Json<Value> = row.get("payload_json");
        let campaign_id: String = row.get("campaign_id");
        let stream_id: String = row.get("stream_id");
        let command_id: String = row.get("command_id");
        let event_type: String = row.get("event_type");
        let data_subject_id: String = row.get("data_subject_id");
        let payload_key_reference: String = row.get("payload_key_reference");
        let subject_cipher = load_subject_payload_cipher(
            pool,
            payload_cipher,
            &data_subject_id,
            &payload_key_reference,
        )
        .await?;
        let resolved_cipher = subject_cipher.as_ref().unwrap_or(payload_cipher);
        let plaintext = resolved_cipher
            .decrypt_json(
                &protected_payload.0,
                &[&campaign_id, &stream_id, &command_id, &event_type],
            )
            .map_err(|_| {
                CanonicalStoreError::IntegrityViolation("committed_event_decryption_failed")
            })?;
        let payload: Value = serde_json::from_slice(plaintext.as_bytes()).map_err(|_| {
            CanonicalStoreError::IntegrityViolation("committed_event_payload_invalid")
        })?;
        let recorded_at: DateTime<Utc> = row.get("recorded_at");
        events.push(CanonicalCommittedEvent {
            sequence: u64::try_from(row.get::<i64, _>("sequence")).map_err(|_| {
                CanonicalStoreError::IntegrityViolation("committed_event_sequence_invalid")
            })?,
            stream_version: u64::try_from(row.get::<i64, _>("stream_version")).map_err(|_| {
                CanonicalStoreError::IntegrityViolation("committed_stream_version_invalid")
            })?,
            event_type,
            payload_json: serde_json::to_string(&payload).map_err(|_| {
                CanonicalStoreError::IntegrityViolation("committed_event_payload_invalid")
            })?,
            command_id,
            idempotency_key: row.get("idempotency_key"),
            occurred_at_unix_ms: u64::try_from(recorded_at.timestamp_millis()).map_err(|_| {
                CanonicalStoreError::IntegrityViolation("committed_event_timestamp_invalid")
            })?,
            event_integrity_hash: row
                .try_get::<Option<String>, _>("event_integrity_hash")
                .map_err(|_| {
                    CanonicalStoreError::IntegrityViolation("committed_event_hash_invalid")
                })?
                .filter(|hash| valid_hmac_hash(hash))
                .ok_or(CanonicalStoreError::IntegrityViolation(
                    "committed_event_hash_invalid",
                ))?,
        });
    }
    let range_matches = events
        .first()
        .zip(events.last())
        .map(|(first, last)| {
            first.sequence == persisted.first_event_sequence as u64
                && last.sequence == persisted.last_event_sequence as u64
                && first.stream_version == persisted.first_stream_version as u64
                && last.stream_version == persisted.last_stream_version as u64
        })
        .unwrap_or(false);
    if !range_matches {
        return Err(CanonicalStoreError::IntegrityViolation(
            "committed_event_range_mismatch",
        ));
    }
    Ok(events)
}

/// Production replay query shared by the canonical store and migration
/// verification. Only encrypted events bound to a formal commit and verified
/// HMAC are eligible; unverified history remains in Event Store for audit but
/// cannot enter production replay or downstream read models.
pub async fn load_canonical_replay_page(
    pool: &PgPool,
    payload_cipher: &PayloadCipher,
    campaign_id: &str,
    after_sequence: i64,
    limit: i64,
) -> Result<Vec<CanonicalReplayEvent>, CanonicalStoreError> {
    if campaign_id.trim().is_empty()
        || campaign_id.len() > 128
        || !campaign_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return Err(CanonicalStoreError::Validation("campaign_id_required"));
    }
    if after_sequence < 0 || !(1..=500).contains(&limit) {
        return Err(CanonicalStoreError::Validation(
            "invalid_replay_page_request",
        ));
    }
    let rows = sqlx::query(
        r#"
        SELECT sequence, stream_version, stream_id, event_type, event_schema_version, campaign_id,
               expected_version, authority_mode,
               authenticated_actor_id, authenticated_actor_role,
               authenticated_actor_origin, resource_type, resource_id,
               authority_contract_id, authority_owner, command_id,
               idempotency_key, idempotency_operation, authority_contract_version,
               visibility_label, visibility_subject,
               fact_provenance_kind, fact_provenance_reference,
               fact_recorded_by, correlation_id, causation_id, trace_id,
               payload_json, recorded_at, event_integrity_hash,
               request_hash, request_hash_source, integrity_status,
               payload_integrity_source, data_subject_id, payload_key_reference,
               event_integrity_version
          FROM event_store
         WHERE campaign_id = $1 AND sequence > $2
           AND integrity_status = 'verified_hmac'
           AND request_hash_source = 'formal_commit'
           AND event_integrity_hash IS NOT NULL
           AND payload_json ? 'protected_payload'
           AND (
               data_subject_id = 'not_applicable'
               OR EXISTS (
                   SELECT 1 FROM privacy_subject_keys AS subject_key
                    WHERE subject_key.subject_id = event_store.data_subject_id
                      AND subject_key.key_reference = event_store.payload_key_reference
                      AND subject_key.wrapped_key IS NOT NULL
                      AND subject_key.destroyed_at IS NULL
               )
           )
         ORDER BY sequence
         LIMIT $3
        "#,
    )
    .bind(campaign_id)
    .bind(after_sequence)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(|_| CanonicalStoreError::PrimaryWrite {
        operation: "load_replay_page",
    })?;

    let mut events = Vec::with_capacity(rows.len());
    for row in rows {
        let protected_payload: Json<Value> = row.get("payload_json");
        let event_type: String = row.get("event_type");
        let stream_id: String = row.get("stream_id");
        let stored_campaign_id: String = row.get("campaign_id");
        let command_id: String = row.get("command_id");
        let integrity_status: String = row.get("integrity_status");
        if protected_payload.0.get("protected_payload").is_none() {
            return Err(CanonicalStoreError::IntegrityViolation(
                "verified_event_payload_not_encrypted",
            ));
        }
        let data_subject_id: String = row.get("data_subject_id");
        let payload_key_reference: String = row.get("payload_key_reference");
        let subject_cipher = load_subject_payload_cipher(
            pool,
            payload_cipher,
            &data_subject_id,
            &payload_key_reference,
        )
        .await?;
        let resolved_cipher = subject_cipher.as_ref().unwrap_or(payload_cipher);
        let plaintext = resolved_cipher
            .decrypt_json(
                &protected_payload.0,
                &[&stored_campaign_id, &stream_id, &command_id, &event_type],
            )
            .map_err(|_| {
                CanonicalStoreError::IntegrityViolation("event_payload_decryption_failed")
            })?;
        let payload = serde_json::from_slice(plaintext.as_bytes())
            .map_err(|_| CanonicalStoreError::IntegrityViolation("event_payload_json_invalid"))?;
        let upcasted = crate::persistence::EventPayloadUpcaster::canonical()
            .upcast(&event_type, row.get("event_schema_version"), payload)
            .map_err(|_| CanonicalStoreError::IntegrityViolation("event_schema_version_unknown"))?;
        let event_integrity_hash: Option<String> = row.get("event_integrity_hash");
        let request_hash: String = row.get("request_hash");
        let request_hash_source: String = row.get("request_hash_source");
        if !replay_integrity_metadata_is_valid(
            &integrity_status,
            &request_hash_source,
            &request_hash,
            event_integrity_hash.as_deref(),
            row.get("event_integrity_version"),
        ) {
            return Err(CanonicalStoreError::IntegrityViolation(
                "event_integrity_metadata_invalid",
            ));
        }
        events.push(CanonicalReplayEvent {
            sequence: row.get("sequence"),
            stream_version: row.get("stream_version"),
            stream_id,
            event_type,
            event_schema_version: upcasted.event_schema_version,
            campaign_id: stored_campaign_id,
            expected_version: row.get("expected_version"),
            authority_mode: row.get("authority_mode"),
            authenticated_actor_id: row.get("authenticated_actor_id"),
            authenticated_actor_role: row.get("authenticated_actor_role"),
            authenticated_actor_origin: row
                .get::<Json<EventActorOriginWire>, _>("authenticated_actor_origin")
                .0,
            resource_type: row.get("resource_type"),
            resource_id: row.get("resource_id"),
            authority_contract_id: row.get("authority_contract_id"),
            authority_owner: row.get("authority_owner"),
            command_id,
            idempotency_key: row.get("idempotency_key"),
            idempotency_operation: row.get("idempotency_operation"),
            authority_contract_version: row.get("authority_contract_version"),
            visibility_label: row.get("visibility_label"),
            visibility_subject: row.get("visibility_subject"),
            provenance_kind: row.get("fact_provenance_kind"),
            provenance_reference: row.get("fact_provenance_reference"),
            provenance_recorded_by: row.get("fact_recorded_by"),
            correlation_id: row.get("correlation_id"),
            causation_id: row.get("causation_id"),
            trace_id: row.get("trace_id"),
            payload: upcasted.payload,
            recorded_at: row.get("recorded_at"),
            event_integrity_hash,
            request_hash,
            request_hash_source,
            integrity_status,
            payload_integrity_source: row.get("payload_integrity_source"),
        });
    }
    Ok(events)
}

async fn load_existing_commit_in_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    commit_id: &str,
    campaign_id: &str,
    stream_id: &str,
    idempotency_key: &str,
) -> Result<Option<PersistedCommit>, CanonicalStoreError> {
    let row = sqlx::query(
        r#"
        SELECT commit_id,
               (response_payload->>'first_event_sequence')::bigint AS first_event_sequence,
               result_event_sequence AS last_event_sequence,
               (response_payload->>'first_stream_version')::bigint AS first_stream_version,
               (response_payload->>'last_stream_version')::bigint AS last_stream_version,
               audit_sequence,
               witness_prepare_sequence, witness_prepare_hash
          FROM formal_commits
         WHERE commit_id = $1
            OR (
                campaign_id = $2
                AND stream_id = $3
                AND idempotency_operation = 'canonical_commit'
                AND idempotency_key = $4
            )
         ORDER BY CASE WHEN commit_id = $1 THEN 0 ELSE 1 END LIMIT 1
        "#,
    )
    .bind(commit_id)
    .bind(campaign_id)
    .bind(stream_id)
    .bind(idempotency_key)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(|_| CanonicalStoreError::PrimaryWrite {
        operation: "load_idempotent_commit",
    })?;
    Ok(row.map(|row| persisted_from_row(&row)))
}

async fn load_request_hash_in_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    commit_id: &str,
) -> Result<String, CanonicalStoreError> {
    sqlx::query_scalar("SELECT request_hash FROM formal_commits WHERE commit_id = $1")
        .bind(commit_id)
        .fetch_one(&mut **transaction)
        .await
        .map_err(|_| CanonicalStoreError::PrimaryWrite {
            operation: "load_idempotent_request_hash",
        })
}

fn persisted_from_row(row: &sqlx::postgres::PgRow) -> PersistedCommit {
    PersistedCommit {
        commit_id: row.get("commit_id"),
        first_event_sequence: row.get("first_event_sequence"),
        last_event_sequence: row.get("last_event_sequence"),
        first_stream_version: row.get("first_stream_version"),
        last_stream_version: row.get("last_stream_version"),
        audit_sequence: row.get("audit_sequence"),
        witness_prepare_sequence: row.get("witness_prepare_sequence"),
        witness_prepare_hash: row.get("witness_prepare_hash"),
    }
}

fn witness_from_row(row: &sqlx::postgres::PgRow) -> WitnessRecord {
    WitnessRecord {
        sequence: row.get("sequence"),
        commit_id: row.get("commit_id"),
        phase: row.get("phase"),
        request_hash: row.get("primary_request_hash"),
        first_sequence: row.get("primary_first_sequence"),
        last_sequence: row.get("primary_last_sequence"),
        reason: row.get("reason"),
        key_id: row.get("integrity_key_id"),
        previous_hash: row.get("previous_hash"),
        record_hash: row.get("record_hash"),
    }
}
