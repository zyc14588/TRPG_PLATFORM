crate::define_data_event_module!(
    SqlxOutboxProjectionCommand,
    SqlxOutboxProjectionOperation,
    append_sqlx_outbox_projection_event,
    "event_store_sqlx_outbox_projection",
    "SqlxOutboxProjectionRecorded",
    "data_eventing.event_store_sqlx_outbox_projection.event_schema",
    crate::DataEventOperation::EventStoreAppend,
    [
        "event_outbox",
        "projection_view",
        "sqlx_transaction_boundary"
    ]
);

use std::fmt;
use std::str::FromStr;
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions, PgSslMode};
use sqlx::types::Json;
use sqlx::{PgPool, Postgres, Row, Transaction};
use trpg_shared_kernel::{
    CanonicalCommitPort, CanonicalCommitReceipt, CanonicalCommitRequest, KernelResult, TrpgError,
};

const GENESIS_HASH: &str =
    "hmac-sha256:0000000000000000000000000000000000000000000000000000000000000000";
const ZERO_REQUEST_HASH: &str =
    "sha256:0000000000000000000000000000000000000000000000000000000000000000";
const CANONICAL_IDEMPOTENCY_OPERATION: &str = "canonical_commit";

type HmacSha256 = Hmac<Sha256>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanonicalEventDraft {
    pub event_type: String,
    pub payload_json: String,
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
    pub authority_mode: String,
    pub authority_contract_version: i64,
    pub authority_contract_id: String,
    pub authority_owner: String,
    pub visibility_label: String,
    pub visibility_subject: String,
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
#[derive(Clone, Debug, PartialEq)]
pub struct CanonicalReplayEvent {
    pub sequence: i64,
    pub stream_version: i64,
    pub stream_id: String,
    pub event_type: String,
    pub campaign_id: String,
    pub authenticated_actor_id: String,
    pub resource_type: String,
    pub resource_id: String,
    pub authority_contract_id: String,
    pub authority_owner: String,
    pub command_id: String,
    pub idempotency_key: String,
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
    pub event_integrity_hash: Option<String>,
    pub request_hash_source: String,
    pub integrity_status: String,
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
    integrity_key: [u8; 32],
}

impl fmt::Debug for PostgresCanonicalStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PostgresCanonicalStore")
            .field("primary", &"[POSTGRESQL POOL]")
            .field("witness", &"[INDEPENDENT POSTGRESQL POOL]")
            .field("integrity_key_id", &self.integrity_key_id)
            .field("integrity_key", &"[REDACTED]")
            .finish()
    }
}

/// Synchronous application port backed by the retained Tokio runtime that
/// owns the SQLx pools. Product composition roots keep this adapter private
/// and inject only the trait object into runtime/agent stores.
#[derive(Clone)]
pub struct PostgresCanonicalCommitPort {
    runtime: Arc<Mutex<tokio::runtime::Runtime>>,
    store: PostgresCanonicalStore,
}

impl PostgresCanonicalCommitPort {
    pub fn new(
        runtime: Arc<Mutex<tokio::runtime::Runtime>>,
        store: PostgresCanonicalStore,
    ) -> Self {
        Self { runtime, store }
    }
}

impl fmt::Debug for PostgresCanonicalCommitPort {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PostgresCanonicalCommitPort")
            .field("runtime", &"[RETAINED TOKIO RUNTIME]")
            .field("store", &self.store)
            .finish()
    }
}

impl CanonicalCommitPort for PostgresCanonicalCommitPort {
    fn commit(&self, request: &CanonicalCommitRequest) -> KernelResult<CanonicalCommitReceipt> {
        let draft = canonical_request_draft(request)?;
        let persisted = self
            .runtime
            .lock()
            .map_err(|_| TrpgError::AuditIntegrityViolation)?
            .block_on(self.store.commit(&draft))
            .map_err(map_canonical_port_error)?;
        Ok(CanonicalCommitReceipt {
            first_stream_version: u64::try_from(persisted.first_stream_version)
                .map_err(|_| TrpgError::AuditIntegrityViolation)?,
            last_stream_version: u64::try_from(persisted.last_stream_version)
                .map_err(|_| TrpgError::AuditIntegrityViolation)?,
        })
    }
}

fn canonical_request_draft(request: &CanonicalCommitRequest) -> KernelResult<AtomicCommitDraft> {
    let expected_version =
        i64::try_from(request.expected_version).map_err(|_| TrpgError::AuditIntegrityViolation)?;
    let authority_contract_version = i64::try_from(request.authority_contract_version)
        .map_err(|_| TrpgError::AuthorityContractVersionConflict)?;
    Ok(AtomicCommitDraft {
        commit_id: request.commit_id.clone(),
        campaign_id: request.campaign_id.clone(),
        // The policy audit is constructed from AuthenticatedCommandContext's
        // ResourceRef. Reusing that resource id avoids a second, drift-prone
        // stream field in the external write request while preserving a
        // lossless authorized-resource -> database-stream mapping.
        stream_id: request.audit.resource_id.clone(),
        idempotency_key: request.idempotency_key.clone(),
        expected_version,
        command_id: request.command_id.clone(),
        authenticated_actor_id: request.authenticated_actor_id.clone(),
        authority_mode: request.authority_mode.clone(),
        authority_contract_version,
        authority_contract_id: request.authority_contract_id.clone(),
        authority_owner: request.authority_owner.clone(),
        visibility_label: request.visibility_label.clone(),
        visibility_subject: request.visibility_subject.clone(),
        provenance_kind: request.provenance_kind.clone(),
        provenance_reference: request.provenance_reference.clone(),
        provenance_recorded_by: request.provenance_recorded_by.clone(),
        correlation_id: request.correlation_id.clone(),
        causation_id: request.causation_id.clone(),
        trace_id: request.trace_id.clone(),
        events: request
            .events
            .iter()
            .map(|event| CanonicalEventDraft {
                event_type: event.event_type.clone(),
                payload_json: event.payload_json.clone(),
            })
            .collect(),
        audit: PolicyAuditDraft {
            actor_id: request.audit.actor_id.clone(),
            actor_origin: request.audit.actor_origin.clone(),
            authentication_reference: request.audit.authentication_reference.clone(),
            resource_type: request.audit.resource_type.clone(),
            resource_id: request.audit.resource_id.clone(),
            action: request.audit.action.clone(),
            requested_role: request.audit.requested_role.clone(),
            openfga_decision_id: request.audit.openfga_decision_id.clone(),
            openfga_policy_revision: request.audit.openfga_policy_revision.clone(),
            opa_decision_id: request.audit.opa_decision_id.clone(),
            opa_policy_revision: request.audit.opa_policy_revision.clone(),
        },
    })
}

fn map_canonical_port_error(error: CanonicalStoreError) -> TrpgError {
    match error {
        CanonicalStoreError::VersionConflict { expected, actual } => {
            match (u64::try_from(expected), u64::try_from(actual)) {
                (Ok(expected), Ok(actual)) => {
                    TrpgError::ExpectedVersionConflict { expected, actual }
                }
                _ => TrpgError::AuditIntegrityViolation,
            }
        }
        CanonicalStoreError::IdempotencyConflict => TrpgError::DuplicateCommand,
        _ => TrpgError::AuditIntegrityViolation,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WitnessPhase {
    Prepared,
    Committed,
    Aborted,
}

impl WitnessPhase {
    fn as_str(self) -> &'static str {
        match self {
            Self::Prepared => "PREPARED",
            Self::Committed => "COMMITTED",
            Self::Aborted => "ABORTED",
        }
    }
}

#[derive(Clone, Debug)]
struct WitnessRecord {
    sequence: i64,
    commit_id: String,
    phase: String,
    request_hash: String,
    first_sequence: Option<i64>,
    last_sequence: Option<i64>,
    reason: String,
    key_id: String,
    previous_hash: String,
    record_hash: String,
}

#[derive(Clone, Debug)]
struct AuditRecord {
    sequence: i64,
    commit_id: String,
    campaign_id: String,
    actor_id: String,
    actor_origin: String,
    authentication_reference: String,
    resource_type: String,
    resource_id: String,
    action: String,
    requested_role: String,
    visibility_label: String,
    visibility_subject: String,
    provenance_kind: String,
    provenance_reference: String,
    provenance_recorded_by: String,
    decision: String,
    openfga_decision_id: String,
    openfga_policy_revision: String,
    opa_decision_id: String,
    opa_policy_revision: String,
    trace_id: String,
    event_batch_hash: String,
    witness_prepare_sequence: i64,
    witness_prepare_hash: String,
    occurred_at: DateTime<Utc>,
    integrity_version: i32,
    key_id: String,
    previous_hash: String,
    record_hash: String,
}

impl PostgresCanonicalStore {
    pub async fn connect(
        primary_url: &str,
        witness_url: &str,
        integrity_key_id: impl Into<String>,
        integrity_key: &[u8],
    ) -> Result<Self, CanonicalStoreError> {
        if integrity_key.len() != 32 {
            return Err(CanonicalStoreError::Configuration(
                "32_byte_integrity_key_required",
            ));
        }
        let integrity_key_id = integrity_key_id.into();
        if integrity_key_id.trim().is_empty() {
            return Err(CanonicalStoreError::Configuration(
                "integrity_key_id_required",
            ));
        }

        let primary_options = parse_connection_options(primary_url, "primary")?;
        let witness_options = parse_connection_options(witness_url, "witness")?;
        if same_endpoint(&primary_options, &witness_options) {
            return Err(CanonicalStoreError::Configuration(
                "independent_witness_endpoint_required",
            ));
        }

        let primary = PgPoolOptions::new()
            .max_connections(10)
            .connect_with(primary_options)
            .await
            .map_err(|_| CanonicalStoreError::Connection {
                component: "primary",
            })?;
        let witness = PgPoolOptions::new()
            .max_connections(5)
            .connect_with(witness_options)
            .await
            .map_err(|_| CanonicalStoreError::Connection {
                component: "witness",
            })?;

        let mut key = [0_u8; 32];
        key.copy_from_slice(integrity_key);
        Ok(Self {
            primary,
            witness,
            integrity_key_id,
            integrity_key: key,
        })
    }

    pub async fn apply_migrations(&self) -> Result<(), CanonicalStoreError> {
        crate::persistence_migrations::migrator()
            .run(&self.primary)
            .await
            .map_err(|error| match error {
                sqlx::migrate::MigrateError::VersionMismatch(version) => {
                    CanonicalStoreError::MigrationChecksumMismatch {
                        component: "primary",
                        version,
                    }
                }
                _ => CanonicalStoreError::Migration {
                    component: "primary",
                },
            })?;

        crate::persistence_migrations::witness_migrator()
            .run(&self.witness)
            .await
            .map_err(|error| match error {
                sqlx::migrate::MigrateError::VersionMismatch(version) => {
                    CanonicalStoreError::MigrationChecksumMismatch {
                        component: "witness",
                        version,
                    }
                }
                _ => CanonicalStoreError::Migration {
                    component: "witness",
                },
            })?;
        Ok(())
    }

    /// Apply migrations, reconcile crash gaps, and prove both chains agree.
    /// Production composition should call this before accepting traffic.
    pub async fn prepare_for_service(&self) -> Result<RecoveryReport, CanonicalStoreError> {
        self.apply_migrations().await?;
        self.recover().await
    }

    pub async fn commit(
        &self,
        draft: &AtomicCommitDraft,
    ) -> Result<PersistedCommit, CanonicalStoreError> {
        let normalized = normalize_and_validate(draft)?;
        let request_hash = request_hash(&normalized);

        if let Some(existing) = self
            .load_existing_commit(
                &normalized.commit_id,
                &normalized.campaign_id,
                &normalized.stream_id,
                &normalized.idempotency_key,
            )
            .await?
        {
            self.validate_existing_commit(&existing, &normalized, &request_hash)
                .await?;
            self.finalize_witness(&existing, &request_hash).await?;
            return Ok(existing);
        }

        let prepared = self
            .append_witness(
                &normalized.commit_id,
                WitnessPhase::Prepared,
                &request_hash,
                None,
                None,
                "primary_commit_pending",
            )
            .await?;

        let persisted = match self
            .commit_primary(&normalized, &request_hash, &prepared)
            .await
        {
            Ok(persisted) => persisted,
            Err(error) => return Err(error),
        };

        self.finalize_witness(&persisted, &request_hash).await?;
        Ok(persisted)
    }

    pub async fn recover(&self) -> Result<RecoveryReport, CanonicalStoreError> {
        let rows = sqlx::query(
            r#"
            SELECT p.sequence, p.commit_id, p.primary_request_hash, p.record_hash
              FROM external_audit_witness p
             WHERE p.phase = 'PREPARED'
               AND NOT EXISTS (
                   SELECT 1 FROM external_audit_witness terminal
                    WHERE terminal.commit_id = p.commit_id
                      AND terminal.phase IN ('COMMITTED', 'ABORTED')
               )
             ORDER BY p.sequence
            "#,
        )
        .fetch_all(&self.witness)
        .await
        .map_err(|_| CanonicalStoreError::WitnessWrite {
            operation: "scan_recovery_candidates",
        })?;

        let mut report = RecoveryReport::default();
        for row in rows {
            let commit_id: String = row.get("commit_id");
            let request_hash: String = row.get("primary_request_hash");
            let prepare_sequence: i64 = row.get("sequence");
            let prepare_hash: String = row.get("record_hash");
            if let Some(existing) = self.load_existing_commit(&commit_id, "", "", "").await? {
                if existing.witness_prepare_sequence != prepare_sequence
                    || existing.witness_prepare_hash != prepare_hash
                {
                    return Err(CanonicalStoreError::IntegrityViolation(
                        "primary_witness_prepare_binding_mismatch",
                    ));
                }
                self.finalize_witness(&existing, &request_hash).await?;
                report.finalized += 1;
            } else {
                self.append_witness(
                    &commit_id,
                    WitnessPhase::Aborted,
                    &request_hash,
                    None,
                    None,
                    "primary_transaction_absent_after_recovery",
                )
                .await?;
                report.aborted += 1;
            }
        }
        self.verify_integrity().await?;
        Ok(report)
    }

    pub async fn verify_integrity(&self) -> Result<(), CanonicalStoreError> {
        let witness_records = self.load_witness_records().await?;
        verify_witness_chain(&witness_records, &self.integrity_key)?;
        let audit_records = self.load_audit_records().await?;
        verify_audit_chain(&audit_records, &self.integrity_key)?;

        let commits = sqlx::query(
            r#"
            SELECT commit_id, primary_request_hash, sequence, phase,
                   primary_first_sequence, primary_last_sequence, record_hash
              FROM external_audit_witness
             WHERE phase IN ('PREPARED', 'COMMITTED', 'ABORTED')
             ORDER BY sequence
            "#,
        )
        .fetch_all(&self.witness)
        .await
        .map_err(|_| CanonicalStoreError::WitnessWrite {
            operation: "verify_bindings",
        })?;

        for row in commits {
            let phase: String = row.get("phase");
            let commit_id: String = row.get("commit_id");
            let primary = self.load_existing_commit(&commit_id, "", "", "").await?;
            match (phase.as_str(), primary) {
                ("PREPARED", _) => {}
                ("ABORTED", None) => {}
                ("ABORTED", Some(_)) => {
                    return Err(CanonicalStoreError::IntegrityViolation(
                        "aborted_witness_has_primary_commit",
                    ));
                }
                ("COMMITTED", Some(persisted)) => {
                    let request_hash: String = row.get("primary_request_hash");
                    let first: Option<i64> = row.get("primary_first_sequence");
                    let last: Option<i64> = row.get("primary_last_sequence");
                    if first != Some(persisted.first_event_sequence)
                        || last != Some(persisted.last_event_sequence)
                    {
                        return Err(CanonicalStoreError::IntegrityViolation(
                            "committed_witness_primary_range_mismatch",
                        ));
                    }
                    let stored_request: String = sqlx::query_scalar(
                        "SELECT request_hash FROM formal_commits WHERE commit_id = $1",
                    )
                    .bind(&commit_id)
                    .fetch_one(&self.primary)
                    .await
                    .map_err(|_| CanonicalStoreError::PrimaryWrite {
                        operation: "verify_request_binding",
                    })?;
                    if request_hash != stored_request {
                        return Err(CanonicalStoreError::IntegrityViolation(
                            "committed_witness_request_mismatch",
                        ));
                    }
                }
                ("COMMITTED", None) => {
                    return Err(CanonicalStoreError::IntegrityViolation(
                        "committed_witness_missing_primary_commit",
                    ));
                }
                _ => {
                    return Err(CanonicalStoreError::IntegrityViolation(
                        "unknown_witness_phase",
                    ));
                }
            }
        }

        let primary_commits = sqlx::query(
            r#"
            SELECT commit_id, request_hash,
                   (response_payload->>'first_event_sequence')::bigint AS first_event_sequence,
                   result_event_sequence AS last_event_sequence,
                   (response_payload->>'first_stream_version')::bigint AS first_stream_version,
                   (response_payload->>'last_stream_version')::bigint AS last_stream_version,
                   witness_prepare_sequence, witness_prepare_hash
              FROM formal_commits
             ORDER BY committed_at, commit_id
            "#,
        )
        .fetch_all(&self.primary)
        .await
        .map_err(|_| CanonicalStoreError::PrimaryWrite {
            operation: "verify_primary_commits",
        })?;
        for row in primary_commits {
            let commit_id: String = row.get("commit_id");
            let request_hash: String = row.get("request_hash");
            let first_event_sequence: i64 = row.get("first_event_sequence");
            let last_event_sequence: i64 = row.get("last_event_sequence");
            let first_stream_version: i64 = row.get("first_stream_version");
            let last_stream_version: i64 = row.get("last_stream_version");
            let prepare_sequence: i64 = row.get("witness_prepare_sequence");
            let prepare_hash: String = row.get("witness_prepare_hash");
            let prepared_count: i64 = sqlx::query_scalar(
                r#"
                SELECT count(*) FROM external_audit_witness
                 WHERE commit_id = $1 AND phase = 'PREPARED'
                   AND sequence = $2 AND record_hash = $3
                   AND primary_request_hash = $4
                "#,
            )
            .bind(&commit_id)
            .bind(prepare_sequence)
            .bind(&prepare_hash)
            .bind(&request_hash)
            .fetch_one(&self.witness)
            .await
            .map_err(|_| CanonicalStoreError::WitnessWrite {
                operation: "verify_primary_prepare",
            })?;
            let committed_count: i64 = sqlx::query_scalar(
                r#"
                SELECT count(*) FROM external_audit_witness
                 WHERE commit_id = $1 AND phase = 'COMMITTED'
                   AND primary_request_hash = $2
                   AND primary_first_sequence = $3
                   AND primary_last_sequence = $4
                "#,
            )
            .bind(&commit_id)
            .bind(&request_hash)
            .bind(first_event_sequence)
            .bind(last_event_sequence)
            .fetch_one(&self.witness)
            .await
            .map_err(|_| CanonicalStoreError::WitnessWrite {
                operation: "verify_primary_finalize",
            })?;
            if prepared_count != 1 || committed_count != 1 {
                return Err(CanonicalStoreError::IntegrityViolation(
                    "primary_commit_missing_external_witness",
                ));
            }

            let event_rows = sqlx::query(
                r#"
                SELECT event.sequence, event.stream_version, event.event_type,
                       event.payload_json, event.payload_integrity_source,
                       event.event_integrity_hash, event.request_hash,
                       event.request_hash_source, event.integrity_status,
                       outbox.request_hash AS outbox_request_hash,
                       outbox.request_hash_source AS outbox_request_hash_source,
                       outbox.integrity_status AS outbox_integrity_status,
                       outbox.campaign_id AS outbox_campaign_id,
                       outbox.stream_id AS outbox_stream_id,
                       event.campaign_id, event.stream_id
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
                let integrity_classification_matches = event_integrity_status
                    == outbox_integrity_status
                    && matches!(
                        event_integrity_status.as_str(),
                        "verified_hmac" | "historical_unverified_hmac"
                    );
                if event.get::<String, _>("request_hash") != request_hash
                    || event.get::<String, _>("outbox_request_hash") != request_hash
                    || event.get::<String, _>("request_hash_source") != "formal_commit"
                    || event.get::<String, _>("outbox_request_hash_source") != "formal_commit"
                    || !integrity_classification_matches
                    || event.get::<String, _>("campaign_id")
                        != event.get::<String, _>("outbox_campaign_id")
                    || event.get::<String, _>("stream_id")
                        != event.get::<String, _>("outbox_stream_id")
                {
                    return Err(CanonicalStoreError::IntegrityViolation(
                        "canonical_event_outbox_binding_mismatch",
                    ));
                }
                let event_type: String = event.get("event_type");
                let payload: Json<Value> = event.get("payload_json");
                let payload_integrity_source: String = event.get("payload_integrity_source");
                let integrity_payload: Value = serde_json::from_str(&payload_integrity_source)
                    .map_err(|_| {
                        CanonicalStoreError::IntegrityViolation("event_payload_json_invalid")
                    })?;
                if integrity_payload != payload.0 {
                    return Err(CanonicalStoreError::IntegrityViolation(
                        "event_payload_integrity_source_mismatch",
                    ));
                }
                let stored_hash: Option<String> = event.get("event_integrity_hash");
                let expected_hash = event_integrity_hash(
                    &self.integrity_key,
                    &request_hash,
                    index,
                    &event_type,
                    &payload_integrity_source,
                );
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
        }
        Ok(())
    }

    /// Loads a bounded, ordered replay page. Authorization and visibility
    /// filtering remain the responsibility of the production composition
    /// root, which owns this store and the live identity verifier together.
    pub async fn load_replay_page(
        &self,
        campaign_id: &str,
        after_sequence: i64,
        limit: i64,
    ) -> Result<Vec<CanonicalReplayEvent>, CanonicalStoreError> {
        load_canonical_replay_page(&self.primary, campaign_id, after_sequence, limit).await
    }

    async fn commit_primary(
        &self,
        draft: &AtomicCommitDraft,
        request_hash: &str,
        prepared: &WitnessRecord,
    ) -> Result<PersistedCommit, CanonicalStoreError> {
        let mut transaction =
            self.primary
                .begin()
                .await
                .map_err(|_| CanonicalStoreError::PrimaryWrite {
                    operation: "begin_transaction",
                })?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1 || ':' || $2, 0))")
            .bind(&draft.campaign_id)
            .bind(&draft.stream_id)
            .execute(&mut *transaction)
            .await
            .map_err(|_| CanonicalStoreError::PrimaryWrite {
                operation: "lock_campaign_stream",
            })?;

        if let Some(existing) = load_existing_commit_in_transaction(
            &mut transaction,
            &draft.commit_id,
            &draft.campaign_id,
            &draft.stream_id,
            &draft.idempotency_key,
        )
        .await?
        {
            if load_request_hash_in_transaction(&mut transaction, &existing.commit_id).await?
                != request_hash
            {
                return Err(CanonicalStoreError::IdempotencyConflict);
            }
            transaction
                .commit()
                .await
                .map_err(|_| CanonicalStoreError::PrimaryWrite {
                    operation: "commit_idempotent_transaction",
                })?;
            return Ok(existing);
        }

        let actual_version: i64 = sqlx::query_scalar(
            "SELECT COALESCE(max(stream_version), 0) FROM event_store WHERE campaign_id = $1 AND stream_id = $2",
        )
        .bind(&draft.campaign_id)
        .bind(&draft.stream_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(|_| CanonicalStoreError::PrimaryWrite {
            operation: "read_stream_version",
        })?;
        if actual_version != draft.expected_version {
            return Err(CanonicalStoreError::VersionConflict {
                expected: draft.expected_version,
                actual: actual_version,
            });
        }

        let mut event_sequences = Vec::with_capacity(draft.events.len());
        let mut event_hashes = Vec::with_capacity(draft.events.len());
        for (index, event) in draft.events.iter().enumerate() {
            let payload: Value = serde_json::from_str(&event.payload_json)
                .map_err(|_| CanonicalStoreError::Validation("event_payload_must_be_json"))?;
            let canonical_payload = serde_json::to_string(&payload)
                .map_err(|_| CanonicalStoreError::Validation("event_payload_must_be_json"))?;
            let stream_version = draft.expected_version + index as i64 + 1;
            let event_hash = event_integrity_hash(
                &self.integrity_key,
                request_hash,
                index,
                &event.event_type,
                &canonical_payload,
            );
            let event_idempotency_key = format!("{}:{index:04}", draft.idempotency_key);
            let sequence: i64 = sqlx::query_scalar(
                r#"
                INSERT INTO event_store (
                    event_type, command_id, idempotency_key, expected_version,
                    authority_mode, authority_contract_version, visibility_label,
                    fact_provenance_kind, fact_provenance_reference, fact_recorded_by,
                    correlation_id, causation_id, payload_json, campaign_id,
                    stream_version, authenticated_actor_id, resource_type, resource_id,
                    authority_contract_id, authority_owner, visibility_subject, trace_id,
                    event_integrity_hash, stream_id, event_schema_version,
                    idempotency_operation, request_hash, request_hash_source,
                    integrity_status, payload_integrity_source
                ) VALUES (
                    $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13,
                    $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24,
                    $25, $26, $27, $28, $29, $30
                ) RETURNING sequence
                "#,
            )
            .bind(&event.event_type)
            .bind(&draft.command_id)
            .bind(&event_idempotency_key)
            .bind(draft.expected_version)
            .bind(&draft.authority_mode)
            .bind(draft.authority_contract_version)
            .bind(&draft.visibility_label)
            .bind(&draft.provenance_kind)
            .bind(&draft.provenance_reference)
            .bind(&draft.provenance_recorded_by)
            .bind(&draft.correlation_id)
            .bind(&draft.causation_id)
            .bind(Json(payload.clone()))
            .bind(&draft.campaign_id)
            .bind(stream_version)
            .bind(&draft.authenticated_actor_id)
            .bind(&draft.audit.resource_type)
            .bind(&draft.audit.resource_id)
            .bind(&draft.authority_contract_id)
            .bind(&draft.authority_owner)
            .bind(&draft.visibility_subject)
            .bind(&draft.trace_id)
            .bind(&event_hash)
            .bind(&draft.stream_id)
            .bind(crate::persistence::CURRENT_EVENT_SCHEMA_VERSION)
            .bind(CANONICAL_IDEMPOTENCY_OPERATION)
            .bind(request_hash)
            .bind("formal_commit")
            .bind("verified_hmac")
            .bind(&canonical_payload)
            .fetch_one(&mut *transaction)
            .await
            .map_err(|_| CanonicalStoreError::PrimaryWrite {
                operation: "insert_event",
            })?;

            sqlx::query(
                r#"
                INSERT INTO event_outbox (
                    event_id, event_sequence, nats_subject, idempotency_key,
                    visibility_label, correlation_id, causation_id, payload_json, commit_id,
                    campaign_id, stream_id, event_schema_version,
                    idempotency_operation, request_hash, request_hash_source,
                    integrity_status
                ) VALUES (
                    $1, $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11,
                    $12, $13, $14, $15
                )
                "#,
            )
            .bind(sequence)
            .bind(crate::NATS_EVENTS_APPENDED)
            .bind(format!("outbox:{event_idempotency_key}"))
            .bind(&draft.visibility_label)
            .bind(&draft.correlation_id)
            .bind(&draft.causation_id)
            .bind(Json(payload))
            .bind(&draft.commit_id)
            .bind(&draft.campaign_id)
            .bind(&draft.stream_id)
            .bind(crate::persistence::CURRENT_EVENT_SCHEMA_VERSION)
            .bind(CANONICAL_IDEMPOTENCY_OPERATION)
            .bind(request_hash)
            .bind("formal_commit")
            .bind("verified_hmac")
            .execute(&mut *transaction)
            .await
            .map_err(|_| CanonicalStoreError::PrimaryWrite {
                operation: "insert_outbox",
            })?;

            event_sequences.push(sequence);
            event_hashes.push(event_hash);
        }

        let first_event_sequence =
            *event_sequences
                .first()
                .ok_or(CanonicalStoreError::Validation(
                    "at_least_one_event_required",
                ))?;
        let last_event_sequence =
            *event_sequences
                .last()
                .ok_or(CanonicalStoreError::Validation(
                    "at_least_one_event_required",
                ))?;
        let event_batch_hash = sha256_fields(&event_hashes);
        let audit_sequence = self
            .insert_audit(&mut transaction, draft, &event_batch_hash, prepared)
            .await?;
        let first_stream_version = draft.expected_version + 1;
        let last_stream_version = draft.expected_version + draft.events.len() as i64;

        sqlx::query(
            r#"
            INSERT INTO formal_commits (
                commit_id, campaign_id, idempotency_key, request_hash, expected_version,
                first_event_sequence, last_event_sequence, first_stream_version,
                last_stream_version, audit_sequence, witness_prepare_sequence,
                witness_prepare_hash, stream_id, idempotency_operation, status,
                result_event_sequence, response_payload
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12,
                $13, $14, $15, $16, $17
            )
            "#,
        )
        .bind(&draft.commit_id)
        .bind(&draft.campaign_id)
        .bind(&draft.idempotency_key)
        .bind(request_hash)
        .bind(draft.expected_version)
        .bind(first_event_sequence)
        .bind(last_event_sequence)
        .bind(first_stream_version)
        .bind(last_stream_version)
        .bind(audit_sequence)
        .bind(prepared.sequence)
        .bind(&prepared.record_hash)
        .bind(&draft.stream_id)
        .bind(CANONICAL_IDEMPOTENCY_OPERATION)
        .bind("committed")
        .bind(last_event_sequence)
        .bind(Json(serde_json::json!({
            "first_event_sequence": first_event_sequence,
            "last_event_sequence": last_event_sequence,
            "first_stream_version": first_stream_version,
            "last_stream_version": last_stream_version,
        })))
        .execute(&mut *transaction)
        .await
        .map_err(|_| CanonicalStoreError::PrimaryWrite {
            operation: "insert_formal_commit",
        })?;

        transaction
            .commit()
            .await
            .map_err(|_| CanonicalStoreError::PrimaryWrite {
                operation: "commit_transaction",
            })?;

        Ok(PersistedCommit {
            commit_id: draft.commit_id.clone(),
            first_event_sequence,
            last_event_sequence,
            first_stream_version,
            last_stream_version,
            audit_sequence,
            witness_prepare_sequence: prepared.sequence,
            witness_prepare_hash: prepared.record_hash.clone(),
        })
    }

    async fn insert_audit(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        draft: &AtomicCommitDraft,
        event_batch_hash: &str,
        prepared: &WitnessRecord,
    ) -> Result<i64, CanonicalStoreError> {
        sqlx::query(
            "SELECT pg_advisory_xact_lock(hashtextextended('trpg.canonical_audit_log.chain', 0))",
        )
        .execute(&mut **transaction)
        .await
        .map_err(|_| CanonicalStoreError::PrimaryWrite {
            operation: "lock_audit_chain",
        })?;
        let previous = sqlx::query(
            "SELECT sequence, record_hash FROM canonical_audit_log ORDER BY sequence DESC LIMIT 1",
        )
        .fetch_optional(&mut **transaction)
        .await
        .map_err(|_| CanonicalStoreError::PrimaryWrite {
            operation: "read_audit_head",
        })?;
        let (sequence, previous_hash) = previous.map_or((1, GENESIS_HASH.to_owned()), |row| {
            (row.get::<i64, _>("sequence") + 1, row.get("record_hash"))
        });
        let record = AuditRecord {
            sequence,
            commit_id: draft.commit_id.clone(),
            campaign_id: draft.campaign_id.clone(),
            actor_id: draft.audit.actor_id.clone(),
            actor_origin: draft.audit.actor_origin.clone(),
            authentication_reference: draft.audit.authentication_reference.clone(),
            resource_type: draft.audit.resource_type.clone(),
            resource_id: draft.audit.resource_id.clone(),
            action: draft.audit.action.clone(),
            requested_role: draft.audit.requested_role.clone(),
            visibility_label: draft.visibility_label.clone(),
            visibility_subject: draft.visibility_subject.clone(),
            provenance_kind: draft.provenance_kind.clone(),
            provenance_reference: draft.provenance_reference.clone(),
            provenance_recorded_by: draft.provenance_recorded_by.clone(),
            decision: "PERMIT".to_owned(),
            openfga_decision_id: draft.audit.openfga_decision_id.clone(),
            openfga_policy_revision: draft.audit.openfga_policy_revision.clone(),
            opa_decision_id: draft.audit.opa_decision_id.clone(),
            opa_policy_revision: draft.audit.opa_policy_revision.clone(),
            trace_id: draft.trace_id.clone(),
            event_batch_hash: event_batch_hash.to_owned(),
            witness_prepare_sequence: prepared.sequence,
            witness_prepare_hash: prepared.record_hash.clone(),
            occurred_at: Utc::now(),
            integrity_version: 2,
            key_id: self.integrity_key_id.clone(),
            previous_hash,
            record_hash: String::new(),
        };
        let record_hash = audit_record_hash(&self.integrity_key, &record);

        let inserted_sequence: i64 = sqlx::query_scalar(
            r#"
            INSERT INTO canonical_audit_log (
                sequence, commit_id, campaign_id, actor_id, actor_origin,
                authentication_reference, resource_type, resource_id, action,
                requested_role, visibility_label, visibility_subject, provenance_kind,
                provenance_reference, provenance_recorded_by, decision,
                openfga_decision_id, openfga_policy_revision, opa_decision_id,
                opa_policy_revision, trace_id, event_batch_hash,
                witness_prepare_sequence, witness_prepare_hash, occurred_at,
                integrity_version, integrity_key_id, previous_hash, record_hash
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13,
                $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24,
                $25, $26, $27, $28, $29
            ) RETURNING sequence
            "#,
        )
        .bind(record.sequence)
        .bind(&record.commit_id)
        .bind(&record.campaign_id)
        .bind(&record.actor_id)
        .bind(&record.actor_origin)
        .bind(&record.authentication_reference)
        .bind(&record.resource_type)
        .bind(&record.resource_id)
        .bind(&record.action)
        .bind(&record.requested_role)
        .bind(&record.visibility_label)
        .bind(&record.visibility_subject)
        .bind(&record.provenance_kind)
        .bind(&record.provenance_reference)
        .bind(&record.provenance_recorded_by)
        .bind(&record.decision)
        .bind(&record.openfga_decision_id)
        .bind(&record.openfga_policy_revision)
        .bind(&record.opa_decision_id)
        .bind(&record.opa_policy_revision)
        .bind(&record.trace_id)
        .bind(&record.event_batch_hash)
        .bind(record.witness_prepare_sequence)
        .bind(&record.witness_prepare_hash)
        .bind(record.occurred_at)
        .bind(record.integrity_version)
        .bind(&record.key_id)
        .bind(&record.previous_hash)
        .bind(&record_hash)
        .fetch_one(&mut **transaction)
        .await
        .map_err(|_| CanonicalStoreError::PrimaryWrite {
            operation: "insert_audit",
        })?;
        if inserted_sequence != sequence {
            return Err(CanonicalStoreError::IntegrityViolation(
                "audit_sequence_changed_during_insert",
            ));
        }
        Ok(sequence)
    }

    async fn append_witness(
        &self,
        commit_id: &str,
        phase: WitnessPhase,
        request_hash: &str,
        first_sequence: Option<i64>,
        last_sequence: Option<i64>,
        reason: &str,
    ) -> Result<WitnessRecord, CanonicalStoreError> {
        let mut transaction = self
            .witness
            .begin()
            .await
            .map_err(|_| CanonicalStoreError::WitnessWrite { operation: "begin" })?;
        sqlx::query(
            "SELECT pg_advisory_xact_lock(hashtextextended('trpg.external_audit_witness.chain', 0))",
        )
        .execute(&mut *transaction)
        .await
        .map_err(|_| CanonicalStoreError::WitnessWrite { operation: "lock" })?;

        if let Some(row) = sqlx::query(
            r#"
            SELECT sequence, commit_id, phase, primary_request_hash,
                   primary_first_sequence, primary_last_sequence, reason,
                   integrity_key_id, previous_hash, record_hash
              FROM external_audit_witness
             WHERE commit_id = $1 AND phase = $2
            "#,
        )
        .bind(commit_id)
        .bind(phase.as_str())
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|_| CanonicalStoreError::WitnessWrite {
            operation: "read_idempotent_record",
        })? {
            let existing = witness_from_row(&row);
            if existing.request_hash != request_hash
                || existing.first_sequence != first_sequence
                || existing.last_sequence != last_sequence
            {
                return Err(CanonicalStoreError::IdempotencyConflict);
            }
            transaction
                .commit()
                .await
                .map_err(|_| CanonicalStoreError::WitnessWrite {
                    operation: "commit_idempotent_record",
                })?;
            return Ok(existing);
        }

        let previous = sqlx::query(
            "SELECT sequence, record_hash FROM external_audit_witness ORDER BY sequence DESC LIMIT 1",
        )
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|_| CanonicalStoreError::WitnessWrite {
            operation: "read_head",
        })?;
        let (sequence, previous_hash) = previous.map_or((1, GENESIS_HASH.to_owned()), |row| {
            (row.get::<i64, _>("sequence") + 1, row.get("record_hash"))
        });
        let mut record = WitnessRecord {
            sequence,
            commit_id: commit_id.to_owned(),
            phase: phase.as_str().to_owned(),
            request_hash: request_hash.to_owned(),
            first_sequence,
            last_sequence,
            reason: reason.to_owned(),
            key_id: self.integrity_key_id.clone(),
            previous_hash,
            record_hash: String::new(),
        };
        record.record_hash = witness_record_hash(&self.integrity_key, &record);

        let inserted_sequence: i64 = sqlx::query_scalar(
            r#"
            INSERT INTO external_audit_witness (
                sequence, commit_id, phase, primary_request_hash,
                primary_first_sequence, primary_last_sequence, reason,
                integrity_key_id, previous_hash, record_hash
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            RETURNING sequence
            "#,
        )
        .bind(record.sequence)
        .bind(&record.commit_id)
        .bind(&record.phase)
        .bind(&record.request_hash)
        .bind(record.first_sequence)
        .bind(record.last_sequence)
        .bind(&record.reason)
        .bind(&record.key_id)
        .bind(&record.previous_hash)
        .bind(&record.record_hash)
        .fetch_one(&mut *transaction)
        .await
        .map_err(|_| CanonicalStoreError::WitnessWrite {
            operation: "insert_record",
        })?;
        if inserted_sequence != sequence {
            return Err(CanonicalStoreError::IntegrityViolation(
                "witness_sequence_changed_during_insert",
            ));
        }
        transaction
            .commit()
            .await
            .map_err(|_| CanonicalStoreError::WitnessWrite {
                operation: "commit_record",
            })?;
        Ok(record)
    }

    async fn finalize_witness(
        &self,
        persisted: &PersistedCommit,
        request_hash: &str,
    ) -> Result<(), CanonicalStoreError> {
        match self
            .append_witness(
                &persisted.commit_id,
                WitnessPhase::Committed,
                request_hash,
                Some(persisted.first_event_sequence),
                Some(persisted.last_event_sequence),
                "primary_commit_verified",
            )
            .await
        {
            Ok(_) => Ok(()),
            Err(CanonicalStoreError::IntegrityViolation(reason)) => {
                Err(CanonicalStoreError::IntegrityViolation(reason))
            }
            Err(CanonicalStoreError::IdempotencyConflict) => {
                Err(CanonicalStoreError::IdempotencyConflict)
            }
            Err(_) => Err(CanonicalStoreError::WitnessFinalizationPending {
                commit_id: persisted.commit_id.clone(),
            }),
        }
    }

    async fn load_existing_commit(
        &self,
        commit_id: &str,
        campaign_id: &str,
        stream_id: &str,
        idempotency_key: &str,
    ) -> Result<Option<PersistedCommit>, CanonicalStoreError> {
        let row = if idempotency_key.is_empty() {
            sqlx::query(
                r#"
                SELECT commit_id,
                       (response_payload->>'first_event_sequence')::bigint AS first_event_sequence,
                       result_event_sequence AS last_event_sequence,
                       (response_payload->>'first_stream_version')::bigint AS first_stream_version,
                       (response_payload->>'last_stream_version')::bigint AS last_stream_version,
                       audit_sequence,
                       witness_prepare_sequence, witness_prepare_hash
                  FROM formal_commits WHERE commit_id = $1
                "#,
            )
            .bind(commit_id)
            .fetch_optional(&self.primary)
            .await
        } else {
            sqlx::query(
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
            .fetch_optional(&self.primary)
            .await
        }
        .map_err(|_| CanonicalStoreError::PrimaryWrite {
            operation: "load_existing_commit",
        })?;
        Ok(row.map(|row| persisted_from_row(&row)))
    }

    async fn validate_existing_commit(
        &self,
        persisted: &PersistedCommit,
        draft: &AtomicCommitDraft,
        request_hash: &str,
    ) -> Result<(), CanonicalStoreError> {
        if persisted.commit_id != draft.commit_id {
            return Err(CanonicalStoreError::IdempotencyConflict);
        }
        let stored_hash: String =
            sqlx::query_scalar("SELECT request_hash FROM formal_commits WHERE commit_id = $1")
                .bind(&persisted.commit_id)
                .fetch_one(&self.primary)
                .await
                .map_err(|_| CanonicalStoreError::PrimaryWrite {
                    operation: "load_existing_request_hash",
                })?;
        if stored_hash != request_hash {
            return Err(CanonicalStoreError::IdempotencyConflict);
        }
        let prepared = sqlx::query(
            r#"
            SELECT sequence, record_hash, primary_request_hash
              FROM external_audit_witness
             WHERE commit_id = $1 AND phase = 'PREPARED'
            "#,
        )
        .bind(&persisted.commit_id)
        .fetch_optional(&self.witness)
        .await
        .map_err(|_| CanonicalStoreError::WitnessWrite {
            operation: "load_existing_prepare",
        })?
        .ok_or(CanonicalStoreError::IntegrityViolation(
            "primary_commit_missing_witness_prepare",
        ))?;
        if prepared.get::<i64, _>("sequence") != persisted.witness_prepare_sequence
            || prepared.get::<String, _>("record_hash") != persisted.witness_prepare_hash
            || prepared.get::<String, _>("primary_request_hash") != request_hash
        {
            return Err(CanonicalStoreError::IntegrityViolation(
                "primary_witness_prepare_binding_mismatch",
            ));
        }
        Ok(())
    }

    async fn load_witness_records(&self) -> Result<Vec<WitnessRecord>, CanonicalStoreError> {
        let rows = sqlx::query(
            r#"
            SELECT sequence, commit_id, phase, primary_request_hash,
                   primary_first_sequence, primary_last_sequence, reason,
                   integrity_key_id, previous_hash, record_hash
              FROM external_audit_witness ORDER BY sequence
            "#,
        )
        .fetch_all(&self.witness)
        .await
        .map_err(|_| CanonicalStoreError::WitnessWrite {
            operation: "load_integrity_chain",
        })?;
        Ok(rows.iter().map(witness_from_row).collect())
    }

    async fn load_audit_records(&self) -> Result<Vec<AuditRecord>, CanonicalStoreError> {
        let rows = sqlx::query(
            r#"
            SELECT sequence, commit_id, campaign_id, actor_id, actor_origin,
                   authentication_reference, resource_type, resource_id, action,
                   requested_role, visibility_label, visibility_subject,
                   provenance_kind, provenance_reference, provenance_recorded_by,
                   decision, openfga_decision_id, openfga_policy_revision,
                   opa_decision_id, opa_policy_revision, trace_id, event_batch_hash,
                   witness_prepare_sequence, witness_prepare_hash, occurred_at,
                   integrity_version, integrity_key_id, previous_hash, record_hash
              FROM canonical_audit_log ORDER BY sequence
            "#,
        )
        .fetch_all(&self.primary)
        .await
        .map_err(|_| CanonicalStoreError::PrimaryWrite {
            operation: "load_audit_integrity_chain",
        })?;
        Ok(rows
            .iter()
            .map(|row| AuditRecord {
                sequence: row.get("sequence"),
                commit_id: row.get("commit_id"),
                campaign_id: row.get("campaign_id"),
                actor_id: row.get("actor_id"),
                actor_origin: row.get("actor_origin"),
                authentication_reference: row.get("authentication_reference"),
                resource_type: row.get("resource_type"),
                resource_id: row.get("resource_id"),
                action: row.get("action"),
                requested_role: row.get("requested_role"),
                visibility_label: row.get("visibility_label"),
                visibility_subject: row.get("visibility_subject"),
                provenance_kind: row.get("provenance_kind"),
                provenance_reference: row.get("provenance_reference"),
                provenance_recorded_by: row.get("provenance_recorded_by"),
                decision: row.get("decision"),
                openfga_decision_id: row.get("openfga_decision_id"),
                openfga_policy_revision: row.get("openfga_policy_revision"),
                opa_decision_id: row.get("opa_decision_id"),
                opa_policy_revision: row.get("opa_policy_revision"),
                trace_id: row.get("trace_id"),
                event_batch_hash: row.get("event_batch_hash"),
                witness_prepare_sequence: row.get("witness_prepare_sequence"),
                witness_prepare_hash: row.get("witness_prepare_hash"),
                occurred_at: row.get("occurred_at"),
                integrity_version: row.get("integrity_version"),
                key_id: row.get("integrity_key_id"),
                previous_hash: row.get("previous_hash"),
                record_hash: row.get("record_hash"),
            })
            .collect())
    }
}

/// Production replay query shared by the canonical store and migration
/// verification. Historical unsigned rows remain replayable, but their absent
/// HMAC and unavailable request binding are explicit in every returned record.
pub async fn load_canonical_replay_page(
    pool: &PgPool,
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
               authenticated_actor_id, resource_type, resource_id,
               authority_contract_id, authority_owner, command_id,
               idempotency_key, authority_contract_version,
               visibility_label, visibility_subject,
               fact_provenance_kind, fact_provenance_reference,
               fact_recorded_by, correlation_id, causation_id, trace_id,
               payload_json, event_integrity_hash, request_hash,
               request_hash_source, integrity_status
          FROM event_store
         WHERE campaign_id = $1 AND sequence > $2
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

    rows.into_iter()
        .map(|row| {
            let payload: Json<Value> = row.get("payload_json");
            let event_type: String = row.get("event_type");
            let upcasted = crate::persistence::EventPayloadUpcaster::canonical()
                .upcast(&event_type, row.get("event_schema_version"), payload.0)
                .map_err(|_| {
                    CanonicalStoreError::IntegrityViolation("event_schema_version_unknown")
                })?;
            let event_integrity_hash: Option<String> = row.get("event_integrity_hash");
            let request_hash: String = row.get("request_hash");
            let request_hash_source: String = row.get("request_hash_source");
            let integrity_status: String = row.get("integrity_status");
            if !replay_integrity_metadata_is_valid(
                &integrity_status,
                &request_hash_source,
                &request_hash,
                event_integrity_hash.as_deref(),
            ) {
                return Err(CanonicalStoreError::IntegrityViolation(
                    "event_integrity_metadata_invalid",
                ));
            }
            Ok(CanonicalReplayEvent {
                sequence: row.get("sequence"),
                stream_version: row.get("stream_version"),
                stream_id: row.get("stream_id"),
                event_type,
                campaign_id: row.get("campaign_id"),
                authenticated_actor_id: row.get("authenticated_actor_id"),
                resource_type: row.get("resource_type"),
                resource_id: row.get("resource_id"),
                authority_contract_id: row.get("authority_contract_id"),
                authority_owner: row.get("authority_owner"),
                command_id: row.get("command_id"),
                idempotency_key: row.get("idempotency_key"),
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
                event_integrity_hash,
                request_hash_source,
                integrity_status,
            })
        })
        .collect()
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

fn parse_connection_options(
    url: &str,
    component: &'static str,
) -> Result<PgConnectOptions, CanonicalStoreError> {
    let options = PgConnectOptions::from_str(url)
        .map_err(|_| CanonicalStoreError::Configuration("invalid_postgresql_url"))?;
    let host = options.get_host();
    let local = matches!(host, "localhost" | "127.0.0.1" | "::1") || host.starts_with('/');
    if !local && !matches!(options.get_ssl_mode(), PgSslMode::VerifyFull) {
        return Err(CanonicalStoreError::Configuration(match component {
            "primary" => "remote_primary_postgresql_requires_sslmode_verify_full",
            _ => "remote_witness_postgresql_requires_sslmode_verify_full",
        }));
    }
    Ok(options)
}

fn same_endpoint(primary: &PgConnectOptions, witness: &PgConnectOptions) -> bool {
    primary.get_host() == witness.get_host() && primary.get_port() == witness.get_port()
}

fn normalize_and_validate(
    draft: &AtomicCommitDraft,
) -> Result<AtomicCommitDraft, CanonicalStoreError> {
    if draft.expected_version < 0 {
        return Err(CanonicalStoreError::Validation(
            "non_negative_expected_version_required",
        ));
    }
    if draft.authority_contract_version <= 0 {
        return Err(CanonicalStoreError::Validation(
            "positive_authority_contract_version_required",
        ));
    }
    if draft.events.is_empty() {
        return Err(CanonicalStoreError::Validation(
            "at_least_one_event_required",
        ));
    }
    if draft.events.len() > 256 {
        return Err(CanonicalStoreError::Validation(
            "event_batch_limit_exceeded",
        ));
    }
    let required = [
        draft.commit_id.as_str(),
        draft.campaign_id.as_str(),
        draft.stream_id.as_str(),
        draft.idempotency_key.as_str(),
        draft.command_id.as_str(),
        draft.authenticated_actor_id.as_str(),
        draft.authority_mode.as_str(),
        draft.authority_contract_id.as_str(),
        draft.authority_owner.as_str(),
        draft.visibility_label.as_str(),
        draft.visibility_subject.as_str(),
        draft.provenance_kind.as_str(),
        draft.provenance_reference.as_str(),
        draft.provenance_recorded_by.as_str(),
        draft.correlation_id.as_str(),
        draft.causation_id.as_str(),
        draft.trace_id.as_str(),
        draft.audit.actor_id.as_str(),
        draft.audit.actor_origin.as_str(),
        draft.audit.authentication_reference.as_str(),
        draft.audit.resource_type.as_str(),
        draft.audit.resource_id.as_str(),
        draft.audit.action.as_str(),
        draft.audit.requested_role.as_str(),
        draft.audit.openfga_decision_id.as_str(),
        draft.audit.openfga_policy_revision.as_str(),
        draft.audit.opa_decision_id.as_str(),
        draft.audit.opa_policy_revision.as_str(),
    ];
    if required.iter().any(|value| value.trim().is_empty()) {
        return Err(CanonicalStoreError::Validation("required_field_missing"));
    }
    if !matches!(draft.authority_mode.as_str(), "human_kp" | "ai_kp") {
        return Err(CanonicalStoreError::Validation("unknown_authority_mode"));
    }
    if draft.audit.resource_type == "campaign" && draft.audit.resource_id != draft.campaign_id {
        return Err(CanonicalStoreError::Validation(
            "audit_campaign_resource_mismatch",
        ));
    }
    if draft.stream_id != draft.audit.resource_id {
        return Err(CanonicalStoreError::Validation(
            "stream_audit_resource_mismatch",
        ));
    }
    if !matches!(
        draft.visibility_label.as_str(),
        "public"
            | "party_visible"
            | "keeper_only"
            | "private_to_player"
            | "investigator_private"
            | "ai_internal"
            | "system_only"
            | "system_private"
    ) {
        return Err(CanonicalStoreError::Validation("unknown_visibility_label"));
    }
    if matches!(
        draft.visibility_label.as_str(),
        "private_to_player" | "investigator_private"
    ) == (draft.visibility_subject == "not_applicable")
    {
        return Err(CanonicalStoreError::Validation(
            "visibility_subject_mismatch",
        ));
    }
    if !matches!(
        draft.provenance_kind.as_str(),
        "human_keeper_statement"
            | "rules_engine_decision"
            | "tool_result"
            | "agent_proposal"
            | "imported_source"
            | "system_fixture"
    ) {
        return Err(CanonicalStoreError::Validation("unknown_provenance_kind"));
    }

    let mut normalized = draft.clone();
    for event in &mut normalized.events {
        if event.event_type.trim().is_empty() {
            return Err(CanonicalStoreError::Validation("event_type_required"));
        }
        if event.payload_json.len() > 1_048_576 {
            return Err(CanonicalStoreError::Validation(
                "event_payload_limit_exceeded",
            ));
        }
        let value: Value = serde_json::from_str(&event.payload_json)
            .map_err(|_| CanonicalStoreError::Validation("event_payload_must_be_json"))?;
        event.payload_json = serde_json::to_string(&value)
            .map_err(|_| CanonicalStoreError::Validation("event_payload_must_be_json"))?;
    }
    Ok(normalized)
}

fn request_hash(draft: &AtomicCommitDraft) -> String {
    let mut fields = vec![
        draft.commit_id.clone(),
        draft.campaign_id.clone(),
        draft.stream_id.clone(),
        draft.idempotency_key.clone(),
        draft.expected_version.to_string(),
        draft.command_id.clone(),
        draft.authenticated_actor_id.clone(),
        draft.authority_mode.clone(),
        draft.authority_contract_version.to_string(),
        draft.authority_contract_id.clone(),
        draft.authority_owner.clone(),
        draft.visibility_label.clone(),
        draft.visibility_subject.clone(),
        draft.provenance_kind.clone(),
        draft.provenance_reference.clone(),
        draft.provenance_recorded_by.clone(),
        draft.correlation_id.clone(),
        draft.causation_id.clone(),
        draft.trace_id.clone(),
        draft.audit.actor_id.clone(),
        draft.audit.actor_origin.clone(),
        draft.audit.authentication_reference.clone(),
        draft.audit.resource_type.clone(),
        draft.audit.resource_id.clone(),
        draft.audit.action.clone(),
        draft.audit.requested_role.clone(),
        draft.audit.openfga_decision_id.clone(),
        draft.audit.openfga_policy_revision.clone(),
        draft.audit.opa_decision_id.clone(),
        draft.audit.opa_policy_revision.clone(),
        draft.events.len().to_string(),
    ];
    for event in &draft.events {
        fields.push(event.event_type.clone());
        fields.push(event.payload_json.clone());
    }
    sha256_fields(&fields)
}

fn event_integrity_hash(
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
    if record.integrity_version == 2 {
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
        if !matches!(record.integrity_version, 1 | 2) {
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

fn replay_integrity_metadata_is_valid(
    integrity_status: &str,
    request_hash_source: &str,
    request_hash: &str,
    event_integrity_hash: Option<&str>,
) -> bool {
    match (integrity_status, request_hash_source) {
        ("verified_hmac", "formal_commit") => {
            event_integrity_hash.is_some() && request_hash != ZERO_REQUEST_HASH
        }
        ("historical_unverified_hmac", "formal_commit") => {
            event_integrity_hash.is_some() && request_hash != ZERO_REQUEST_HASH
        }
        ("historical_unsigned", "historical_unavailable") => {
            event_integrity_hash.is_none() && request_hash == ZERO_REQUEST_HASH
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use trpg_shared_kernel::{CanonicalCommitEvent, CanonicalPolicyAudit};

    #[test]
    fn field_encoding_prevents_separator_ambiguity() {
        assert_ne!(
            sha256_fields(&["a|b".to_owned(), "c".to_owned()]),
            sha256_fields(&["a".to_owned(), "b|c".to_owned()])
        );
    }

    #[test]
    fn canonical_request_maps_the_authorized_resource_to_the_database_stream() {
        let request = CanonicalCommitRequest {
            commit_id: "commit_scene_alpha".to_owned(),
            campaign_id: "campaign_mapping".to_owned(),
            idempotency_key: "mapping_key".to_owned(),
            expected_version: 0,
            command_id: "command_mapping".to_owned(),
            authenticated_actor_id: "workflow_mapping".to_owned(),
            authority_mode: "human_kp".to_owned(),
            authority_contract_version: 1,
            authority_contract_id: "authority_mapping".to_owned(),
            authority_owner: "keeper_mapping".to_owned(),
            visibility_label: "party_visible".to_owned(),
            visibility_subject: "not_applicable".to_owned(),
            provenance_kind: "rules_engine_decision".to_owned(),
            provenance_reference: "decision_mapping".to_owned(),
            provenance_recorded_by: "rules_engine_mapping".to_owned(),
            correlation_id: "correlation_mapping".to_owned(),
            causation_id: "causation_mapping".to_owned(),
            trace_id: "trace_mapping".to_owned(),
            events: vec![CanonicalCommitEvent {
                event_type: "SceneAdvanced".to_owned(),
                payload_json: "{}".to_owned(),
            }],
            audit: CanonicalPolicyAudit {
                actor_id: "keeper_mapping".to_owned(),
                actor_origin: "user_session".to_owned(),
                authentication_reference: "session_mapping".to_owned(),
                resource_type: "scene".to_owned(),
                resource_id: "scene_alpha".to_owned(),
                action: "write_official_state".to_owned(),
                requested_role: "human_keeper".to_owned(),
                openfga_decision_id: "fga_mapping".to_owned(),
                openfga_policy_revision: "fga_revision_mapping".to_owned(),
                opa_decision_id: "opa_mapping".to_owned(),
                opa_policy_revision: "opa_revision_mapping".to_owned(),
            },
        };
        let draft = canonical_request_draft(&request).unwrap();
        assert_eq!(draft.campaign_id, "campaign_mapping");
        assert_eq!(draft.stream_id, "scene_alpha");
        assert_eq!(draft.stream_id, draft.audit.resource_id);
        assert!(normalize_and_validate(&draft).is_ok());
    }

    #[test]
    fn audit_integrity_v1_remains_compatible_with_pre_p03_records() {
        let mut record = AuditRecord {
            sequence: 1,
            commit_id: "success".to_owned(),
            campaign_id: "campaign_atomic_commit".to_owned(),
            actor_id: "keeper_atomic_commit".to_owned(),
            actor_origin: "user_session".to_owned(),
            authentication_reference: "session_atomic_commit".to_owned(),
            resource_type: "campaign".to_owned(),
            resource_id: "campaign_atomic_commit".to_owned(),
            action: "write_official_state".to_owned(),
            requested_role: "human_keeper".to_owned(),
            visibility_label: "party_visible".to_owned(),
            visibility_subject: "not_applicable".to_owned(),
            provenance_kind: "rules_engine_decision".to_owned(),
            provenance_reference: "decision_success".to_owned(),
            provenance_recorded_by: "rules_engine_atomic_commit".to_owned(),
            decision: "PERMIT".to_owned(),
            openfga_decision_id: "fga_success".to_owned(),
            openfga_policy_revision: "fga_model_atomic_commit".to_owned(),
            opa_decision_id: "opa_success".to_owned(),
            opa_policy_revision: "opa_bundle_atomic_commit".to_owned(),
            trace_id: "trace_success".to_owned(),
            event_batch_hash:
                "sha256:f84537114f6cf20ae34cf69c92384ecc45b7247fea73d4aac7deb76eb34d4cc3".to_owned(),
            witness_prepare_sequence: 1,
            witness_prepare_hash:
                "hmac-sha256:426f0375be7bb6ec0632d2cfed79d9c112039bc372f3acf3a7fe9c8b903ad78b"
                    .to_owned(),
            occurred_at: "2026-07-15T16:43:12.930939Z".parse().unwrap(),
            integrity_version: 1,
            key_id: "p02-canonical-test-key".to_owned(),
            previous_hash: GENESIS_HASH.to_owned(),
            record_hash: String::new(),
        };
        let expected =
            "hmac-sha256:c8222f856a745c6527633b334c44b4bd980c95e432d8423656d06f3916bbbbae";
        assert_eq!(audit_record_hash(&[0x9c; 32], &record), expected);

        // v1 intentionally retains its historical input set; timestamp
        // binding begins only at v2 so old signed records do not need re-signing.
        record.occurred_at += chrono::TimeDelta::days(1);
        assert_eq!(audit_record_hash(&[0x9c; 32], &record), expected);
    }

    #[test]
    fn remote_postgresql_is_fail_closed_without_hostname_verification() {
        let error = parse_connection_options(
            "postgresql://app@example.invalid/trpg?sslmode=require",
            "primary",
        )
        .unwrap_err();
        assert_eq!(
            error,
            CanonicalStoreError::Configuration(
                "remote_primary_postgresql_requires_sslmode_verify_full"
            )
        );
        assert!(parse_connection_options(
            "postgresql://app@example.invalid/trpg?sslmode=verify-full",
            "primary"
        )
        .is_ok());
    }

    #[test]
    fn replay_integrity_metadata_rejects_mixed_states() {
        let verified_hash = format!("hmac-sha256:{}", "a".repeat(64));
        let formal_hash = format!("sha256:{}", "b".repeat(64));
        assert!(replay_integrity_metadata_is_valid(
            "verified_hmac",
            "formal_commit",
            &formal_hash,
            Some(&verified_hash),
        ));
        assert!(replay_integrity_metadata_is_valid(
            "historical_unsigned",
            "historical_unavailable",
            ZERO_REQUEST_HASH,
            None,
        ));
        assert!(replay_integrity_metadata_is_valid(
            "historical_unverified_hmac",
            "formal_commit",
            &formal_hash,
            Some(&verified_hash),
        ));
        assert!(!replay_integrity_metadata_is_valid(
            "verified_hmac",
            "historical_unavailable",
            ZERO_REQUEST_HASH,
            Some(&verified_hash),
        ));
        assert!(!replay_integrity_metadata_is_valid(
            "historical_unsigned",
            "formal_commit",
            &formal_hash,
            None,
        ));
        assert!(!replay_integrity_metadata_is_valid(
            "unknown",
            "formal_commit",
            &formal_hash,
            Some(&verified_hash),
        ));
    }
}
