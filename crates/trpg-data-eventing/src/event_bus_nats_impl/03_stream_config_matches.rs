
fn stream_config_matches(actual: &StreamConfig, desired: &StreamConfig) -> bool {
    let mut actual_subjects = actual.subjects.clone();
    let mut desired_subjects = desired.subjects.clone();
    let mut actual_application_metadata = actual.metadata.clone();
    actual_application_metadata.retain(|key, _| !is_nats_server_versioning_metadata(key));
    actual_subjects.sort_unstable();
    desired_subjects.sort_unstable();
    actual.name == desired.name
        && actual.description == desired.description
        && actual_subjects == desired_subjects
        && actual.max_bytes == desired.max_bytes
        && actual.max_messages == desired.max_messages
        && actual.max_messages_per_subject == desired.max_messages_per_subject
        && actual.discard == desired.discard
        && actual.discard_new_per_subject == desired.discard_new_per_subject
        && actual.retention == desired.retention
        && actual.max_consumers == desired.max_consumers
        && actual.max_age == desired.max_age
        && actual.max_message_size == desired.max_message_size
        && actual.duplicate_window == desired.duplicate_window
        && actual.storage == desired.storage
        && actual.num_replicas == desired.num_replicas
        && actual.no_ack == desired.no_ack
        && actual.template_owner == desired.template_owner
        && actual.sealed == desired.sealed
        && actual.allow_rollup == desired.allow_rollup
        && actual.deny_delete == desired.deny_delete
        && actual.deny_purge == desired.deny_purge
        && actual.republish == desired.republish
        && actual.allow_direct == desired.allow_direct
        && actual.mirror_direct == desired.mirror_direct
        && actual.mirror == desired.mirror
        && actual.sources == desired.sources
        // NATS 2.12+ annotates assets with its own API/version metadata when
        // returning them. Those three exact keys are server-owned response
        // data, not mutable application stream policy. Keep every other
        // metadata key fail-closed so operator or application drift is still
        // rejected.
        && actual_application_metadata == desired.metadata
        && actual.subject_transform == desired.subject_transform
        && actual.compression == desired.compression
        && actual.consumer_limits == desired.consumer_limits
        && actual.first_sequence == desired.first_sequence
        && actual.placement == desired.placement
        && actual.persist_mode == desired.persist_mode
}

fn is_nats_server_versioning_metadata(key: &str) -> bool {
    matches!(key, "_nats.req.level" | "_nats.ver" | "_nats.level")
}

fn event_envelope(
    row: &OutboxClaim,
) -> Result<EventEnvelopeWire<serde_json::Value>, JetStreamOutboxError> {
    let authenticated_actor_origin = row.authenticated_actor_origin.0.clone();
    let occurred_at_unix_ms = u64::try_from(row.recorded_at.timestamp_millis())
        .map_err(|_| JetStreamOutboxError::InvalidOutboxPayload)?;
    Ok(EventEnvelopeWire {
        schema_version: EVENT_ENVELOPE_WIRE_SCHEMA_VERSION,
        event_schema_version: u32::try_from(row.event_schema_version)
            .map_err(|_| JetStreamOutboxError::InvalidOutboxPayload)?,
        sequence: u64::try_from(row.event_sequence)
            .map_err(|_| JetStreamOutboxError::InvalidOutboxPayload)?,
        stream_id: row.stream_id.clone(),
        stream_version: u64::try_from(row.stream_version)
            .map_err(|_| JetStreamOutboxError::InvalidOutboxPayload)?,
        event_type: row.event_type.clone(),
        campaign_id: row.campaign_id.clone(),
        authenticated_actor_id: row.authenticated_actor_id.clone(),
        authenticated_actor_role: row.authenticated_actor_role.clone(),
        authenticated_actor_origin,
        resource_campaign_id: row.campaign_id.clone(),
        resource_type: row.resource_type.clone(),
        resource_id: row.resource_id.clone(),
        authority_contract_id: row.authority_contract_id.clone(),
        authority_owner: row.authority_owner.clone(),
        command_id: row.command_id.clone(),
        idempotency_key: row.event_idempotency_key.clone(),
        authority_contract_version: u64::try_from(row.authority_contract_version)
            .map_err(|_| JetStreamOutboxError::InvalidOutboxPayload)?,
        visibility_label: row.visibility_label.clone(),
        visibility_subject: (row.visibility_subject != "not_applicable")
            .then(|| row.visibility_subject.clone()),
        provenance_kind: row.provenance_kind.clone(),
        provenance_reference: row.provenance_reference.clone(),
        provenance_recorded_by: row.provenance_recorded_by.clone(),
        correlation_id: row.correlation_id.clone(),
        causation_id: row.causation_id.clone(),
        trace_id: row.trace_id.clone(),
        occurred_at_unix_ms,
        payload: row.payload_json.clone(),
        request_hash_source: row.request_hash_source.clone(),
        integrity_status: row.integrity_status.clone(),
        integrity_hash: row.event_integrity_hash.clone(),
    })
}

fn validate_worker_id(worker_id: &str) -> Result<(), JetStreamOutboxError> {
    if worker_id.trim().is_empty()
        || worker_id.len() > 128
        || !worker_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        Err(JetStreamOutboxError::Configuration("invalid_worker_id"))
    } else {
        Ok(())
    }
}

fn map_worker_error(error: EventWorkerError) -> JetStreamOutboxError {
    match error {
        EventWorkerError::Configuration(reason) => JetStreamOutboxError::Configuration(reason),
        EventWorkerError::InvalidOutboxPayload => JetStreamOutboxError::InvalidOutboxPayload,
        EventWorkerError::ClaimLost => JetStreamOutboxError::Database("outbox_claim_lost"),
        EventWorkerError::Database(operation) => JetStreamOutboxError::Database(operation),
        EventWorkerError::ProjectionHash(_)
        | EventWorkerError::ProjectionSerialization
        | EventWorkerError::ProjectionReadModelConflict
        | EventWorkerError::ProjectionStreamGap { .. }
        | EventWorkerError::CheckpointIdentityMismatch
        | EventWorkerError::CheckpointConflict { .. } => {
            JetStreamOutboxError::Database("unexpected_projection_worker_error")
        }
    }
}

fn validate_nats_url(nats_url: &str) -> Result<(bool, bool), JetStreamOutboxError> {
    let url = Url::parse(nats_url)
        .map_err(|_| JetStreamOutboxError::Configuration("invalid_nats_url"))?;
    let host = url
        .host_str()
        .ok_or(JetStreamOutboxError::Configuration("nats_host_required"))?;
    let local = matches!(host, "localhost" | "127.0.0.1" | "::1");
    let tls = url.scheme() == "tls";
    if !matches!(url.scheme(), "nats" | "tls") {
        return Err(JetStreamOutboxError::Configuration(
            "unsupported_nats_scheme",
        ));
    }
    if !local && !tls {
        return Err(JetStreamOutboxError::Configuration(
            "remote_nats_requires_tls",
        ));
    }
    Ok((local, tls))
}

fn nats_url_credentials(nats_url: &str) -> Result<Option<(String, String)>, JetStreamOutboxError> {
    let url = Url::parse(nats_url)
        .map_err(|_| JetStreamOutboxError::Configuration("invalid_nats_url"))?;
    match (url.username(), url.password()) {
        ("", None) => Ok(None),
        (username, Some(password)) if !username.is_empty() && !password.is_empty() => {
            let username = percent_decode_str(username)
                .decode_utf8()
                .map_err(|_| {
                    JetStreamOutboxError::Configuration("invalid_nats_url_credentials_encoding")
                })?
                .into_owned();
            let password = percent_decode_str(password)
                .decode_utf8()
                .map_err(|_| {
                    JetStreamOutboxError::Configuration("invalid_nats_url_credentials_encoding")
                })?
                .into_owned();
            if username.is_empty() || password.is_empty() {
                return Err(JetStreamOutboxError::Configuration(
                    "nats_url_credentials_incomplete",
                ));
            }
            Ok(Some((username, password)))
        }
        _ => Err(JetStreamOutboxError::Configuration(
            "nats_url_credentials_incomplete",
        )),
    }
}

fn nats_endpoint_without_userinfo(nats_url: &str) -> Result<String, JetStreamOutboxError> {
    let mut url = Url::parse(nats_url)
        .map_err(|_| JetStreamOutboxError::Configuration("invalid_nats_url"))?;
    url.set_username("")
        .map_err(|_| JetStreamOutboxError::Configuration("invalid_nats_url"))?;
    url.set_password(None)
        .map_err(|_| JetStreamOutboxError::Configuration("invalid_nats_url"))?;
    Ok(url.into())
}

#[cfg(test)]
fn outbox_integrity_metadata_is_valid(
    integrity_status: &str,
    request_hash_source: &str,
    has_integrity_hash: bool,
    has_commit_id: bool,
) -> bool {
    match (integrity_status, request_hash_source) {
        ("verified_hmac", "formal_commit") => has_integrity_hash && has_commit_id,
        _ => false,
    }
}

fn insert_outbox_header(
    headers: &mut HeaderMap,
    name: &'static str,
    value: &str,
) -> Result<(), JetStreamOutboxError> {
    // async-nats' infallible `From<&str>` implementation asserts on CR/LF.
    // Historical rows can predate today's database validators, so parse every
    // dynamic value through the fallible API and keep failure row-scoped.
    let value =
        HeaderValue::from_str(value).map_err(|_| JetStreamOutboxError::InvalidOutboxPayload)?;
    headers.insert(name, value);
    Ok(())
}

fn outbox_headers(row: &OutboxClaim) -> Result<HeaderMap, JetStreamOutboxError> {
    let mut headers = HeaderMap::new();
    // JetStream duplicate detection is global to the NATS stream. Bind its
    // message id to the complete persisted idempotency scope so equal client
    // keys in distinct campaign/resource streams cannot suppress one another.
    insert_outbox_header(&mut headers, "Nats-Msg-Id", &nats_message_id(row))?;
    insert_outbox_header(&mut headers, "Trpg-Idempotency-Key", &row.idempotency_key)?;
    insert_outbox_header(&mut headers, "Trpg-Stream-Id", &row.stream_id)?;
    insert_outbox_header(
        &mut headers,
        "Trpg-Idempotency-Operation",
        &row.idempotency_operation,
    )?;
    if let Some(commit_id) = &row.commit_id {
        insert_outbox_header(&mut headers, "Trpg-Commit-Id", commit_id)?;
    }
    insert_outbox_header(&mut headers, "Trpg-Correlation-Id", &row.correlation_id)?;
    insert_outbox_header(&mut headers, "Trpg-Visibility", &row.visibility_label)?;
    insert_outbox_header(
        &mut headers,
        "Trpg-Data-Subject-Digest",
        &data_subject_digest(&row.data_subject_id),
    )?;
    insert_outbox_header(&mut headers, "Trpg-Integrity-Status", &row.integrity_status)?;
    insert_outbox_header(
        &mut headers,
        "Trpg-Request-Hash-Source",
        &row.request_hash_source,
    )?;
    Ok(headers)
}

fn data_subject_digest(data_subject_id: &str) -> String {
    format!("sha256:{:x}", Sha256::digest(data_subject_id.as_bytes()))
}

fn canonical_delivery_subject(row: &OutboxClaim) -> String {
    if row.data_subject_id == "not_applicable" {
        format!("{}.unscoped", row.subject)
    } else {
        format!(
            "{}.subject.{:x}",
            row.subject,
            Sha256::digest(row.data_subject_id.as_bytes())
        )
    }
}

fn nats_message_id(row: &OutboxClaim) -> String {
    let mut digest = Sha256::new();
    for field in [
        row.campaign_id.as_str(),
        row.stream_id.as_str(),
        row.idempotency_operation.as_str(),
        row.idempotency_key.as_str(),
    ] {
        digest.update((field.len() as u64).to_be_bytes());
        digest.update(field.as_bytes());
    }
    format!("trpg-outbox-sha256:{:x}", digest.finalize())
}
