
#[derive(Clone)]
struct CanonicalEventIntegrityRecord {
    sequence: i64,
    event_index: usize,
    stream_version: i64,
    event_type: String,
    command_id: String,
    idempotency_key: String,
    expected_version: i64,
    authority_mode: String,
    authority_contract_version: i64,
    visibility_label: String,
    provenance_kind: String,
    provenance_reference: String,
    provenance_recorded_by: String,
    correlation_id: String,
    causation_id: String,
    campaign_id: String,
    authenticated_actor_id: String,
    authenticated_actor_role: String,
    authenticated_actor_origin: String,
    resource_type: String,
    resource_id: String,
    authority_contract_id: String,
    authority_owner: String,
    visibility_subject: String,
    trace_id: String,
    stream_id: String,
    event_schema_version: i32,
    idempotency_operation: String,
    request_hash: String,
    request_hash_source: String,
    integrity_status: String,
    payload_integrity_source: String,
    payload_ciphertext: Option<Vec<u8>>,
    payload_key_reference: Option<String>,
    payload_nonce: Option<Vec<u8>>,
    data_subject_id: String,
    projection_targets_json: String,
    recorded_at_micros: i64,
    derived_source_event_sequence: Option<i64>,
    derived_snapshot_id: Option<String>,
    derived_chunk_id: Option<String>,
    derived_content_hash: Option<String>,
    derived_source_type: Option<String>,
    derived_copyright_status: Option<String>,
    derived_allowed_use: Option<String>,
    derived_embedding_model: Option<String>,
    derived_embedding_dimensions: Option<i32>,
    derived_embedding_hash: Option<String>,
    deletion_job_id: Option<String>,
    deletion_subject_id: Option<String>,
    deletion_requested_by: Option<String>,
    deletion_retention_policy: Option<String>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct CanonicalDeletionRequestPayload {
    job_id: String,
    subject_id: String,
    requested_by: String,
    retention_policy: String,
    reason: String,
}

#[derive(serde::Deserialize)]
enum CanonicalPrivacyEventPayload {
    DataDeletionRequested(CanonicalDeletionRequestPayload),
}

#[derive(Default)]
struct DeletionRequestFields {
    job_id: Option<String>,
    subject_id: Option<String>,
    requested_by: Option<String>,
    retention_policy: Option<String>,
}

fn deletion_request_fields(
    event_type: &str,
    payload: &Value,
) -> Result<DeletionRequestFields, CanonicalStoreError> {
    if event_type != "platform.security_privacy_copyright.data_deletion_requested" {
        return Ok(DeletionRequestFields::default());
    }
    let CanonicalPrivacyEventPayload::DataDeletionRequested(deletion) =
        serde_json::from_value(payload.clone())
            .map_err(|_| CanonicalStoreError::Validation("deletion_request_payload_invalid"))?;
    let valid_identifier = |value: &str| {
        !value.is_empty()
            && value.len() <= 160
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
    };
    if !valid_identifier(&deletion.job_id)
        || !valid_identifier(&deletion.subject_id)
        || !valid_identifier(&deletion.requested_by)
        || deletion.retention_policy.trim().is_empty()
        || deletion.retention_policy.len() > 128
        || deletion.reason.len() > 1_024
    {
        return Err(CanonicalStoreError::Validation(
            "deletion_request_payload_invalid",
        ));
    }
    Ok(DeletionRequestFields {
        job_id: Some(deletion.job_id),
        subject_id: Some(deletion.subject_id),
        requested_by: Some(deletion.requested_by),
        retention_policy: Some(deletion.retention_policy),
    })
}

fn rag_derivation_fields(
    event_type: &str,
    payload: &Value,
) -> Result<RagDerivationFields, CanonicalStoreError> {
    if event_type != "RagChunkDerived" {
        return Ok(RagDerivationFields::default());
    }
    let derivation: RagChunkDerivationPayload = serde_json::from_value(payload.clone())
        .map_err(|_| CanonicalStoreError::Validation("rag_derivation_payload_invalid"))?;
    let valid_identifier = |value: &str| {
        !value.is_empty()
            && value.len() <= 160
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    };
    let valid_policy_token = |value: &str| {
        !value.is_empty()
            && value.len() <= 128
            && value
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    };
    let valid_lower_hex = |value: &str| {
        value.len() == 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    };
    if derivation.source_event_sequence <= 0
        || !valid_identifier(&derivation.snapshot_id)
        || !valid_identifier(&derivation.chunk_id)
        || !valid_lower_hex(&derivation.content_hash)
        || !valid_policy_token(&derivation.source_type)
        || !valid_policy_token(&derivation.copyright_status)
        || !valid_policy_token(&derivation.allowed_use)
        || derivation.embedding_model.trim().is_empty()
        || derivation.embedding_model.len() > 256
        || !(1..=4_096).contains(&derivation.embedding_dimensions)
        || !valid_lower_hex(&derivation.embedding_hash)
    {
        return Err(CanonicalStoreError::Validation(
            "rag_derivation_payload_invalid",
        ));
    }
    Ok(RagDerivationFields {
        source_event_sequence: Some(derivation.source_event_sequence),
        snapshot_id: Some(derivation.snapshot_id),
        chunk_id: Some(derivation.chunk_id),
        content_hash: Some(derivation.content_hash),
        source_type: Some(derivation.source_type),
        copyright_status: Some(derivation.copyright_status),
        allowed_use: Some(derivation.allowed_use),
        embedding_model: Some(derivation.embedding_model),
        embedding_dimensions: Some(derivation.embedding_dimensions),
        embedding_hash: Some(derivation.embedding_hash),
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolicyAuditDraft {
    pub actor_id: String,
    pub actor_origin: String,
    pub authentication_reference: String,
    pub resource_type: String,
    pub resource_id: String,
    pub action: String,
    pub requested_role: String,
    pub openfga_decision_id: String,
    pub openfga_policy_revision: String,
    pub opa_decision_id: String,
    pub opa_policy_revision: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AtomicCommitDraft {
    pub commit_id: String,
    pub campaign_id: String,
    /// Canonical aggregate stream. It must equal the policy-audited resource
    /// id so a caller cannot acquire one resource grant and write another
    /// stream inside the same campaign.
    pub stream_id: String,
    pub idempotency_key: String,
    pub expected_version: i64,
    pub command_id: String,
    pub authenticated_actor_id: String,
    pub authenticated_actor_role: String,
    pub authenticated_actor_origin: EventActorOriginWire,
    pub authority_mode: String,
    pub authority_contract_version: i64,
    pub authority_contract_id: String,
    pub authority_owner: String,
    pub visibility_label: String,
    pub visibility_subject: String,
    pub data_subject_id: String,
    pub provenance_kind: String,
    pub provenance_reference: String,
    pub provenance_recorded_by: String,
    pub correlation_id: String,
    pub causation_id: String,
    pub trace_id: String,
    pub events: Vec<CanonicalEventDraft>,
    pub audit: PolicyAuditDraft,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PersistedCommit {
    pub commit_id: String,
    pub first_event_sequence: i64,
    pub last_event_sequence: i64,
    pub first_stream_version: i64,
    pub last_stream_version: i64,
    pub audit_sequence: i64,
    pub witness_prepare_sequence: i64,
    pub witness_prepare_hash: String,
}

/// Read-only canonical event record returned to a production transport.  The
/// data adapter deliberately does not decide who may see the record; the
/// composition root must apply an identity-minted replay capability before it
/// serializes any event to a client.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct CanonicalReplayEvent {
    pub sequence: i64,
    pub stream_version: i64,
    pub stream_id: String,
    pub event_type: String,
    pub event_schema_version: i32,
    pub campaign_id: String,
    pub expected_version: i64,
    pub authority_mode: String,
    pub authenticated_actor_id: String,
    pub authenticated_actor_role: String,
    pub authenticated_actor_origin: EventActorOriginWire,
    pub resource_type: String,
    pub resource_id: String,
    pub authority_contract_id: String,
    pub authority_owner: String,
    pub command_id: String,
    pub idempotency_key: String,
    pub idempotency_operation: String,
    pub authority_contract_version: i64,
    pub visibility_label: String,
    pub visibility_subject: String,
    pub provenance_kind: String,
    pub provenance_reference: String,
    pub provenance_recorded_by: String,
    pub correlation_id: String,
    pub causation_id: String,
    pub trace_id: String,
    pub payload: Value,
    pub recorded_at: DateTime<Utc>,
    pub event_integrity_hash: Option<String>,
    pub request_hash: String,
    pub request_hash_source: String,
    pub integrity_status: String,
    pub payload_integrity_source: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RecoveryReport {
    pub finalized: usize,
    pub aborted: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CanonicalStoreError {
    Configuration(&'static str),
    Validation(&'static str),
    Connection {
        component: &'static str,
    },
    Migration {
        component: &'static str,
    },
    MigrationChecksumMismatch {
        component: &'static str,
        version: i64,
    },
    WitnessWrite {
        operation: &'static str,
    },
    PrimaryWrite {
        operation: &'static str,
    },
    VersionConflict {
        expected: i64,
        actual: i64,
    },
    IdempotencyConflict,
    WitnessFinalizationPending {
        commit_id: String,
    },
    IntegrityViolation(&'static str),
}

impl fmt::Display for CanonicalStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Configuration(reason) => write!(formatter, "configuration error: {reason}"),
            Self::Validation(reason) => write!(formatter, "commit validation error: {reason}"),
            Self::Connection { component } => write!(formatter, "{component} connection failed"),
            Self::Migration { component } => write!(formatter, "{component} migration failed"),
            Self::MigrationChecksumMismatch { component, version } => write!(
                formatter,
                "{component} migration checksum mismatch at immutable version {version}"
            ),
            Self::WitnessWrite { operation } => {
                write!(formatter, "external witness operation failed: {operation}")
            }
            Self::PrimaryWrite { operation } => {
                write!(formatter, "canonical store operation failed: {operation}")
            }
            Self::VersionConflict { expected, actual } => {
                write!(
                    formatter,
                    "expected version {expected}, actual version {actual}"
                )
            }
            Self::IdempotencyConflict => formatter.write_str("idempotency conflict"),
            Self::WitnessFinalizationPending { commit_id } => {
                write!(
                    formatter,
                    "commit {commit_id} is durable but witness finalization is pending"
                )
            }
            Self::IntegrityViolation(reason) => write!(formatter, "integrity violation: {reason}"),
        }
    }
}

impl std::error::Error for CanonicalStoreError {}

#[derive(Clone)]
pub struct PostgresCanonicalStore {
    primary: PgPool,
    witness: PgPool,
    integrity_key_id: String,
    integrity_key: Arc<Zeroizing<[u8; 32]>>,
    payload_cipher: Arc<PayloadCipher>,
}
