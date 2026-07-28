struct PrimaryCommitEventBindings<'a> {
    commit_id: String,
    formal_campaign_id: String,
    formal_stream_id: String,
    formal_idempotency_key: String,
    formal_expected_version: i64,
    request_hash: String,
    first_event_sequence: i64,
    last_event_sequence: i64,
    first_stream_version: i64,
    last_stream_version: i64,
    audit: &'a AuditRecord,
}

impl PostgresCanonicalStore {
    async fn verify_primary_commit_events(
        &self,
        bindings: PrimaryCommitEventBindings<'_>,
    ) -> Result<(), CanonicalStoreError> {
        let PrimaryCommitEventBindings {
            commit_id,
            formal_campaign_id,
            formal_stream_id,
            formal_idempotency_key,
            formal_expected_version,
            request_hash,
            first_event_sequence,
            last_event_sequence,
            first_stream_version,
            last_stream_version,
            audit,
        } = bindings;
        let event_rows = sqlx::query(
            r#"
            SELECT event.sequence, event.stream_version, event.event_type,
                   event.command_id, event.idempotency_key,
                   event.expected_version, event.authority_mode,
                   event.authority_contract_version,
                   event.visibility_label, event.fact_provenance_kind,
                   event.fact_provenance_reference, event.fact_recorded_by,
                   event.correlation_id, event.causation_id,
                   event.payload_json, event.campaign_id,
                   event.authenticated_actor_id,
                   event.authenticated_actor_role,
                   event.authenticated_actor_origin,
                   event.resource_type, event.resource_id,
                   event.authority_contract_id, event.authority_owner,
                   event.visibility_subject, event.trace_id,
                   event.event_integrity_hash, event.stream_id,
                   event.event_schema_version, event.idempotency_operation,
                   event.request_hash, event.request_hash_source,
                   event.integrity_status, event.payload_integrity_source,
                   event.payload_ciphertext, event.payload_key_reference,
                   event.payload_nonce, event.data_subject_id,
                   event.projection_targets,
                   event.recorded_at, event.event_integrity_version,
                   event.derived_source_event_sequence,
                   event.derived_snapshot_id, event.derived_chunk_id,
                   event.derived_content_hash, event.derived_source_type,
                   event.derived_copyright_status, event.derived_allowed_use,
                   event.derived_embedding_model,
                   event.derived_embedding_dimensions,
                   event.derived_embedding_hash, event.deletion_job_id,
                   event.deletion_subject_id, event.deletion_requested_by,
                   event.deletion_retention_policy,
                   outbox.event_id AS outbox_event_id,
                   outbox.event_sequence AS outbox_event_sequence,
                   outbox.nats_subject AS outbox_nats_subject,
                   outbox.idempotency_key AS outbox_idempotency_key,
                   outbox.visibility_label AS outbox_visibility_label,
                   outbox.correlation_id AS outbox_correlation_id,
                   outbox.causation_id AS outbox_causation_id,
                   outbox.payload_json AS outbox_payload_json,
                   outbox.commit_id AS outbox_commit_id,
                   outbox.request_hash AS outbox_request_hash,
                   outbox.request_hash_source AS outbox_request_hash_source,
                   outbox.integrity_status AS outbox_integrity_status,
                   outbox.campaign_id AS outbox_campaign_id,
                   outbox.stream_id AS outbox_stream_id,
                   outbox.event_schema_version AS outbox_event_schema_version,
                   outbox.idempotency_operation AS outbox_idempotency_operation,
                   outbox.visibility_subject AS outbox_visibility_subject,
                   outbox.payload_ciphertext AS outbox_payload_ciphertext,
                   outbox.payload_key_reference AS outbox_payload_key_reference,
                   outbox.payload_nonce AS outbox_payload_nonce,
                   outbox.data_subject_id AS outbox_data_subject_id
              FROM event_store AS event
              JOIN event_outbox AS outbox
                ON outbox.event_sequence = event.sequence
             WHERE outbox.commit_id = $1
             ORDER BY event.stream_version, event.sequence
            "#,
        )
        .bind(&commit_id)
        .fetch_all(&self.primary)
        .await
        .map_err(|_| CanonicalStoreError::PrimaryWrite {
            operation: "verify_commit_events",
        })?;
        let expected_event_count = last_stream_version
            .checked_sub(first_stream_version)
            .and_then(|range| range.checked_add(1))
            .and_then(|count| usize::try_from(count).ok())
            .ok_or(CanonicalStoreError::IntegrityViolation(
                "invalid_primary_stream_range",
            ))?;
        if event_rows.len() != expected_event_count {
            return Err(CanonicalStoreError::IntegrityViolation(
                "primary_stream_event_count_mismatch",
            ));
        }
        let actual_range = event_rows
            .first()
            .zip(event_rows.last())
            .map(|(first, last)| {
                (
                    first.get::<i64, _>("sequence"),
                    last.get::<i64, _>("sequence"),
                    first.get::<i64, _>("stream_version"),
                    last.get::<i64, _>("stream_version"),
                )
            });
        if actual_range
            != Some((
                first_event_sequence,
                last_event_sequence,
                first_stream_version,
                last_stream_version,
            ))
        {
            return Err(CanonicalStoreError::IntegrityViolation(
                "primary_event_bounds_mismatch",
            ));
        }
        let mut event_hashes = Vec::with_capacity(event_rows.len());
        for (index, event) in event_rows.iter().enumerate() {
            let event_integrity_status: String = event.get("integrity_status");
            let outbox_integrity_status: String = event.get("outbox_integrity_status");
            let event_integrity_version: i32 = event.get("event_integrity_version");
            let event_sequence: i64 = event.get("sequence");
            let event_stream_version: i64 = event.get("stream_version");
            let event_campaign_id: String = event.get("campaign_id");
            let event_stream_id: String = event.get("stream_id");
            let event_idempotency_key: String = event.get("idempotency_key");
            let event_visibility_label: String = event.get("visibility_label");
            let event_visibility_subject: String = event.get("visibility_subject");
            let event_correlation_id: String = event.get("correlation_id");
            let event_causation_id: String = event.get("causation_id");
            let event_schema_version: i32 = event.get("event_schema_version");
            let event_idempotency_operation: String = event.get("idempotency_operation");
            let event_payload: Json<Value> = event.get("payload_json");
            let event_payload_ciphertext: Option<Vec<u8>> = event.get("payload_ciphertext");
            let event_payload_key_reference: Option<String> =
                event.get("payload_key_reference");
            let event_payload_nonce: Option<Vec<u8>> = event.get("payload_nonce");
            let event_data_subject_id: String = event.get("data_subject_id");
            let event_projection_targets: Json<Vec<PersistedCanonicalProjectionTarget>> =
                event.get("projection_targets");
            let event_projection_targets_json =
                serde_json::to_string(&event_projection_targets.0).map_err(|_| {
                    CanonicalStoreError::IntegrityViolation("event_projection_targets_invalid")
                })?;
            let expected_event_idempotency_key =
                format!("{}:{index:04}", formal_idempotency_key);

            if event.get::<String, _>("request_hash") != request_hash
                || event.get::<String, _>("outbox_request_hash") != request_hash
                || event.get::<String, _>("request_hash_source") != "formal_commit"
                || event.get::<String, _>("outbox_request_hash_source") != "formal_commit"
                || event_integrity_status != outbox_integrity_status
                || !matches!(
                    (event_integrity_status.as_str(), event_integrity_version),
                    ("verified_hmac", 2)
                        | ("verified_hmac", CURRENT_EVENT_INTEGRITY_VERSION)
                        | ("historical_unverified_hmac", 1)
                )
                || event_sequence != event.get::<i64, _>("outbox_event_id")
                || event_sequence != event.get::<i64, _>("outbox_event_sequence")
                || event.get::<String, _>("outbox_nats_subject") != crate::NATS_EVENTS_APPENDED
                || event.get::<String, _>("outbox_idempotency_key")
                    != format!("outbox:{event_idempotency_key}")
                || event_visibility_label != event.get::<String, _>("outbox_visibility_label")
                || event_correlation_id != event.get::<String, _>("outbox_correlation_id")
                || event_causation_id != event.get::<String, _>("outbox_causation_id")
                || event_payload.0 != event.get::<Json<Value>, _>("outbox_payload_json").0
                || event.get::<String, _>("outbox_commit_id") != commit_id
                || event_campaign_id != event.get::<String, _>("outbox_campaign_id")
                || event_stream_id != event.get::<String, _>("outbox_stream_id")
                || event_schema_version != event.get::<i32, _>("outbox_event_schema_version")
                || event_idempotency_operation
                    != event.get::<String, _>("outbox_idempotency_operation")
                || Some(event_visibility_subject.clone())
                    != event.get::<Option<String>, _>("outbox_visibility_subject")
                || event_payload_ciphertext
                    != event.get::<Option<Vec<u8>>, _>("outbox_payload_ciphertext")
                || event_payload_key_reference
                    != event.get::<Option<String>, _>("outbox_payload_key_reference")
                || event_payload_nonce
                    != event.get::<Option<Vec<u8>>, _>("outbox_payload_nonce")
                || event_data_subject_id != event.get::<String, _>("outbox_data_subject_id")
            {
                return Err(CanonicalStoreError::IntegrityViolation(
                    "canonical_event_outbox_binding_mismatch",
                ));
            }
            if event_campaign_id != formal_campaign_id
                || event_stream_id != formal_stream_id
                || event_idempotency_key != expected_event_idempotency_key
                || event.get::<i64, _>("expected_version") != formal_expected_version
                || event_stream_version != first_stream_version + index as i64
                || event_idempotency_operation != CANONICAL_IDEMPOTENCY_OPERATION
                || ((event_visibility_label != audit.visibility_label
                    || event_visibility_subject != audit.visibility_subject)
                    && !(audit.resource_type == "campaign_fork"
                        && audit.action == "write_official_state"
                        && event.get::<String, _>("event_type") == "CampaignForkMaterialized"))
                || event.get::<String, _>("fact_provenance_kind") != audit.provenance_kind
                || event.get::<String, _>("fact_provenance_reference")
                    != audit.provenance_reference
                || event.get::<String, _>("fact_recorded_by") != audit.provenance_recorded_by
                || event_correlation_id != audit.correlation_id
                || event_causation_id != audit.causation_id
                || event.get::<String, _>("trace_id") != audit.trace_id
                || event.get::<String, _>("resource_type") != audit.resource_type
                || event.get::<String, _>("resource_id") != audit.resource_id
            {
                return Err(CanonicalStoreError::IntegrityViolation(
                    "canonical_event_commit_audit_binding_mismatch",
                ));
            }
            let event_type: String = event.get("event_type");
            let payload_integrity_source: String = event.get("payload_integrity_source");
            let integrity_payload: Value = serde_json::from_str(&payload_integrity_source)
                .map_err(|_| {
                    CanonicalStoreError::IntegrityViolation("event_payload_json_invalid")
                })?;
            if integrity_payload != event_payload.0 {
                return Err(CanonicalStoreError::IntegrityViolation(
                    "event_payload_integrity_source_mismatch",
                ));
            }
            let stored_hash: Option<String> = event.get("event_integrity_hash");
            let expected_hash = if event_integrity_version == 1 {
                legacy_event_integrity_hash(
                    self.integrity_key(),
                    &request_hash,
                    index,
                    &event_type,
                    &payload_integrity_source,
                )
            } else {
                let authenticated_actor_origin = serde_json::to_string(
                    &event
                        .get::<Json<EventActorOriginWire>, _>("authenticated_actor_origin")
                        .0,
                )
                .map_err(|_| {
                    CanonicalStoreError::IntegrityViolation(
                        "authenticated_actor_origin_invalid",
                    )
                })?;
                let integrity_record = CanonicalEventIntegrityRecord {
                    sequence: event_sequence,
                    event_index: index,
                    stream_version: event_stream_version,
                    event_type,
                    command_id: event.get("command_id"),
                    idempotency_key: event_idempotency_key,
                    expected_version: event.get("expected_version"),
                    authority_mode: event.get("authority_mode"),
                    authority_contract_version: event.get("authority_contract_version"),
                    visibility_label: event_visibility_label,
                    provenance_kind: event.get("fact_provenance_kind"),
                    provenance_reference: event.get("fact_provenance_reference"),
                    provenance_recorded_by: event.get("fact_recorded_by"),
                    correlation_id: event_correlation_id,
                    causation_id: event_causation_id,
                    campaign_id: event_campaign_id,
                    authenticated_actor_id: event.get("authenticated_actor_id"),
                    authenticated_actor_role: event.get("authenticated_actor_role"),
                    authenticated_actor_origin,
                    resource_type: event.get("resource_type"),
                    resource_id: event.get("resource_id"),
                    authority_contract_id: event.get("authority_contract_id"),
                    authority_owner: event.get("authority_owner"),
                    visibility_subject: event_visibility_subject,
                    trace_id: event.get("trace_id"),
                    stream_id: event_stream_id,
                    event_schema_version,
                    idempotency_operation: event_idempotency_operation,
                    request_hash: request_hash.clone(),
                    request_hash_source: event.get("request_hash_source"),
                    integrity_status: event_integrity_status,
                    payload_integrity_source,
                    payload_ciphertext: event_payload_ciphertext,
                    payload_key_reference: event_payload_key_reference,
                    payload_nonce: event_payload_nonce,
                    data_subject_id: event_data_subject_id,
                    projection_targets_json: event_projection_targets_json,
                    recorded_at_micros: event
                        .get::<DateTime<Utc>, _>("recorded_at")
                        .timestamp_micros(),
                    derived_source_event_sequence: event.get("derived_source_event_sequence"),
                    derived_snapshot_id: event.get("derived_snapshot_id"),
                    derived_chunk_id: event.get("derived_chunk_id"),
                    derived_content_hash: event.get("derived_content_hash"),
                    derived_source_type: event.get("derived_source_type"),
                    derived_copyright_status: event.get("derived_copyright_status"),
                    derived_allowed_use: event.get("derived_allowed_use"),
                    derived_embedding_model: event.get("derived_embedding_model"),
                    derived_embedding_dimensions: event.get("derived_embedding_dimensions"),
                    derived_embedding_hash: event.get("derived_embedding_hash"),
                    deletion_job_id: event.get("deletion_job_id"),
                    deletion_subject_id: event.get("deletion_subject_id"),
                    deletion_requested_by: event.get("deletion_requested_by"),
                    deletion_retention_policy: event.get("deletion_retention_policy"),
                };
                match event_integrity_version {
                    2 => event_integrity_hash_v2(self.integrity_key(), &integrity_record),
                    3 => event_integrity_hash_v3(self.integrity_key(), &integrity_record),
                    _ => {
                        return Err(CanonicalStoreError::IntegrityViolation(
                            "event_integrity_version_invalid",
                        ))
                    }
                }
            };
            if stored_hash.as_deref() != Some(expected_hash.as_str()) {
                return Err(CanonicalStoreError::IntegrityViolation(
                    "canonical_event_hmac_mismatch",
                ));
            }
            event_hashes.push(expected_hash);
        }
        let event_batch_hash = sha256_fields(&event_hashes);
        let audit_count: i64 = sqlx::query_scalar(
            r#"
            SELECT count(*)
              FROM canonical_audit_log audit
              JOIN formal_commits formal ON formal.audit_sequence = audit.sequence
             WHERE formal.commit_id = $1
               AND audit.commit_id = formal.commit_id
               AND audit.event_batch_hash = $2
               AND audit.witness_prepare_sequence = formal.witness_prepare_sequence
               AND audit.witness_prepare_hash = formal.witness_prepare_hash
            "#,
        )
        .bind(&commit_id)
        .bind(&event_batch_hash)
        .fetch_one(&self.primary)
        .await
        .map_err(|_| CanonicalStoreError::PrimaryWrite {
            operation: "verify_commit_audit",
        })?;
        let outbox_count: i64 = sqlx::query_scalar(
            r#"
            SELECT count(*) FROM event_outbox
             WHERE commit_id = $1
            "#,
        )
        .bind(&commit_id)
        .fetch_one(&self.primary)
        .await
        .map_err(|_| CanonicalStoreError::PrimaryWrite {
            operation: "verify_commit_outbox",
        })?;
        if audit_count != 1 || outbox_count != event_rows.len() as i64 {
            return Err(CanonicalStoreError::IntegrityViolation(
                "primary_atomic_commit_components_incomplete",
            ));
        }
        Ok(())
    }
}
