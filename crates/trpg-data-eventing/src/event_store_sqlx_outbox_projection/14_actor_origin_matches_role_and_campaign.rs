
fn actor_origin_matches_role_and_campaign(
    actor_role: &str,
    origin: &EventActorOriginWire,
    campaign_id: &str,
) -> bool {
    match origin {
        EventActorOriginWire::UserSession { session_id } => {
            !session_id.trim().is_empty()
                && matches!(
                    actor_role,
                    "server_owner"
                        | "campaign_owner"
                        | "human_keeper"
                        | "investigator"
                        | "moderator"
                        | "spectator"
                )
        }
        EventActorOriginWire::Workload { role } => {
            !role.trim().is_empty()
                && matches!(
                    (actor_role, role.as_str()),
                    ("workflow", "workflow_engine")
                        | ("rules_engine", "rules_engine")
                        | ("system", "api_server")
                        | ("system", "realtime_server")
                        | ("system", "agent_worker")
                        | ("system", "audit_writer")
                )
        }
        EventActorOriginWire::AgentRun {
            run_id,
            class,
            campaign_id: actor_campaign_id,
        } => {
            !run_id.trim().is_empty()
                && actor_campaign_id == campaign_id
                && matches!(
                    (actor_role, class.as_str()),
                    ("ai_keeper", "ai_keeper_orchestrator")
                        | ("investigator", "keeper_copilot")
                        | ("investigator", "atmosphere_writer")
                        | ("investigator", "memory_curator")
                )
        }
    }
}

fn legacy_event_integrity_hash(
    key: &[u8; 32],
    request_hash: &str,
    index: usize,
    event_type: &str,
    payload_json: &str,
) -> String {
    hmac_fields(
        key,
        &[
            request_hash.to_owned(),
            index.to_string(),
            event_type.to_owned(),
            payload_json.to_owned(),
        ],
    )
}

fn event_integrity_fields(
    record: &CanonicalEventIntegrityRecord,
    version: i32,
    include_projection_targets: bool,
) -> Vec<String> {
    let mut fields = vec![
        format!("canonical_event_integrity_v{version}"),
        version.to_string(),
        record.sequence.to_string(),
        record.event_index.to_string(),
        record.stream_version.to_string(),
        record.event_type.clone(),
        record.command_id.clone(),
        record.idempotency_key.clone(),
        record.expected_version.to_string(),
        record.authority_mode.clone(),
        record.authority_contract_version.to_string(),
        record.visibility_label.clone(),
        record.provenance_kind.clone(),
        record.provenance_reference.clone(),
        record.provenance_recorded_by.clone(),
        record.correlation_id.clone(),
        record.causation_id.clone(),
        record.campaign_id.clone(),
        record.authenticated_actor_id.clone(),
        record.authenticated_actor_role.clone(),
        record.authenticated_actor_origin.clone(),
        record.resource_type.clone(),
        record.resource_id.clone(),
        record.authority_contract_id.clone(),
        record.authority_owner.clone(),
        record.visibility_subject.clone(),
        record.trace_id.clone(),
        record.stream_id.clone(),
        record.event_schema_version.to_string(),
        record.idempotency_operation.clone(),
        record.request_hash.clone(),
        record.request_hash_source.clone(),
        record.integrity_status.clone(),
        record.payload_integrity_source.clone(),
        option_bytes(record.payload_ciphertext.as_deref()),
        option_string(record.payload_key_reference.as_deref()),
        option_bytes(record.payload_nonce.as_deref()),
        record.data_subject_id.clone(),
    ];
    if include_projection_targets {
        fields.push(record.projection_targets_json.clone());
    }
    fields.extend([
        record.recorded_at_micros.to_string(),
        integrity_option_i64(record.derived_source_event_sequence),
        option_string(record.derived_snapshot_id.as_deref()),
        option_string(record.derived_chunk_id.as_deref()),
        option_string(record.derived_content_hash.as_deref()),
        option_string(record.derived_source_type.as_deref()),
        option_string(record.derived_copyright_status.as_deref()),
        option_string(record.derived_allowed_use.as_deref()),
        option_string(record.derived_embedding_model.as_deref()),
        option_i32(record.derived_embedding_dimensions),
        option_string(record.derived_embedding_hash.as_deref()),
        option_string(record.deletion_job_id.as_deref()),
        option_string(record.deletion_subject_id.as_deref()),
        option_string(record.deletion_requested_by.as_deref()),
        option_string(record.deletion_retention_policy.as_deref()),
    ]);
    fields
}

fn event_integrity_hash_v2(key: &[u8; 32], record: &CanonicalEventIntegrityRecord) -> String {
    hmac_fields(key, &event_integrity_fields(record, 2, false))
}

fn event_integrity_hash_v3(key: &[u8; 32], record: &CanonicalEventIntegrityRecord) -> String {
    hmac_fields(key, &event_integrity_fields(record, 3, true))
}

fn witness_record_hash(key: &[u8; 32], record: &WitnessRecord) -> String {
    hmac_fields(
        key,
        &[
            record.sequence.to_string(),
            record.commit_id.clone(),
            record.phase.clone(),
            record.request_hash.clone(),
            option_i64(record.first_sequence),
            option_i64(record.last_sequence),
            record.reason.clone(),
            record.key_id.clone(),
            record.previous_hash.clone(),
        ],
    )
}

fn audit_record_hash(key: &[u8; 32], record: &AuditRecord) -> String {
    let mut fields = vec![
        record.sequence.to_string(),
        record.commit_id.clone(),
        record.campaign_id.clone(),
        record.actor_id.clone(),
        record.actor_origin.clone(),
        record.authentication_reference.clone(),
        record.resource_type.clone(),
        record.resource_id.clone(),
        record.action.clone(),
        record.requested_role.clone(),
        record.visibility_label.clone(),
        record.visibility_subject.clone(),
        record.provenance_kind.clone(),
        record.provenance_reference.clone(),
        record.provenance_recorded_by.clone(),
        record.decision.clone(),
        record.openfga_decision_id.clone(),
        record.openfga_policy_revision.clone(),
        record.opa_decision_id.clone(),
        record.opa_policy_revision.clone(),
        record.trace_id.clone(),
        record.event_batch_hash.clone(),
        record.witness_prepare_sequence.to_string(),
        record.witness_prepare_hash.clone(),
    ];
    if record.integrity_version == 3 {
        fields.push(record.correlation_id.clone());
        fields.push(record.causation_id.clone());
    }
    if matches!(record.integrity_version, 2 | 3) {
        // PostgreSQL stores TIMESTAMPTZ at microsecond precision. Hash the same
        // integer representation before insertion and after reloading so the
        // database round-trip cannot change the signed bytes.
        fields.push(record.occurred_at.timestamp_micros().to_string());
        fields.push(record.integrity_version.to_string());
    }
    fields.push(record.key_id.clone());
    fields.push(record.previous_hash.clone());
    hmac_fields(key, &fields)
}

fn verify_witness_chain(
    records: &[WitnessRecord],
    key: &[u8; 32],
) -> Result<(), CanonicalStoreError> {
    let mut previous = GENESIS_HASH.to_owned();
    for (index, record) in records.iter().enumerate() {
        if record.sequence != index as i64 + 1 || record.previous_hash != previous {
            return Err(CanonicalStoreError::IntegrityViolation(
                "external_witness_chain_discontinuity",
            ));
        }
        if record.record_hash != witness_record_hash(key, record) {
            return Err(CanonicalStoreError::IntegrityViolation(
                "external_witness_hmac_mismatch",
            ));
        }
        previous.clone_from(&record.record_hash);
    }
    Ok(())
}

fn verify_audit_chain(records: &[AuditRecord], key: &[u8; 32]) -> Result<(), CanonicalStoreError> {
    let mut previous = GENESIS_HASH.to_owned();
    for (index, record) in records.iter().enumerate() {
        if !matches!(record.integrity_version, 1..=3) {
            return Err(CanonicalStoreError::IntegrityViolation(
                "unsupported_canonical_audit_integrity_version",
            ));
        }
        if record.sequence != index as i64 + 1 || record.previous_hash != previous {
            return Err(CanonicalStoreError::IntegrityViolation(
                "canonical_audit_chain_discontinuity",
            ));
        }
        if record.record_hash != audit_record_hash(key, record) {
            return Err(CanonicalStoreError::IntegrityViolation(
                "canonical_audit_hmac_mismatch",
            ));
        }
        previous.clone_from(&record.record_hash);
    }
    Ok(())
}

fn sha256_fields(fields: &[String]) -> String {
    let mut digest = Sha256::new();
    update_fields(&mut digest, fields);
    format!("sha256:{}", lowercase_hex(&digest.finalize()))
}

fn hmac_fields(key: &[u8; 32], fields: &[String]) -> String {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts a 32-byte key");
    update_fields(&mut mac, fields);
    format!(
        "hmac-sha256:{}",
        lowercase_hex(&mac.finalize().into_bytes())
    )
}

fn update_fields<T: sha2::digest::Update>(digest: &mut T, fields: &[String]) {
    for field in fields {
        digest.update(&(field.len() as u64).to_be_bytes());
        digest.update(field.as_bytes());
    }
}

fn lowercase_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use fmt::Write as _;
        let _ = write!(output, "{byte:02x}");
    }
    output
}

fn option_i64(value: Option<i64>) -> String {
    value.map_or_else(|| "none".to_owned(), |value| value.to_string())
}

fn integrity_option_i64(value: Option<i64>) -> String {
    value.map_or_else(|| "absent".to_owned(), |value| format!("present:{value}"))
}

fn option_i32(value: Option<i32>) -> String {
    value.map_or_else(|| "absent".to_owned(), |value| format!("present:{value}"))
}

fn option_string(value: Option<&str>) -> String {
    value.map_or_else(
        || "absent".to_owned(),
        |value| format!("present:{}:{value}", value.len()),
    )
}

fn option_bytes(value: Option<&[u8]>) -> String {
    value.map_or_else(
        || "absent".to_owned(),
        |value| format!("present:{}:{}", value.len(), lowercase_hex(value)),
    )
}

fn replay_integrity_metadata_is_valid(
    integrity_status: &str,
    request_hash_source: &str,
    request_hash: &str,
    event_integrity_hash: Option<&str>,
    event_integrity_version: i32,
) -> bool {
    match (integrity_status, request_hash_source) {
        ("verified_hmac", "formal_commit") => {
            matches!(event_integrity_version, 2 | CURRENT_EVENT_INTEGRITY_VERSION)
                && event_integrity_hash.is_some()
                && request_hash != ZERO_REQUEST_HASH
        }
        ("historical_unverified_hmac", "formal_commit") => {
            event_integrity_version == 1
                && event_integrity_hash.is_some()
                && request_hash != ZERO_REQUEST_HASH
        }
        ("historical_unsigned", "historical_unavailable") => {
            event_integrity_version == 0
                && event_integrity_hash.is_none()
                && request_hash == ZERO_REQUEST_HASH
        }
        _ => false,
    }
}
