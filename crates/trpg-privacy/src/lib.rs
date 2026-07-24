use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use redis::aio::ConnectionManager;
use ring::{aead, rand as ring_rand};
use s3::{creds::Credentials, serde_types::ObjectIdentifier, Bucket, Region};
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::migrate::Migrator;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions, PgSslMode};
use sqlx::{PgPool, Row};
use trpg_security_governance::cloud_egress::{
    CloudConsentQuery, CloudEgressAuditRecord, CloudEgressDecision, CloudEgressDenial,
    CloudEgressLedger, CloudRouteSnapshotRecord, ConsentVisibilityScope, PersistedCloudConsent,
};
use trpg_shared_kernel::{EntityId, KernelResult, TrpgError};
use url::Url;
use zeroize::{Zeroize, Zeroizing};

pub const REQUIRED_DELETION_TARGETS: [DeletionTarget; 7] = [
    DeletionTarget::Database,
    DeletionTarget::RagIndex,
    DeletionTarget::ObjectStorage,
    DeletionTarget::Cache,
    DeletionTarget::Export,
    DeletionTarget::BackupKey,
    DeletionTarget::Queue,
];

/// The repository-wide migration tree is the sole executable schema source.
/// Privacy must not carry a second Rust-owned copy of its DDL.
static MIGRATOR: Migrator = sqlx::migrate!("../../migrations");

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeletionTarget {
    Database,
    RagIndex,
    ObjectStorage,
    Cache,
    Queue,
    Export,
    BackupKey,
}

impl DeletionTarget {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Database => "database",
            Self::RagIndex => "rag_index",
            Self::ObjectStorage => "object_storage",
            Self::Cache => "cache",
            Self::Queue => "queue",
            Self::Export => "export",
            Self::BackupKey => "backup_key",
        }
    }

    fn parse(value: &str) -> Result<Self, PrivacyError> {
        match value {
            "database" => Ok(Self::Database),
            "rag_index" => Ok(Self::RagIndex),
            "object_storage" => Ok(Self::ObjectStorage),
            "cache" => Ok(Self::Cache),
            "queue" => Ok(Self::Queue),
            "export" => Ok(Self::Export),
            "backup_key" => Ok(Self::BackupKey),
            _ => Err(PrivacyError::InvalidPersistedState),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeletionJobStatus {
    Requested,
    BlockedLegalHold,
    Running,
    Verifying,
    Completed,
    Failed,
}

impl DeletionJobStatus {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Requested => "requested",
            Self::BlockedLegalHold => "blocked_legal_hold",
            Self::Running => "running",
            Self::Verifying => "verifying",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }

    fn parse(value: &str) -> Result<Self, PrivacyError> {
        match value {
            "requested" => Ok(Self::Requested),
            "blocked_legal_hold" => Ok(Self::BlockedLegalHold),
            "running" => Ok(Self::Running),
            "verifying" => Ok(Self::Verifying),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            _ => Err(PrivacyError::InvalidPersistedState),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeletionTargetStatus {
    Pending,
    Deleted,
    Verified,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeletionEvidenceStatus {
    Pending,
    Confirmed,
}

impl DeletionEvidenceStatus {
    fn parse(value: &str) -> Result<Self, PrivacyError> {
        match value {
            "pending" => Ok(Self::Pending),
            "confirmed" => Ok(Self::Confirmed),
            _ => Err(PrivacyError::InvalidPersistedState),
        }
    }
}

/// Immutable identity of the canonical deletion-request event expected by a
/// deletion job. A job is deliberately non-executable until the corresponding
/// event sequence and integrity digest have been confirmed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeletionRequestEvidence {
    campaign_id: EntityId,
    command_id: EntityId,
    correlation_id: EntityId,
    causation_id: EntityId,
    event_type: String,
}

impl DeletionRequestEvidence {
    pub fn new(
        campaign_id: impl Into<String>,
        command_id: impl Into<String>,
        correlation_id: impl Into<String>,
        causation_id: impl Into<String>,
        event_type: impl Into<String>,
    ) -> Result<Self, PrivacyError> {
        let event_type = event_type.into();
        if event_type.is_empty()
            || event_type.len() > 160
            || !event_type
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
        {
            return Err(PrivacyError::InvalidInput);
        }
        Ok(Self {
            campaign_id: EntityId::new(campaign_id).map_err(|_| PrivacyError::InvalidInput)?,
            command_id: EntityId::new(command_id).map_err(|_| PrivacyError::InvalidInput)?,
            correlation_id: EntityId::new(correlation_id)
                .map_err(|_| PrivacyError::InvalidInput)?,
            causation_id: EntityId::new(causation_id).map_err(|_| PrivacyError::InvalidInput)?,
            event_type,
        })
    }

    pub fn campaign_id(&self) -> &EntityId {
        &self.campaign_id
    }

    pub fn command_id(&self) -> &EntityId {
        &self.command_id
    }

    pub fn correlation_id(&self) -> &EntityId {
        &self.correlation_id
    }

    pub fn causation_id(&self) -> &EntityId {
        &self.causation_id
    }

    pub fn event_type(&self) -> &str {
        &self.event_type
    }
}

impl DeletionTargetStatus {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Deleted => "deleted",
            Self::Verified => "verified",
            Self::Failed => "failed",
        }
    }

    fn parse(value: &str) -> Result<Self, PrivacyError> {
        match value {
            "pending" => Ok(Self::Pending),
            "deleted" => Ok(Self::Deleted),
            "verified" => Ok(Self::Verified),
            "failed" => Ok(Self::Failed),
            _ => Err(PrivacyError::InvalidPersistedState),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeletionTargetRecord {
    pub target: DeletionTarget,
    pub status: DeletionTargetStatus,
    pub error_code: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeletionJob {
    pub job_id: String,
    pub campaign_id: String,
    pub subject_id: String,
    pub requested_by: String,
    pub retention_policy: String,
    pub status: DeletionJobStatus,
    pub failure_code: Option<String>,
    pub evidence_status: DeletionEvidenceStatus,
    pub canonical_event_sequence: Option<u64>,
    pub canonical_event_integrity_hash: Option<String>,
    pub targets: Vec<DeletionTargetRecord>,
}

impl DeletionJob {
    pub fn all_targets_verified(&self) -> bool {
        REQUIRED_DELETION_TARGETS.iter().all(|required| {
            self.targets.iter().any(|record| {
                record.target == *required && record.status == DeletionTargetStatus::Verified
            })
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PrivacyError {
    InvalidInput,
    Database,
    Storage,
    Cache,
    Queue,
    JobNotFound,
    MissingSurface(DeletionTarget),
    VerificationFailed(DeletionTarget),
    InvalidPersistedState,
    EvidenceUnconfirmed,
    LegacyTwoPhaseDisabled,
    JobAlreadyRunning,
    DeletionInProgress,
    ProtectedCanonicalIdentity,
    Cryptography,
}

impl PrivacyError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidInput => "PRIVACY_INVALID_INPUT",
            Self::Database => "PRIVACY_DATABASE_ERROR",
            Self::Storage => "PRIVACY_STORAGE_ERROR",
            Self::Cache => "PRIVACY_CACHE_ERROR",
            Self::Queue => "PRIVACY_QUEUE_ERROR",
            Self::JobNotFound => "DELETION_JOB_NOT_FOUND",
            Self::MissingSurface(_) => "DELETION_SURFACE_MISSING",
            Self::VerificationFailed(_) => "DELETION_VERIFICATION_FAILED",
            Self::InvalidPersistedState => "PRIVACY_PERSISTED_STATE_INVALID",
            Self::EvidenceUnconfirmed => "DELETION_CANONICAL_EVIDENCE_UNCONFIRMED",
            Self::LegacyTwoPhaseDisabled => "DELETION_TWO_PHASE_REQUEST_DISABLED",
            Self::JobAlreadyRunning => "DELETION_JOB_ALREADY_RUNNING",
            Self::DeletionInProgress => "DELETION_IN_PROGRESS",
            Self::ProtectedCanonicalIdentity => "DELETION_CANONICAL_IDENTITY_REQUIRES_FORK",
            Self::Cryptography => "PRIVACY_CRYPTOGRAPHY_ERROR",
        }
    }
}

impl fmt::Display for PrivacyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

impl std::error::Error for PrivacyError {}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ProtectedPayload {
    algorithm: String,
    key_reference: String,
    nonce: String,
    ciphertext: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ProtectedPayloadEnvelope {
    protected_payload: ProtectedPayload,
}

/// AES-256-GCM field protection for canonical event/outbox payloads. The key
/// and decrypted bytes are zeroized and this type deliberately has no Debug or
/// Clone implementation.
pub struct PayloadCipher {
    key_reference: EntityId,
    key: Zeroizing<[u8; 32]>,
}

/// Zeroizing plaintext view with no Debug/Clone implementation.
pub struct DecryptedPayload(Zeroizing<Vec<u8>>);

/// Ciphertext metadata suitable for separate database columns. It contains no
/// plaintext and deliberately has no Debug implementation.
pub struct EncryptedPayload {
    envelope: serde_json::Value,
    ciphertext: Vec<u8>,
    nonce: [u8; 12],
    key_reference: EntityId,
}

impl EncryptedPayload {
    pub fn envelope(&self) -> &serde_json::Value {
        &self.envelope
    }

    pub fn ciphertext(&self) -> &[u8] {
        &self.ciphertext
    }

    pub const fn nonce(&self) -> &[u8; 12] {
        &self.nonce
    }

    pub fn key_reference(&self) -> &EntityId {
        &self.key_reference
    }

    pub fn into_envelope(self) -> serde_json::Value {
        self.envelope
    }
}

impl DecryptedPayload {
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_slice()
    }
}

impl PayloadCipher {
    pub fn new(key_reference: impl Into<String>, key: &[u8]) -> Result<Self, PrivacyError> {
        if key.len() != 32 {
            return Err(PrivacyError::InvalidInput);
        }
        let mut protected_key = Zeroizing::new([0_u8; 32]);
        protected_key.copy_from_slice(key);
        Ok(Self {
            key_reference: EntityId::new(key_reference).map_err(|_| PrivacyError::InvalidInput)?,
            key: protected_key,
        })
    }

    pub fn key_reference(&self) -> &EntityId {
        &self.key_reference
    }

    pub fn encrypt_json(
        &self,
        plaintext_json: &[u8],
        associated_fields: &[&str],
    ) -> Result<serde_json::Value, PrivacyError> {
        self.encrypt_json_field(plaintext_json, associated_fields)
            .map(EncryptedPayload::into_envelope)
    }

    pub fn encrypt_json_field(
        &self,
        plaintext_json: &[u8],
        associated_fields: &[&str],
    ) -> Result<EncryptedPayload, PrivacyError> {
        if plaintext_json.is_empty() || plaintext_json.len() > 1_048_576 {
            return Err(PrivacyError::InvalidInput);
        }
        let key = aead::UnboundKey::new(&aead::AES_256_GCM, self.key.as_slice())
            .map_err(|_| PrivacyError::Cryptography)?;
        let key = aead::LessSafeKey::new(key);
        let random = ring_rand::SystemRandom::new();
        let mut nonce_bytes = [0_u8; 12];
        ring_rand::SecureRandom::fill(&random, &mut nonce_bytes)
            .map_err(|_| PrivacyError::Cryptography)?;
        let nonce = aead::Nonce::assume_unique_for_key(nonce_bytes);
        let aad = associated_data(associated_fields)?;
        let mut ciphertext = plaintext_json.to_vec();
        key.seal_in_place_append_tag(nonce, aead::Aad::from(aad.as_slice()), &mut ciphertext)
            .map_err(|_| PrivacyError::Cryptography)?;
        let envelope = ProtectedPayloadEnvelope {
            protected_payload: ProtectedPayload {
                algorithm: "AES-256-GCM".to_owned(),
                key_reference: self.key_reference.to_string(),
                nonce: BASE64.encode(nonce_bytes),
                ciphertext: BASE64.encode(&ciphertext),
            },
        };
        Ok(EncryptedPayload {
            envelope: serde_json::to_value(envelope).map_err(|_| PrivacyError::Cryptography)?,
            ciphertext,
            nonce: nonce_bytes,
            key_reference: self.key_reference.clone(),
        })
    }

    pub fn decrypt_json(
        &self,
        envelope: &serde_json::Value,
        associated_fields: &[&str],
    ) -> Result<DecryptedPayload, PrivacyError> {
        let envelope: ProtectedPayloadEnvelope =
            serde_json::from_value(envelope.clone()).map_err(|_| PrivacyError::Cryptography)?;
        let protected = envelope.protected_payload;
        if protected.algorithm != "AES-256-GCM"
            || protected.key_reference != self.key_reference.as_str()
        {
            return Err(PrivacyError::Cryptography);
        }
        let nonce = BASE64
            .decode(protected.nonce)
            .map_err(|_| PrivacyError::Cryptography)?;
        let nonce: [u8; 12] = nonce.try_into().map_err(|_| PrivacyError::Cryptography)?;
        let mut ciphertext = Zeroizing::new(
            BASE64
                .decode(protected.ciphertext)
                .map_err(|_| PrivacyError::Cryptography)?,
        );
        let key = aead::UnboundKey::new(&aead::AES_256_GCM, self.key.as_slice())
            .map_err(|_| PrivacyError::Cryptography)?;
        let key = aead::LessSafeKey::new(key);
        let aad = associated_data(associated_fields)?;
        let plaintext = key
            .open_in_place(
                aead::Nonce::assume_unique_for_key(nonce),
                aead::Aad::from(aad.as_slice()),
                ciphertext.as_mut_slice(),
            )
            .map_err(|_| PrivacyError::Cryptography)?;
        let plaintext_len = plaintext.len();
        ciphertext.truncate(plaintext_len);
        Ok(DecryptedPayload(ciphertext))
    }
}

impl Drop for PayloadCipher {
    fn drop(&mut self) {
        self.key.zeroize();
    }
}

fn associated_data(fields: &[&str]) -> Result<Zeroizing<Vec<u8>>, PrivacyError> {
    if fields.is_empty() || fields.iter().any(|field| field.len() > 65_536) {
        return Err(PrivacyError::InvalidInput);
    }
    let mut result = Zeroizing::new(Vec::new());
    for field in fields {
        let length = u32::try_from(field.len()).map_err(|_| PrivacyError::InvalidInput)?;
        result.extend_from_slice(&length.to_be_bytes());
        result.extend_from_slice(field.as_bytes());
    }
    Ok(result)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CloudConsentGrant {
    pub consent_id: EntityId,
    pub subject_id: EntityId,
    pub target_provider: EntityId,
    pub purpose: EntityId,
    pub policy_version: EntityId,
    pub visibility_scope: ConsentVisibilityScope,
    pub expires_at_unix_ms: u64,
}

#[derive(Clone)]
pub struct PostgresCloudEgressLedger {
    pool: PgPool,
}

impl PostgresCloudEgressLedger {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn load_recorded_decision(
        &self,
        snapshot_id: &EntityId,
    ) -> KernelResult<(String, Option<String>, String)> {
        let row = sqlx::query(
            "SELECT route.decision, route.denial_code, audit.context_manifest_hash \
             FROM cloud_egress_route_snapshots route \
             JOIN cloud_egress_audit audit ON audit.snapshot_id = route.snapshot_id \
             WHERE route.snapshot_id = $1",
        )
        .bind(snapshot_id.as_str())
        .fetch_one(&self.pool)
        .await
        .map_err(|_| TrpgError::PolicyEvidenceUntrusted)?;
        Ok((
            row.try_get("decision")
                .map_err(|_| TrpgError::PolicyEvidenceUntrusted)?,
            row.try_get("denial_code")
                .map_err(|_| TrpgError::PolicyEvidenceUntrusted)?,
            row.try_get("context_manifest_hash")
                .map_err(|_| TrpgError::PolicyEvidenceUntrusted)?,
        ))
    }
}

#[async_trait]
impl CloudEgressLedger for PostgresCloudEgressLedger {
    async fn trusted_now_unix_ms(&self) -> KernelResult<u64> {
        let now: i64 = sqlx::query_scalar(
            "SELECT floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|_| TrpgError::PolicyUnavailable)?;
        u64::try_from(now).map_err(|_| TrpgError::PolicyEvidenceUntrusted)
    }

    async fn notice_is_recorded(
        &self,
        notice_reference: &EntityId,
        subject_id: &EntityId,
        policy_version: &EntityId,
    ) -> KernelResult<bool> {
        sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM cloud_egress_notices \
             WHERE notice_reference = $1 AND subject_id = $2 AND policy_version = $3)",
        )
        .bind(notice_reference.as_str())
        .bind(subject_id.as_str())
        .bind(policy_version.as_str())
        .fetch_one(&self.pool)
        .await
        .map_err(|_| TrpgError::PolicyUnavailable)
    }

    async fn load_active_consent(
        &self,
        query: &CloudConsentQuery,
    ) -> KernelResult<Option<PersistedCloudConsent>> {
        let now =
            i64::try_from(query.now_unix_ms).map_err(|_| TrpgError::PolicyEvidenceUntrusted)?;
        let row = sqlx::query(
            "SELECT consent_id, subject_id, target_provider, purpose, policy_version, \
             visibility_scope, expires_at_unix_ms \
             FROM cloud_egress_consents \
             WHERE subject_id = $1 AND target_provider = $2 AND purpose = $3 \
               AND policy_version = $4 AND granted = true AND expires_at_unix_ms > $5 \
             ORDER BY updated_at DESC, consent_id DESC LIMIT 1",
        )
        .bind(query.subject_id.as_str())
        .bind(query.target_provider.as_str())
        .bind(query.purpose.as_str())
        .bind(query.policy_version.as_str())
        .bind(now)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| TrpgError::PolicyUnavailable)?;
        let Some(row) = row else {
            return Ok(None);
        };
        let expires_at: i64 = row
            .try_get("expires_at_unix_ms")
            .map_err(|_| TrpgError::PolicyEvidenceUntrusted)?;
        let expires_at =
            u64::try_from(expires_at).map_err(|_| TrpgError::PolicyEvidenceUntrusted)?;
        let id = |column: &str| -> KernelResult<EntityId> {
            let value: String = row
                .try_get(column)
                .map_err(|_| TrpgError::PolicyEvidenceUntrusted)?;
            EntityId::new(value).map_err(|_| TrpgError::PolicyEvidenceUntrusted)
        };
        let visibility_scope: String = row
            .try_get("visibility_scope")
            .map_err(|_| TrpgError::PolicyEvidenceUntrusted)?;
        Ok(Some(PersistedCloudConsent::loaded_from_repository(
            id("consent_id")?,
            id("subject_id")?,
            id("target_provider")?,
            id("purpose")?,
            id("policy_version")?,
            ConsentVisibilityScope::parse(&visibility_scope)?,
            expires_at,
        )))
    }

    async fn record_route_decision(
        &self,
        mut snapshot: CloudRouteSnapshotRecord,
        mut audit: CloudEgressAuditRecord,
    ) -> KernelResult<bool> {
        if snapshot.snapshot_id != audit.snapshot_id
            || snapshot.subject_id != audit.subject_id
            || snapshot.source_provider != audit.source_provider
            || snapshot.target_provider != audit.target_provider
            || snapshot.source_endpoint != audit.source_endpoint
            || snapshot.target_endpoint != audit.target_endpoint
            || snapshot.model_id != audit.model_id
            || snapshot.source_credential_id != audit.source_credential_id
            || snapshot.source_credential_version != audit.source_credential_version
            || snapshot.target_credential_id != audit.target_credential_id
            || snapshot.target_credential_version != audit.target_credential_version
            || snapshot.fallback_policy != audit.fallback_policy
            || snapshot.privacy_boundary != audit.privacy_boundary
            || snapshot.decision != audit.decision
            || snapshot.denial_code != audit.denial_code
            || snapshot.context_manifest_hash != audit.context_manifest_hash
            || snapshot.created_at_unix_ms != audit.created_at_unix_ms
        {
            return Err(TrpgError::PolicyEvidenceUntrusted);
        }
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|_| TrpgError::PolicyUnavailable)?;
        let mut persisted_as_candidate = true;
        if snapshot.decision == CloudEgressDecision::Allow {
            let Some(consent_id) = snapshot.consent_id.as_ref() else {
                return Err(TrpgError::PolicyEvidenceUntrusted);
            };
            let consent_expires_at = snapshot
                .consent_expires_at_unix_ms
                .ok_or(TrpgError::PolicyEvidenceUntrusted)
                .and_then(|value| {
                    i64::try_from(value).map_err(|_| TrpgError::PolicyEvidenceUntrusted)
                })?;
            let notice_reference = snapshot
                .notice_reference
                .as_ref()
                .ok_or(TrpgError::PolicyEvidenceUntrusted)?;
            let still_active = sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS (SELECT 1 FROM cloud_egress_consents \
                 WHERE consent_id = $1 AND subject_id = $2 AND target_provider = $3 \
                   AND purpose = $4 AND policy_version = $5 AND granted = true \
                   AND expires_at_unix_ms = $6 \
                   AND expires_at_unix_ms > \
                       floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint \
                   AND EXISTS (SELECT 1 FROM cloud_egress_notices \
                               WHERE notice_reference = $7 AND subject_id = $2 \
                                 AND policy_version = $5) \
                   FOR SHARE)",
            )
            .bind(consent_id.as_str())
            .bind(snapshot.subject_id.as_str())
            .bind(snapshot.target_provider.as_str())
            .bind(snapshot.purpose.as_str())
            .bind(snapshot.policy_version.as_str())
            .bind(consent_expires_at)
            .bind(notice_reference.as_str())
            .fetch_one(&mut *transaction)
            .await
            .map_err(|_| TrpgError::PolicyUnavailable)?;
            if !still_active {
                persisted_as_candidate = false;
                snapshot.decision = CloudEgressDecision::Deny;
                snapshot.denial_code = Some(CloudEgressDenial::ConsentChanged.code());
                snapshot.allowed_fact_ids.clear();
                audit.decision = CloudEgressDecision::Deny;
                audit.denial_code = Some(CloudEgressDenial::ConsentChanged.code());
            }
        }
        let allowed_fact_ids = serde_json::Value::Array(
            snapshot
                .allowed_fact_ids
                .iter()
                .map(|id| serde_json::Value::String(id.to_string()))
                .collect(),
        );
        let created_at = i64::try_from(snapshot.created_at_unix_ms)
            .map_err(|_| TrpgError::PolicyEvidenceUntrusted)?;
        sqlx::query(
            "INSERT INTO cloud_egress_route_snapshots (\
             snapshot_id, subject_id, consent_id, source_provider, target_provider, purpose, \
             policy_version, notice_reference, context_manifest_hash, allowed_fact_ids, \
             decision, denial_code, created_at_unix_ms, source_endpoint, target_endpoint, model_id, \
             source_credential_id, source_credential_version, target_credential_id, \
             target_credential_version, fallback_policy, privacy_boundary, \
             consent_expires_at_unix_ms) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, \
                     $17, $18, $19, $20, $21, $22, $23)",
        )
        .bind(snapshot.snapshot_id.as_str())
        .bind(snapshot.subject_id.as_str())
        .bind(snapshot.consent_id.as_ref().map(EntityId::as_str))
        .bind(snapshot.source_provider.as_str())
        .bind(snapshot.target_provider.as_str())
        .bind(snapshot.purpose.as_str())
        .bind(snapshot.policy_version.as_str())
        .bind(snapshot.notice_reference.as_ref().map(EntityId::as_str))
        .bind(&snapshot.context_manifest_hash)
        .bind(allowed_fact_ids)
        .bind(snapshot.decision.as_str())
        .bind(snapshot.denial_code)
        .bind(created_at)
        .bind(&snapshot.source_endpoint)
        .bind(&snapshot.target_endpoint)
        .bind(snapshot.model_id.as_str())
        .bind(&snapshot.source_credential_id)
        .bind(i64::try_from(snapshot.source_credential_version)
            .map_err(|_| TrpgError::PolicyEvidenceUntrusted)?)
        .bind(&snapshot.target_credential_id)
        .bind(i64::try_from(snapshot.target_credential_version)
            .map_err(|_| TrpgError::PolicyEvidenceUntrusted)?)
        .bind(snapshot.fallback_policy.as_str())
        .bind(snapshot.privacy_boundary.as_str())
        .bind(
            snapshot
                .consent_expires_at_unix_ms
                .map(i64::try_from)
                .transpose()
                .map_err(|_| TrpgError::PolicyEvidenceUntrusted)?,
        )
        .execute(&mut *transaction)
        .await
        .map_err(|_| TrpgError::PolicyUnavailable)?;
        sqlx::query(
            "INSERT INTO cloud_egress_audit (\
             audit_id, snapshot_id, subject_id, decision, denial_code, \
             context_manifest_hash, created_at_unix_ms, source_provider, target_provider, \
             source_endpoint, target_endpoint, model_id, source_credential_id, \
             source_credential_version, target_credential_id, target_credential_version, \
             fallback_policy, privacy_boundary) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, \
                     $13, $14, $15, $16, $17, $18)",
        )
        .bind(audit.audit_id.as_str())
        .bind(audit.snapshot_id.as_str())
        .bind(audit.subject_id.as_str())
        .bind(audit.decision.as_str())
        .bind(audit.denial_code)
        .bind(&audit.context_manifest_hash)
        .bind(created_at)
        .bind(audit.source_provider.as_str())
        .bind(audit.target_provider.as_str())
        .bind(&audit.source_endpoint)
        .bind(&audit.target_endpoint)
        .bind(audit.model_id.as_str())
        .bind(&audit.source_credential_id)
        .bind(
            i64::try_from(audit.source_credential_version)
                .map_err(|_| TrpgError::PolicyEvidenceUntrusted)?,
        )
        .bind(&audit.target_credential_id)
        .bind(
            i64::try_from(audit.target_credential_version)
                .map_err(|_| TrpgError::PolicyEvidenceUntrusted)?,
        )
        .bind(audit.fallback_policy.as_str())
        .bind(audit.privacy_boundary.as_str())
        .execute(&mut *transaction)
        .await
        .map_err(|_| TrpgError::PolicyUnavailable)?;
        transaction
            .commit()
            .await
            .map_err(|_| TrpgError::PolicyUnavailable)?;
        Ok(persisted_as_candidate)
    }
}

#[derive(Clone)]
pub struct PostgresDeletionRepository {
    pool: PgPool,
}

/// Canonical event and workflow fields that must be persisted atomically when
/// a deletion request becomes executable.
pub struct ConfirmedDeletionRecord<'a> {
    pub job_id: &'a str,
    pub subject_id: &'a str,
    pub requested_by: &'a str,
    pub retention_policy: &'a str,
    pub evidence: &'a DeletionRequestEvidence,
    pub canonical_event_sequence: u64,
    pub canonical_event_integrity_hash: &'a str,
}

#[async_trait]
pub trait DeletionRequestPort: Send + Sync {
    async fn request_deletion(
        &self,
        job_id: &str,
        subject_id: &str,
        requested_by: &str,
        retention_policy: &str,
        evidence: &DeletionRequestEvidence,
    ) -> Result<DeletionJob, PrivacyError>;

    async fn confirm_deletion_evidence(
        &self,
        job_id: &str,
        evidence: &DeletionRequestEvidence,
        canonical_event_sequence: u64,
        canonical_event_integrity_hash: &str,
    ) -> Result<DeletionJob, PrivacyError>;

    /// Atomically creates the workflow job and all required targets from an
    /// already verified canonical event. Production callers use this method so
    /// a failed canonical commit cannot leave a pending side-table job.
    async fn record_confirmed_deletion(
        &self,
        record: ConfirmedDeletionRecord<'_>,
    ) -> Result<DeletionJob, PrivacyError>;
}

#[async_trait]
impl DeletionRequestPort for PostgresDeletionRepository {
    async fn request_deletion(
        &self,
        job_id: &str,
        subject_id: &str,
        requested_by: &str,
        retention_policy: &str,
        evidence: &DeletionRequestEvidence,
    ) -> Result<DeletionJob, PrivacyError> {
        self.request(job_id, subject_id, requested_by, retention_policy, evidence)
            .await
    }

    async fn confirm_deletion_evidence(
        &self,
        job_id: &str,
        evidence: &DeletionRequestEvidence,
        canonical_event_sequence: u64,
        canonical_event_integrity_hash: &str,
    ) -> Result<DeletionJob, PrivacyError> {
        self.confirm_evidence(
            job_id,
            evidence,
            canonical_event_sequence,
            canonical_event_integrity_hash,
        )
        .await
    }

    async fn record_confirmed_deletion(
        &self,
        record: ConfirmedDeletionRecord<'_>,
    ) -> Result<DeletionJob, PrivacyError> {
        self.record_confirmed(
            record.job_id,
            record.subject_id,
            record.requested_by,
            record.retention_policy,
            record.evidence,
            record.canonical_event_sequence,
            record.canonical_event_integrity_hash,
        )
        .await
    }
}

impl PostgresDeletionRepository {
    pub async fn connect(database_url: &str) -> Result<Self, PrivacyError> {
        let options =
            PgConnectOptions::from_str(database_url).map_err(|_| PrivacyError::InvalidInput)?;
        let host = options.get_host();
        let local = matches!(host, "localhost" | "127.0.0.1" | "::1") || host.starts_with('/');
        if !local && !matches!(options.get_ssl_mode(), PgSslMode::VerifyFull) {
            return Err(PrivacyError::InvalidInput);
        }
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect_with(options)
            .await
            .map_err(|_| PrivacyError::Database)?;
        Ok(Self::new(pool))
    }

    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn migrate(&self) -> Result<(), PrivacyError> {
        MIGRATOR
            .run(&self.pool)
            .await
            .map_err(|_| PrivacyError::Database)?;
        Ok(())
    }

    pub async fn check_readiness(&self) -> Result<(), PrivacyError> {
        let ready: bool = sqlx::query_scalar(
            "SELECT to_regclass('public.privacy_deletion_jobs') IS NOT NULL \
                    AND to_regclass('public.privacy_deletion_job_targets') IS NOT NULL \
                    AND to_regclass('public.privacy_subject_deletion_fences') IS NOT NULL \
                    AND to_regprocedure('public.enforce_privacy_deletion_job_evidence()') \
                        IS NOT NULL",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|_| PrivacyError::Database)?;
        if ready {
            Ok(())
        } else {
            Err(PrivacyError::InvalidPersistedState)
        }
    }

    pub async fn request(
        &self,
        job_id: &str,
        subject_id: &str,
        requested_by: &str,
        retention_policy: &str,
        evidence: &DeletionRequestEvidence,
    ) -> Result<DeletionJob, PrivacyError> {
        validate_id(job_id)?;
        validate_id(subject_id)?;
        validate_id(requested_by)?;
        if retention_policy.trim().is_empty()
            || retention_policy.len() > 128
            || evidence.event_type()
                != "platform.security_privacy_copyright.data_deletion_requested"
        {
            return Err(PrivacyError::InvalidInput);
        }
        // A side-table job must never precede its canonical request event.
        // Production callers commit first and then call `record_confirmed`.
        Err(PrivacyError::LegacyTwoPhaseDisabled)
    }

    pub async fn confirm_evidence(
        &self,
        job_id: &str,
        evidence: &DeletionRequestEvidence,
        canonical_event_sequence: u64,
        canonical_event_integrity_hash: &str,
    ) -> Result<DeletionJob, PrivacyError> {
        validate_id(job_id)?;
        if canonical_event_sequence == 0 || !valid_integrity_hash(canonical_event_integrity_hash) {
            return Err(PrivacyError::InvalidInput);
        }
        let _ = evidence;
        Err(PrivacyError::LegacyTwoPhaseDisabled)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn record_confirmed(
        &self,
        job_id: &str,
        subject_id: &str,
        requested_by: &str,
        retention_policy: &str,
        evidence: &DeletionRequestEvidence,
        canonical_event_sequence: u64,
        canonical_event_integrity_hash: &str,
    ) -> Result<DeletionJob, PrivacyError> {
        validate_id(job_id)?;
        validate_id(subject_id)?;
        validate_id(requested_by)?;
        if retention_policy.trim().is_empty()
            || retention_policy.len() > 128
            || canonical_event_sequence == 0
            || !valid_integrity_hash(canonical_event_integrity_hash)
        {
            return Err(PrivacyError::InvalidInput);
        }
        let canonical_event_sequence =
            i64::try_from(canonical_event_sequence).map_err(|_| PrivacyError::InvalidInput)?;
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|_| PrivacyError::Database)?;
        sqlx::query(
            "INSERT INTO privacy_deletion_jobs \
             (job_id, campaign_id, subject_id, requested_by, retention_policy, status, evidence_status, \
              command_id, correlation_id, causation_id, canonical_event_type, \
              canonical_event_sequence, canonical_event_integrity_hash) \
             VALUES ($1, $2, $3, $4, $5, 'requested', 'confirmed', $6, $7, $8, $9, $10, $11) \
             ON CONFLICT (job_id) DO NOTHING",
        )
        .bind(job_id)
        .bind(evidence.campaign_id().as_str())
        .bind(subject_id)
        .bind(requested_by)
        .bind(retention_policy.trim())
        .bind(evidence.command_id().as_str())
        .bind(evidence.correlation_id().as_str())
        .bind(evidence.causation_id().as_str())
        .bind(evidence.event_type())
        .bind(canonical_event_sequence)
        .bind(canonical_event_integrity_hash)
        .execute(&mut *transaction)
        .await
        .map_err(|_| PrivacyError::Database)?;
        let persisted = sqlx::query(
            "SELECT campaign_id, subject_id, requested_by, retention_policy, command_id, correlation_id, \
                    causation_id, canonical_event_type, evidence_status, \
                    canonical_event_sequence, canonical_event_integrity_hash \
               FROM privacy_deletion_jobs WHERE job_id = $1 FOR UPDATE",
        )
        .bind(job_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(|_| PrivacyError::Database)?;
        if persisted.get::<String, _>("campaign_id") != evidence.campaign_id().as_str()
            || persisted.get::<String, _>("subject_id") != subject_id
            || persisted.get::<String, _>("requested_by") != requested_by
            || persisted.get::<String, _>("retention_policy") != retention_policy.trim()
            || persisted.get::<String, _>("command_id") != evidence.command_id().as_str()
            || persisted.get::<String, _>("correlation_id") != evidence.correlation_id().as_str()
            || persisted.get::<String, _>("causation_id") != evidence.causation_id().as_str()
            || persisted.get::<String, _>("canonical_event_type") != evidence.event_type()
            || persisted.get::<String, _>("evidence_status") != "confirmed"
            || persisted.get::<Option<i64>, _>("canonical_event_sequence")
                != Some(canonical_event_sequence)
            || persisted
                .get::<Option<String>, _>("canonical_event_integrity_hash")
                .as_deref()
                != Some(canonical_event_integrity_hash)
        {
            return Err(PrivacyError::InvalidPersistedState);
        }
        for target in REQUIRED_DELETION_TARGETS {
            sqlx::query(
                "INSERT INTO privacy_deletion_job_targets (job_id, target, status) \
                 VALUES ($1, $2, 'pending') ON CONFLICT (job_id, target) DO NOTHING",
            )
            .bind(job_id)
            .bind(target.as_str())
            .execute(&mut *transaction)
            .await
            .map_err(|_| PrivacyError::Database)?;
        }
        transaction
            .commit()
            .await
            .map_err(|_| PrivacyError::Database)?;
        self.load(job_id).await
    }

    pub async fn load(&self, job_id: &str) -> Result<DeletionJob, PrivacyError> {
        self.load_scoped(job_id, None).await
    }

    pub async fn load_for_campaign(
        &self,
        job_id: &str,
        campaign_id: &EntityId,
    ) -> Result<DeletionJob, PrivacyError> {
        self.load_scoped(job_id, Some(campaign_id)).await
    }

    async fn load_scoped(
        &self,
        job_id: &str,
        campaign_id: Option<&EntityId>,
    ) -> Result<DeletionJob, PrivacyError> {
        validate_id(job_id)?;
        let row = sqlx::query(
            "SELECT job_id, campaign_id, subject_id, requested_by, retention_policy, status, failure_code, \
             evidence_status, canonical_event_sequence, canonical_event_integrity_hash \
             FROM privacy_deletion_jobs \
             WHERE job_id = $1 AND ($2::text IS NULL OR campaign_id = $2)",
        )
        .bind(job_id)
        .bind(campaign_id.map(EntityId::as_str))
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| PrivacyError::Database)?
        .ok_or(PrivacyError::JobNotFound)?;
        let target_rows = sqlx::query(
            "SELECT target, status, error_code FROM privacy_deletion_job_targets \
             WHERE job_id = $1 ORDER BY target",
        )
        .bind(job_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|_| PrivacyError::Database)?;
        let targets = target_rows
            .iter()
            .map(|row| {
                Ok(DeletionTargetRecord {
                    target: DeletionTarget::parse(row.try_get("target").map_err(db_error)?)?,
                    status: DeletionTargetStatus::parse(row.try_get("status").map_err(db_error)?)?,
                    error_code: row.try_get("error_code").map_err(db_error)?,
                })
            })
            .collect::<Result<Vec<_>, PrivacyError>>()?;

        Ok(DeletionJob {
            job_id: row.try_get("job_id").map_err(db_error)?,
            campaign_id: row.try_get("campaign_id").map_err(db_error)?,
            subject_id: row.try_get("subject_id").map_err(db_error)?,
            requested_by: row.try_get("requested_by").map_err(db_error)?,
            retention_policy: row.try_get("retention_policy").map_err(db_error)?,
            status: DeletionJobStatus::parse(row.try_get("status").map_err(db_error)?)?,
            failure_code: row.try_get("failure_code").map_err(db_error)?,
            evidence_status: DeletionEvidenceStatus::parse(
                row.try_get("evidence_status").map_err(db_error)?,
            )?,
            canonical_event_sequence: row
                .try_get::<Option<i64>, _>("canonical_event_sequence")
                .map_err(db_error)?
                .map(|value| u64::try_from(value).map_err(|_| PrivacyError::InvalidPersistedState))
                .transpose()?,
            canonical_event_integrity_hash: row
                .try_get("canonical_event_integrity_hash")
                .map_err(db_error)?,
            targets,
        })
    }

    async fn set_status(
        &self,
        job_id: &str,
        status: DeletionJobStatus,
        failure_code: Option<&str>,
    ) -> Result<(), PrivacyError> {
        let affected = sqlx::query(
            "UPDATE privacy_deletion_jobs SET status = $2, failure_code = $3, \
             updated_at = now() WHERE job_id = $1",
        )
        .bind(job_id)
        .bind(status.as_str())
        .bind(failure_code)
        .execute(&self.pool)
        .await
        .map_err(|_| PrivacyError::Database)?
        .rows_affected();
        if affected == 1 {
            Ok(())
        } else {
            Err(PrivacyError::JobNotFound)
        }
    }

    async fn set_target_status(
        &self,
        job_id: &str,
        target: DeletionTarget,
        status: DeletionTargetStatus,
        error_code: Option<&str>,
    ) -> Result<(), PrivacyError> {
        let affected = sqlx::query(
            "UPDATE privacy_deletion_job_targets SET status = $3, error_code = $4, \
             deleted_at = CASE WHEN $3 IN ('deleted', 'verified') THEN now() ELSE deleted_at END, \
             verified_at = CASE WHEN $3 = 'verified' THEN now() ELSE verified_at END \
             WHERE job_id = $1 AND target = $2",
        )
        .bind(job_id)
        .bind(target.as_str())
        .bind(status.as_str())
        .bind(error_code)
        .execute(&self.pool)
        .await
        .map_err(|_| PrivacyError::Database)?
        .rows_affected();
        if affected == 1 {
            Ok(())
        } else {
            Err(PrivacyError::InvalidPersistedState)
        }
    }

    async fn claim_execution(&self, job_id: &str, subject_id: &str) -> Result<bool, PrivacyError> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|_| PrivacyError::Database)?;
        sqlx::query(
            "SELECT pg_advisory_xact_lock(hashtextextended('privacy_subject_delete:' || $1, 0))",
        )
        .bind(subject_id)
        .execute(&mut *transaction)
        .await
        .map_err(|_| PrivacyError::Database)?;
        let row = sqlx::query(
            "SELECT subject_id, status, evidence_status FROM privacy_deletion_jobs \
             WHERE job_id = $1 FOR UPDATE",
        )
        .bind(job_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|_| PrivacyError::Database)?
        .ok_or(PrivacyError::JobNotFound)?;
        let persisted_subject: String = row.get("subject_id");
        let status: String = row.get("status");
        let evidence_status: String = row.get("evidence_status");
        if persisted_subject != subject_id || evidence_status != "confirmed" {
            return Err(PrivacyError::EvidenceUnconfirmed);
        }
        if status == "running" || status == "verifying" {
            return Err(PrivacyError::JobAlreadyRunning);
        }
        if status == "completed" {
            return Ok(true);
        }
        let held: bool = sqlx::query_scalar(
            "SELECT COALESCE((SELECT active FROM privacy_legal_holds \
                              WHERE subject_id = $1), false)",
        )
        .bind(subject_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(|_| PrivacyError::Database)?;
        if held {
            sqlx::query(
                "UPDATE privacy_deletion_jobs SET status = 'blocked_legal_hold', \
                 failure_code = NULL, updated_at = now() WHERE job_id = $1",
            )
            .bind(job_id)
            .execute(&mut *transaction)
            .await
            .map_err(|_| PrivacyError::Database)?;
            transaction
                .commit()
                .await
                .map_err(|_| PrivacyError::Database)?;
            return Ok(false);
        }
        let existing_fence = sqlx::query(
            "SELECT job_id, status FROM privacy_subject_deletion_fences \
             WHERE subject_id = $1 FOR UPDATE",
        )
        .bind(subject_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|_| PrivacyError::Database)?;
        if let Some(fence) = existing_fence {
            let fenced_job: String = fence.get("job_id");
            let fenced_status: String = fence.get("status");
            if fenced_job != job_id || fenced_status == "completed" {
                return Err(PrivacyError::DeletionInProgress);
            }
            sqlx::query(
                "UPDATE privacy_subject_deletion_fences SET status = 'running', \
                 updated_at = now() WHERE subject_id = $1",
            )
            .bind(subject_id)
            .execute(&mut *transaction)
            .await
            .map_err(|_| PrivacyError::Database)?;
        } else {
            sqlx::query(
                "INSERT INTO privacy_subject_deletion_fences \
                 (subject_id, job_id, status) VALUES ($1, $2, 'running')",
            )
            .bind(subject_id)
            .bind(job_id)
            .execute(&mut *transaction)
            .await
            .map_err(|_| PrivacyError::Database)?;
        }
        sqlx::query(
            "UPDATE privacy_deletion_jobs SET status = 'running', failure_code = NULL, \
             updated_at = now() WHERE job_id = $1",
        )
        .bind(job_id)
        .execute(&mut *transaction)
        .await
        .map_err(|_| PrivacyError::Database)?;
        transaction
            .commit()
            .await
            .map_err(|_| PrivacyError::Database)?;
        Ok(true)
    }

    async fn finish_execution(
        &self,
        job_id: &str,
        subject_id: &str,
        status: DeletionJobStatus,
        failure_code: Option<&str>,
    ) -> Result<(), PrivacyError> {
        let fence_status = match status {
            DeletionJobStatus::Completed => "completed",
            DeletionJobStatus::Failed => "failed",
            _ => return Err(PrivacyError::InvalidInput),
        };
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|_| PrivacyError::Database)?;
        let affected = sqlx::query(
            "UPDATE privacy_subject_deletion_fences SET status = $3, updated_at = now() \
             WHERE subject_id = $1 AND job_id = $2 AND status = 'running'",
        )
        .bind(subject_id)
        .bind(job_id)
        .bind(fence_status)
        .execute(&mut *transaction)
        .await
        .map_err(|_| PrivacyError::Database)?
        .rows_affected();
        if affected != 1 {
            return Err(PrivacyError::InvalidPersistedState);
        }
        sqlx::query(
            "UPDATE privacy_deletion_jobs SET status = $2, failure_code = $3, \
             updated_at = now() WHERE job_id = $1",
        )
        .bind(job_id)
        .bind(status.as_str())
        .bind(failure_code)
        .execute(&mut *transaction)
        .await
        .map_err(|_| PrivacyError::Database)?;
        transaction
            .commit()
            .await
            .map_err(|_| PrivacyError::Database)
    }
}

fn validate_id(value: &str) -> Result<(), PrivacyError> {
    EntityId::new(value)
        .map(|_| ())
        .map_err(|_| PrivacyError::InvalidInput)
}

fn valid_integrity_hash(value: &str) -> bool {
    const PREFIX: &str = "hmac-sha256:";
    value.len() == PREFIX.len() + 64
        && value.starts_with(PREFIX)
        && value[PREFIX.len()..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn db_error(_: sqlx::Error) -> PrivacyError {
    PrivacyError::Database
}

#[async_trait]
pub trait LegalHoldResolver: Send + Sync {
    async fn has_active_hold(&self, subject_id: &str) -> Result<bool, PrivacyError>;
}

#[derive(Clone)]
pub struct PostgresLegalHoldResolver {
    pool: PgPool,
}

impl PostgresLegalHoldResolver {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn set_hold(
        &self,
        subject_id: &str,
        hold_reference: &str,
        active: bool,
    ) -> Result<(), PrivacyError> {
        validate_id(subject_id)?;
        validate_id(hold_reference)?;
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|_| PrivacyError::Database)?;
        sqlx::query(
            "SELECT pg_advisory_xact_lock(hashtextextended('privacy_subject_delete:' || $1, 0))",
        )
        .bind(subject_id)
        .execute(&mut *transaction)
        .await
        .map_err(|_| PrivacyError::Database)?;
        if active {
            let deletion_running: bool = sqlx::query_scalar(
                "SELECT EXISTS (SELECT 1 FROM privacy_subject_deletion_fences \
                                WHERE subject_id = $1 AND status = 'running')",
            )
            .bind(subject_id)
            .fetch_one(&mut *transaction)
            .await
            .map_err(|_| PrivacyError::Database)?;
            if deletion_running {
                return Err(PrivacyError::DeletionInProgress);
            }
        }
        sqlx::query(
            "INSERT INTO privacy_legal_holds (subject_id, hold_reference, active) \
             VALUES ($1, $2, $3) ON CONFLICT (subject_id) DO UPDATE SET \
             hold_reference = EXCLUDED.hold_reference, active = EXCLUDED.active, updated_at = now()",
        )
        .bind(subject_id)
        .bind(hold_reference)
        .bind(active)
        .execute(&mut *transaction)
        .await
        .map_err(|_| PrivacyError::Database)?;
        transaction
            .commit()
            .await
            .map_err(|_| PrivacyError::Database)
    }
}

#[async_trait]
impl LegalHoldResolver for PostgresLegalHoldResolver {
    async fn has_active_hold(&self, subject_id: &str) -> Result<bool, PrivacyError> {
        validate_id(subject_id)?;
        sqlx::query_scalar::<_, bool>(
            "SELECT active FROM privacy_legal_holds WHERE subject_id = $1",
        )
        .bind(subject_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| PrivacyError::Database)
        .map(|active| active.unwrap_or(false))
    }
}

#[async_trait]
pub trait DeletionSurface: Send + Sync {
    fn target(&self) -> DeletionTarget;
    async fn delete_subject(&self, subject_id: &str) -> Result<(), PrivacyError>;
    async fn verify_absent(&self, subject_id: &str) -> Result<bool, PrivacyError>;
}

#[derive(Clone)]
pub struct S3ObjectDeletionSurface {
    bucket: Box<Bucket>,
}

impl std::fmt::Debug for S3ObjectDeletionSurface {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("S3ObjectDeletionSurface")
            .field("bucket", &self.bucket.name)
            .field("endpoint", &"[REDACTED]")
            .finish()
    }
}

impl S3ObjectDeletionSurface {
    pub async fn connect(
        endpoint: &str,
        region: &str,
        bucket_name: &str,
        access_key: &str,
        secret_key: &str,
    ) -> Result<Self, PrivacyError> {
        validate_secure_service_url(endpoint, "http", "https")?;
        validate_id(region)?;
        validate_id(bucket_name)?;
        if access_key.trim().is_empty() || secret_key.len() < 8 {
            return Err(PrivacyError::InvalidInput);
        }
        let credentials = Credentials::new(Some(access_key), Some(secret_key), None, None, None)
            .map_err(|_| PrivacyError::Storage)?;
        let region = Region::Custom {
            region: region.to_owned(),
            endpoint: endpoint.trim_end_matches('/').to_owned(),
        };
        let bucket = Bucket::new(bucket_name, region, credentials)
            .map_err(|_| PrivacyError::Storage)?
            .with_path_style();
        if !bucket.exists().await.map_err(|_| PrivacyError::Storage)? {
            return Err(PrivacyError::Storage);
        }
        let surface = Self { bucket };
        surface.require_unversioned_bucket().await?;
        Ok(surface)
    }

    pub fn subject_prefix(subject_id: &str) -> Result<String, PrivacyError> {
        validate_id(subject_id)?;
        Ok(format!("subjects/{}/", sha256_hex(subject_id.as_bytes())))
    }

    pub async fn put_protected_object(
        &self,
        subject_id: &str,
        object_id: &str,
        protected_payload: &[u8],
    ) -> Result<(), PrivacyError> {
        validate_id(object_id)?;
        if protected_payload.is_empty() {
            return Err(PrivacyError::InvalidInput);
        }
        let key = format!("{}{}", Self::subject_prefix(subject_id)?, object_id);
        self.bucket
            .put_object(key, protected_payload)
            .await
            .map(|_| ())
            .map_err(|_| PrivacyError::Storage)
    }

    async fn subject_keys(&self, subject_id: &str) -> Result<Vec<String>, PrivacyError> {
        let prefix = Self::subject_prefix(subject_id)?;
        self.bucket
            .list(prefix, None)
            .await
            .map(|pages| {
                pages
                    .into_iter()
                    .flat_map(|page| page.contents)
                    .map(|object| object.key)
                    .collect()
            })
            .map_err(|_| PrivacyError::Storage)
    }

    /// The current adapter can prove deletion only for an unversioned bucket.
    /// A versioned bucket would make a normal DELETE create a delete marker
    /// while retaining recoverable historical bytes, so startup and every
    /// deletion/verification boundary fail closed if S3 reports a version id.
    async fn require_unversioned_bucket(&self) -> Result<(), PrivacyError> {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| PrivacyError::Storage)?
            .as_nanos();
        let key = format!(
            ".trpg-erasure-versioning-probe/{}-{nonce}",
            std::process::id()
        );
        let response = self
            .bucket
            .put_object(&key, b"versioning-probe")
            .await
            .map_err(|_| PrivacyError::Storage)?;
        let version_id = response
            .headers()
            .get("x-amz-version-id")
            .filter(|value| !value.trim().is_empty() && value.as_str() != "null")
            .cloned();
        if let Some(version_id) = version_id {
            let cleanup = self
                .bucket
                .delete_objects(vec![ObjectIdentifier::with_version(&key, version_id)])
                .await
                .map_err(|_| PrivacyError::Storage)?;
            if !cleanup.errors.is_empty() {
                return Err(PrivacyError::Storage);
            }
            return Err(PrivacyError::InvalidPersistedState);
        }
        self.bucket
            .delete_object(&key)
            .await
            .map_err(|_| PrivacyError::Storage)?;
        Ok(())
    }
}

#[async_trait]
impl DeletionSurface for S3ObjectDeletionSurface {
    fn target(&self) -> DeletionTarget {
        DeletionTarget::ObjectStorage
    }

    async fn delete_subject(&self, subject_id: &str) -> Result<(), PrivacyError> {
        self.require_unversioned_bucket().await?;
        for key in self.subject_keys(subject_id).await? {
            self.bucket
                .delete_object(key)
                .await
                .map_err(|_| PrivacyError::Storage)?;
        }
        Ok(())
    }

    async fn verify_absent(&self, subject_id: &str) -> Result<bool, PrivacyError> {
        self.require_unversioned_bucket().await?;
        Ok(self.subject_keys(subject_id).await?.is_empty())
    }
}

const REDIS_DELETE_SUBJECT_KEYS: &str = r#"
local members = redis.call('SMEMBERS', KEYS[1])
local removed = 0
for _, key in ipairs(members) do
  removed = removed + redis.call('DEL', key)
end
redis.call('DEL', KEYS[1])
return removed
"#;

const REDIS_COUNT_SUBJECT_KEYS: &str = r#"
return redis.call('SCARD', KEYS[1]) + redis.call('EXISTS', KEYS[1])
"#;

#[derive(Clone)]
pub struct RedisCacheDeletionSurface {
    connection: ConnectionManager,
    namespace: String,
}

impl RedisCacheDeletionSurface {
    pub async fn connect(redis_url: &str, namespace: &str) -> Result<Self, PrivacyError> {
        Self::connect_with_tls(redis_url, namespace, None, None, None).await
    }

    pub async fn connect_with_tls(
        redis_url: &str,
        namespace: &str,
        root_certificate: Option<&[u8]>,
        client_certificate: Option<&[u8]>,
        client_private_key: Option<&[u8]>,
    ) -> Result<Self, PrivacyError> {
        validate_secure_service_url(redis_url, "redis", "rediss")?;
        validate_redis_namespace(namespace)?;
        let client = build_redis_client(
            redis_url,
            root_certificate,
            client_certificate,
            client_private_key,
        )?;
        let mut connection = ConnectionManager::new(client)
            .await
            .map_err(|_| PrivacyError::Cache)?;
        let pong: String = redis::cmd("PING")
            .query_async(&mut connection)
            .await
            .map_err(|_| PrivacyError::Cache)?;
        if pong != "PONG" {
            return Err(PrivacyError::Cache);
        }
        Ok(Self {
            connection,
            namespace: namespace.to_owned(),
        })
    }

    pub async fn put_for_test(
        &self,
        subject_id: &str,
        record_key: &str,
        protected_payload: &[u8],
    ) -> Result<(), PrivacyError> {
        validate_id(subject_id)?;
        validate_id(record_key)?;
        if protected_payload.is_empty() {
            return Err(PrivacyError::InvalidInput);
        }
        let mut connection = self.connection.clone();
        let entry_key = self.key(record_key);
        let subject_index_key = self.subject_index_key(subject_id);
        redis::cmd("SET")
            .arg(&entry_key)
            .arg(protected_payload)
            .query_async::<()>(&mut connection)
            .await
            .map_err(|_| PrivacyError::Cache)?;
        redis::cmd("SADD")
            .arg(subject_index_key)
            .arg(entry_key)
            .query_async::<i64>(&mut connection)
            .await
            .map(|_| ())
            .map_err(|_| PrivacyError::Cache)
    }

    fn key(&self, record_key: &str) -> String {
        format!(
            "{}:entry:{}",
            self.namespace,
            sha256_hex(record_key.as_bytes())
        )
    }

    fn subject_index_key(&self, subject_id: &str) -> String {
        format!(
            "{}:subject:{}",
            self.namespace,
            sha256_hex(subject_id.as_bytes())
        )
    }
}

fn validate_redis_namespace(value: &str) -> Result<(), PrivacyError> {
    if value.trim().is_empty()
        || value.len() > 256
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b':' | b'_' | b'-' | b'.'))
    {
        Err(PrivacyError::InvalidInput)
    } else {
        Ok(())
    }
}

#[async_trait]
impl DeletionSurface for RedisCacheDeletionSurface {
    fn target(&self) -> DeletionTarget {
        DeletionTarget::Cache
    }

    async fn delete_subject(&self, subject_id: &str) -> Result<(), PrivacyError> {
        validate_id(subject_id)?;
        let mut connection = self.connection.clone();
        redis::Script::new(REDIS_DELETE_SUBJECT_KEYS)
            .key(self.subject_index_key(subject_id))
            .invoke_async::<i64>(&mut connection)
            .await
            .map(|_| ())
            .map_err(|_| PrivacyError::Cache)
    }

    async fn verify_absent(&self, subject_id: &str) -> Result<bool, PrivacyError> {
        validate_id(subject_id)?;
        let mut connection = self.connection.clone();
        let count = redis::Script::new(REDIS_COUNT_SUBJECT_KEYS)
            .key(self.subject_index_key(subject_id))
            .invoke_async::<i64>(&mut connection)
            .await
            .map_err(|_| PrivacyError::Cache)?;
        Ok(count == 0)
    }
}

fn build_redis_client(
    redis_url: &str,
    root_certificate: Option<&[u8]>,
    client_certificate: Option<&[u8]>,
    client_private_key: Option<&[u8]>,
) -> Result<redis::Client, PrivacyError> {
    let material = [
        root_certificate.is_some(),
        client_certificate.is_some(),
        client_private_key.is_some(),
    ];
    if material.iter().any(|present| *present) && !material.iter().all(|present| *present) {
        return Err(PrivacyError::InvalidInput);
    }
    if Url::parse(redis_url)
        .map_err(|_| PrivacyError::InvalidInput)?
        .scheme()
        == "rediss"
    {
        let (root_certificate, client_certificate, client_private_key) = (
            root_certificate.ok_or(PrivacyError::InvalidInput)?,
            client_certificate.ok_or(PrivacyError::InvalidInput)?,
            client_private_key.ok_or(PrivacyError::InvalidInput)?,
        );
        redis::Client::build_with_tls(
            redis_url,
            redis::TlsCertificates {
                client_tls: Some(redis::ClientTlsConfig {
                    client_cert: client_certificate.to_vec(),
                    client_key: client_private_key.to_vec(),
                }),
                root_cert: Some(root_certificate.to_vec()),
            },
        )
        .map_err(|_| PrivacyError::InvalidInput)
    } else {
        redis::Client::open(redis_url).map_err(|_| PrivacyError::InvalidInput)
    }
}

fn sha256_hex(value: &[u8]) -> String {
    let digest = Sha256::digest(value);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[derive(Clone)]
pub struct NatsQueueDeletionSurface {
    jetstream: async_nats::jetstream::Context,
    stream_name: String,
    subject_prefix: String,
    canonical_pool: Option<PgPool>,
}

impl NatsQueueDeletionSurface {
    pub async fn connect(
        nats_url: &str,
        stream_name: &str,
        subject_prefix: &str,
    ) -> Result<Self, PrivacyError> {
        let surface = Self::connect_context(
            nats_url,
            stream_name,
            subject_prefix,
            None,
            None,
            None,
            None,
        )
        .await?;
        surface
            .jetstream
            .get_stream(&surface.stream_name)
            .await
            .map_err(|_| PrivacyError::Queue)?;
        Ok(surface)
    }

    pub async fn connect_or_create_for_test(
        nats_url: &str,
        stream_name: &str,
        subject_prefix: &str,
    ) -> Result<Self, PrivacyError> {
        let surface = Self::connect_context(
            nats_url,
            stream_name,
            subject_prefix,
            None,
            None,
            None,
            None,
        )
        .await?;
        surface
            .jetstream
            .get_or_create_stream(async_nats::jetstream::stream::Config {
                name: surface.stream_name.clone(),
                subjects: vec![format!("{}.*", surface.subject_prefix)],
                ..Default::default()
            })
            .await
            .map_err(|_| PrivacyError::Queue)?;
        Ok(surface)
    }

    async fn connect_context(
        nats_url: &str,
        stream_name: &str,
        subject_prefix: &str,
        ca_certificate_path: Option<&Path>,
        client_certificate_path: Option<&Path>,
        client_private_key_path: Option<&Path>,
        credentials_path: Option<&Path>,
    ) -> Result<Self, PrivacyError> {
        validate_secure_service_url(nats_url, "nats", "tls")?;
        validate_id(stream_name)?;
        if subject_prefix.is_empty()
            || subject_prefix.len() > 128
            || !subject_prefix.split('.').all(|token| {
                !token.is_empty()
                    && token
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
            })
        {
            return Err(PrivacyError::InvalidInput);
        }
        if client_certificate_path.is_some() != client_private_key_path.is_some() {
            return Err(PrivacyError::InvalidInput);
        }
        let mut options = async_nats::ConnectOptions::new().require_tls(
            Url::parse(nats_url)
                .map_err(|_| PrivacyError::InvalidInput)?
                .scheme()
                == "tls",
        );
        if let Some(path) = ca_certificate_path {
            options = options.add_root_certificates(path.to_path_buf());
        }
        if let (Some(certificate), Some(private_key)) =
            (client_certificate_path, client_private_key_path)
        {
            options = options
                .add_client_certificate(certificate.to_path_buf(), private_key.to_path_buf());
        }
        if let Some(path) = credentials_path {
            options = options
                .credentials_file(path)
                .await
                .map_err(|_| PrivacyError::InvalidInput)?;
        }
        let client = options
            .connect(nats_url)
            .await
            .map_err(|_| PrivacyError::Queue)?;
        Ok(Self {
            jetstream: async_nats::jetstream::new(client),
            stream_name: stream_name.to_owned(),
            subject_prefix: subject_prefix.to_owned(),
            canonical_pool: None,
        })
    }

    /// Production queue deletion dead-letters unpublished subject rows and
    /// removes every already-published message whose server-authored subject
    /// digest matches the erased subject. Canonical PostgreSQL history remains
    /// append-only, while the delivery surface is proved byte-absent.
    pub async fn connect_crypto_erasure(
        nats_url: &str,
        stream_name: &str,
        canonical_pool: PgPool,
    ) -> Result<Self, PrivacyError> {
        Self::connect_crypto_erasure_with_credentials(
            nats_url,
            stream_name,
            canonical_pool,
            None,
            None,
            None,
            None,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn connect_crypto_erasure_with_credentials(
        nats_url: &str,
        stream_name: &str,
        canonical_pool: PgPool,
        ca_certificate_path: Option<&Path>,
        client_certificate_path: Option<&Path>,
        client_private_key_path: Option<&Path>,
        credentials_path: Option<&Path>,
    ) -> Result<Self, PrivacyError> {
        let mut surface = Self::connect_context(
            nats_url,
            stream_name,
            "trpg.events",
            ca_certificate_path,
            client_certificate_path,
            client_private_key_path,
            credentials_path,
        )
        .await?;
        surface
            .jetstream
            .get_stream(&surface.stream_name)
            .await
            .map_err(|_| PrivacyError::Queue)?;
        surface.canonical_pool = Some(canonical_pool);
        Ok(surface)
    }

    pub async fn put_for_test(
        &self,
        subject_id: &str,
        protected_payload: &[u8],
    ) -> Result<(), PrivacyError> {
        validate_id(subject_id)?;
        if protected_payload.is_empty() {
            return Err(PrivacyError::InvalidInput);
        }
        self.jetstream
            .publish(self.subject(subject_id), protected_payload.to_vec().into())
            .await
            .map_err(|_| PrivacyError::Queue)?
            .await
            .map_err(|_| PrivacyError::Queue)?;
        Ok(())
    }

    pub async fn cleanup_for_test(&self) -> Result<(), PrivacyError> {
        self.jetstream
            .delete_stream(&self.stream_name)
            .await
            .map_err(|_| PrivacyError::Queue)?;
        Ok(())
    }

    fn subject(&self, subject_id: &str) -> String {
        format!("{}.{}", self.subject_prefix, subject_id)
    }

    async fn canonical_subject_message_sequences(
        &self,
        subject_id: &str,
    ) -> Result<Vec<u64>, PrivacyError> {
        let mut stream = self
            .jetstream
            .get_stream(&self.stream_name)
            .await
            .map_err(|_| PrivacyError::Queue)?;
        let info = stream.info().await.map_err(|_| PrivacyError::Queue)?;
        if info.state.messages == 0 {
            return Ok(Vec::new());
        }
        let expected = format!("sha256:{}", sha256_hex(subject_id.as_bytes()));
        let mut matches = Vec::new();
        for sequence in info.state.first_sequence..=info.state.last_sequence {
            match stream.get_raw_message(sequence).await {
                Ok(message) => {
                    let header_digest = message
                        .headers
                        .get("Trpg-Data-Subject-Digest")
                        .map(|value| value.as_str());
                    let observed =
                        retained_message_subject_digest(header_digest, &message.payload)?;
                    if observed == expected {
                        matches.push(sequence);
                    }
                }
                Err(error)
                    if error.kind()
                        == async_nats::jetstream::stream::RawMessageErrorKind::NoMessageFound => {}
                Err(_) => return Err(PrivacyError::Queue),
            }
        }
        Ok(matches)
    }
}

fn retained_message_subject_digest(
    header_digest: Option<&str>,
    payload: &[u8],
) -> Result<String, PrivacyError> {
    let payload_subject = serde_json::from_slice::<Value>(payload)
        .ok()
        .and_then(|payload| {
            payload
                .get("data_subject_id")
                .and_then(Value::as_str)
                .map(str::to_owned)
        });
    let payload_digest = payload_subject
        .as_deref()
        .map(|subject| format!("sha256:{}", sha256_hex(subject.as_bytes())));
    match (header_digest, payload_digest.as_deref()) {
        (Some(header), Some(payload)) if header == payload => Ok(header.to_owned()),
        (Some(header), None)
            if header.len() == 71
                && header.starts_with("sha256:")
                && header[7..].bytes().all(|byte| byte.is_ascii_hexdigit()) =>
        {
            Ok(header.to_owned())
        }
        (None, Some(payload)) => Ok(payload.to_owned()),
        // An unclassified or inconsistently classified retained message makes
        // absence unprovable. Never convert it into a successful result.
        _ => Err(PrivacyError::InvalidPersistedState),
    }
}

#[async_trait]
impl DeletionSurface for NatsQueueDeletionSurface {
    fn target(&self) -> DeletionTarget {
        DeletionTarget::Queue
    }

    async fn delete_subject(&self, subject_id: &str) -> Result<(), PrivacyError> {
        validate_id(subject_id)?;
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
            for sequence in self.canonical_subject_message_sequences(subject_id).await? {
                if !stream
                    .delete_message(sequence)
                    .await
                    .map_err(|_| PrivacyError::Queue)?
                {
                    return Err(PrivacyError::Queue);
                }
            }
            return Ok(());
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
            .map(|_| ())
            .map_err(|_| PrivacyError::Queue)
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
                    .canonical_subject_message_sequences(subject_id)
                    .await?
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
    let local = matches!(host, "localhost" | "127.0.0.1" | "::1");
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

    async fn delete_database_subject(&self, subject_id: &str) -> Result<(), PrivacyError> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|_| PrivacyError::Database)?;
        sqlx::query(
            "SELECT pg_advisory_xact_lock(hashtextextended('privacy_subject_delete:' || $1, 0))",
        )
        .bind(subject_id)
        .execute(&mut *transaction)
        .await
        .map_err(|_| PrivacyError::Database)?;
        let owns_immutable_authority: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM authority_contracts WHERE authority_owner = $1)",
        )
        .bind(subject_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(|_| PrivacyError::Database)?;
        if owns_immutable_authority {
            return Err(PrivacyError::ProtectedCanonicalIdentity);
        }

        sqlx::query("DELETE FROM sessions WHERE user_id = $1")
            .bind(subject_id)
            .execute(&mut *transaction)
            .await
            .map_err(|_| PrivacyError::Database)?;
        sqlx::query(
            "UPDATE campaign_group_memberships SET revoked_at = COALESCE(revoked_at, now()) \
             WHERE user_id = $1",
        )
        .bind(subject_id)
        .execute(&mut *transaction)
        .await
        .map_err(|_| PrivacyError::Database)?;
        sqlx::query(
            "UPDATE campaign_memberships SET revoked_at = COALESCE(revoked_at, now()) \
             WHERE user_id = $1",
        )
        .bind(subject_id)
        .execute(&mut *transaction)
        .await
        .map_err(|_| PrivacyError::Database)?;

        let digest = sha256_hex(subject_id.as_bytes());
        let expected_erasure_digest = format!("sha256:{digest}");
        let persisted_erasure_digest = sqlx::query_scalar::<_, String>(
            "SELECT erasure_digest FROM privacy_erased_subjects WHERE subject_id = $1",
        )
        .bind(subject_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|_| PrivacyError::Database)?;
        if persisted_erasure_digest
            .as_deref()
            .is_some_and(|persisted| persisted != expected_erasure_digest)
        {
            return Err(PrivacyError::InvalidPersistedState);
        }
        sqlx::query(
            "UPDATE users SET login_normalized = $2, password_hash = $3, \
             disabled_at = COALESCE(disabled_at, now()) WHERE user_id = $1",
        )
        .bind(subject_id)
        .bind(format!("deleted_{digest}"))
        .bind(format!("DELETED_ACCOUNT_NO_LOGIN_{digest}"))
        .execute(&mut *transaction)
        .await
        .map_err(|_| PrivacyError::Database)?;
        sqlx::query(
            "UPDATE cloud_egress_consents SET granted = false, updated_at = now() \
             WHERE subject_id = $1 AND granted = true",
        )
        .bind(subject_id)
        .execute(&mut *transaction)
        .await
        .map_err(|_| PrivacyError::Database)?;
        if persisted_erasure_digest.is_none() {
            sqlx::query(
                "INSERT INTO privacy_erased_subjects (subject_id, erasure_digest) \
                 VALUES ($1, $2)",
            )
            .bind(subject_id)
            .bind(expected_erasure_digest)
            .execute(&mut *transaction)
            .await
            .map_err(|_| PrivacyError::Database)?;
        }
        // Remove rows written by pre-P05 test/preview implementations. They
        // are not used as proof of production deletion.
        sqlx::query(
            "DELETE FROM privacy_deletion_surface_records \
             WHERE surface = 'database' AND subject_id = $1",
        )
        .bind(subject_id)
        .execute(&mut *transaction)
        .await
        .map_err(|_| PrivacyError::Database)?;
        transaction
            .commit()
            .await
            .map_err(|_| PrivacyError::Database)
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

#[async_trait]
impl DeletionSurface for PostgresRecordDeletionSurface {
    fn target(&self) -> DeletionTarget {
        self.target
    }

    async fn delete_subject(&self, subject_id: &str) -> Result<(), PrivacyError> {
        validate_id(subject_id)?;
        match self.target {
            DeletionTarget::Database => self.delete_database_subject(subject_id).await,
            DeletionTarget::RagIndex => {
                let mut transaction = self
                    .pool
                    .begin()
                    .await
                    .map_err(|_| PrivacyError::Database)?;
                sqlx::query(
                    "DELETE FROM rag_snapshot_chunk WHERE visibility_subject = $1 \
                     OR source_event_sequence IN (SELECT sequence FROM event_store \
                                                   WHERE data_subject_id = $1)",
                )
                .bind(subject_id)
                .execute(&mut *transaction)
                .await
                .map_err(|_| PrivacyError::Database)?;
                sqlx::query(
                    "DELETE FROM privacy_deletion_surface_records \
                     WHERE surface = 'rag_index' AND subject_id = $1",
                )
                .bind(subject_id)
                .execute(&mut *transaction)
                .await
                .map_err(|_| PrivacyError::Database)?;
                transaction
                    .commit()
                    .await
                    .map_err(|_| PrivacyError::Database)
            }
            _ => Err(PrivacyError::InvalidInput),
        }
    }

    async fn verify_absent(&self, subject_id: &str) -> Result<bool, PrivacyError> {
        validate_id(subject_id)?;
        match self.target {
            DeletionTarget::Database => self.verify_database_subject(subject_id).await,
            DeletionTarget::RagIndex => {
                let count = sqlx::query_scalar::<_, i64>(
                    "SELECT count(*) FROM rag_snapshot_chunk \
                     WHERE visibility_subject = $1 OR source_event_sequence IN \
                     (SELECT sequence FROM event_store WHERE data_subject_id = $1)",
                )
                .bind(subject_id)
                .fetch_one(&self.pool)
                .await
                .map_err(|_| PrivacyError::Database)?;
                Ok(count == 0)
            }
            _ => Err(PrivacyError::InvalidInput),
        }
    }
}

pub struct FilesystemDeletionSurface {
    root: PathBuf,
    target: DeletionTarget,
}

impl FilesystemDeletionSurface {
    pub fn new(root: impl AsRef<Path>, target: DeletionTarget) -> Result<Self, PrivacyError> {
        let root = root.as_ref();
        if !root.is_absolute()
            || root.parent().is_none()
            || !matches!(
                target,
                DeletionTarget::ObjectStorage | DeletionTarget::Export
            )
        {
            return Err(PrivacyError::InvalidInput);
        }
        Ok(Self {
            root: root.to_path_buf(),
            target,
        })
    }

    pub fn subject_path(&self, subject_id: &str) -> Result<PathBuf, PrivacyError> {
        validate_id(subject_id)?;
        Ok(self.root.join(subject_id))
    }
}

#[async_trait]
impl DeletionSurface for FilesystemDeletionSurface {
    fn target(&self) -> DeletionTarget {
        self.target
    }

    async fn delete_subject(&self, subject_id: &str) -> Result<(), PrivacyError> {
        let path = self.subject_path(subject_id)?;
        match tokio::fs::remove_dir_all(path).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(PrivacyError::Storage),
        }
    }

    async fn verify_absent(&self, subject_id: &str) -> Result<bool, PrivacyError> {
        match tokio::fs::metadata(self.subject_path(subject_id)?).await {
            Ok(_) => Ok(false),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(true),
            Err(_) => Err(PrivacyError::Storage),
        }
    }
}

pub struct BackupKeyDeletionSurface {
    pool: PgPool,
}

impl BackupKeyDeletionSurface {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn put_for_test(
        &self,
        subject_id: &str,
        key_reference: &str,
        wrapped_key: &[u8],
    ) -> Result<(), PrivacyError> {
        validate_id(subject_id)?;
        validate_id(key_reference)?;
        if wrapped_key.is_empty() {
            return Err(PrivacyError::InvalidInput);
        }
        sqlx::query(
            "INSERT INTO privacy_subject_keys (subject_id, key_reference, wrapped_key) \
             VALUES ($1, $2, $3) ON CONFLICT (subject_id) DO UPDATE SET \
             key_reference = EXCLUDED.key_reference, wrapped_key = EXCLUDED.wrapped_key, \
             destroyed_at = NULL",
        )
        .bind(subject_id)
        .bind(key_reference)
        .bind(wrapped_key)
        .execute(&self.pool)
        .await
        .map_err(|_| PrivacyError::Database)?;
        Ok(())
    }
}

#[async_trait]
impl DeletionSurface for BackupKeyDeletionSurface {
    fn target(&self) -> DeletionTarget {
        DeletionTarget::BackupKey
    }

    async fn delete_subject(&self, subject_id: &str) -> Result<(), PrivacyError> {
        validate_id(subject_id)?;
        let affected = sqlx::query(
            "UPDATE privacy_subject_keys SET wrapped_key = NULL, destroyed_at = now() \
             WHERE subject_id = $1 AND wrapped_key IS NOT NULL AND destroyed_at IS NULL",
        )
        .bind(subject_id)
        .execute(&self.pool)
        .await
        .map_err(|_| PrivacyError::Database)?
        .rows_affected();
        if affected == 0 {
            let key_state = sqlx::query(
                "SELECT wrapped_key IS NULL AS material_destroyed, \
                        destroyed_at IS NOT NULL AS destruction_recorded \
                   FROM privacy_subject_keys WHERE subject_id = $1",
            )
            .bind(subject_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|_| PrivacyError::Database)?;
            if let Some(key_state) = key_state {
                return if key_state.get::<bool, _>("material_destroyed")
                    && key_state.get::<bool, _>("destruction_recorded")
                {
                    Ok(())
                } else {
                    Err(PrivacyError::InvalidPersistedState)
                };
            }
            let protected_events: bool = sqlx::query_scalar(
                "SELECT EXISTS (SELECT 1 FROM event_store WHERE data_subject_id = $1)",
            )
            .bind(subject_id)
            .fetch_one(&self.pool)
            .await
            .map_err(|_| PrivacyError::Database)?;
            if protected_events {
                return Err(PrivacyError::InvalidPersistedState);
            }
        }
        Ok(())
    }

    async fn verify_absent(&self, subject_id: &str) -> Result<bool, PrivacyError> {
        let material_exists = sqlx::query_scalar::<_, bool>(
            "SELECT wrapped_key IS NOT NULL FROM privacy_subject_keys WHERE subject_id = $1",
        )
        .bind(subject_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| PrivacyError::Database)?;
        let replayable_events: bool = sqlx::query_scalar(
            "SELECT EXISTS (\
                SELECT 1 FROM event_store AS event \
                 JOIN privacy_subject_keys AS subject_key \
                   ON subject_key.subject_id = event.data_subject_id \
                  AND subject_key.key_reference = event.payload_key_reference \
                WHERE event.data_subject_id = $1 \
                  AND subject_key.wrapped_key IS NOT NULL \
                  AND subject_key.destroyed_at IS NULL\
             )",
        )
        .bind(subject_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|_| PrivacyError::Database)?;
        Ok(!material_exists.unwrap_or(false) && !replayable_events)
    }
}

pub struct DeletionWorker {
    repository: PostgresDeletionRepository,
    legal_holds: std::sync::Arc<dyn LegalHoldResolver>,
    surfaces: HashMap<DeletionTarget, Box<dyn DeletionSurface>>,
}

impl DeletionWorker {
    pub fn new(
        repository: PostgresDeletionRepository,
        legal_holds: std::sync::Arc<dyn LegalHoldResolver>,
        surfaces: Vec<Box<dyn DeletionSurface>>,
    ) -> Result<Self, PrivacyError> {
        let mut by_target = HashMap::new();
        for surface in surfaces {
            let target = surface.target();
            if by_target.insert(target, surface).is_some() {
                return Err(PrivacyError::InvalidInput);
            }
        }
        Ok(Self {
            repository,
            legal_holds,
            surfaces: by_target,
        })
    }

    pub async fn execute_next(&self, limit: i64) -> Result<Vec<DeletionJob>, PrivacyError> {
        if !(1..=100).contains(&limit) {
            return Err(PrivacyError::InvalidInput);
        }
        let job_ids = sqlx::query_scalar::<_, String>(
            "SELECT job_id FROM privacy_deletion_jobs \
             WHERE evidence_status = 'confirmed' \
               AND status IN ('requested', 'blocked_legal_hold') \
             ORDER BY created_at, job_id LIMIT $1",
        )
        .bind(limit)
        .fetch_all(self.repository.pool())
        .await
        .map_err(|_| PrivacyError::Database)?;
        let mut completed = Vec::with_capacity(job_ids.len());
        for job_id in job_ids {
            match self.execute(&job_id).await {
                Ok(job) => completed.push(job),
                Err(PrivacyError::JobAlreadyRunning) => {}
                Err(error) => return Err(error),
            }
        }
        Ok(completed)
    }

    pub async fn execute(&self, job_id: &str) -> Result<DeletionJob, PrivacyError> {
        let job = self.repository.load(job_id).await?;
        let was_completed = job.status == DeletionJobStatus::Completed;
        if job.evidence_status != DeletionEvidenceStatus::Confirmed
            || job.canonical_event_sequence.is_none()
            || !job
                .canonical_event_integrity_hash
                .as_deref()
                .is_some_and(valid_integrity_hash)
        {
            return Err(PrivacyError::EvidenceUnconfirmed);
        }
        if !was_completed && self.legal_holds.has_active_hold(&job.subject_id).await? {
            self.repository
                .set_status(job_id, DeletionJobStatus::BlockedLegalHold, None)
                .await?;
            return self.repository.load(job_id).await;
        }
        if !was_completed
            && !self
                .repository
                .claim_execution(job_id, &job.subject_id)
                .await?
        {
            return self.repository.load(job_id).await;
        }

        for target in REQUIRED_DELETION_TARGETS {
            let current = self.repository.load(job_id).await?;
            let already_verified = current.targets.iter().any(|record| {
                record.target == target && record.status == DeletionTargetStatus::Verified
            });
            let surface = self
                .surfaces
                .get(&target)
                .ok_or(PrivacyError::MissingSurface(target));
            let surface = match surface {
                Ok(surface) => surface,
                Err(error) => {
                    self.fail(job_id, target, error.code()).await?;
                    return Err(error);
                }
            };
            if let Err(error) = surface.delete_subject(&job.subject_id).await {
                if !was_completed && !already_verified {
                    self.fail(job_id, target, error.code()).await?;
                }
                return Err(error);
            }
            if !already_verified {
                self.repository
                    .set_target_status(job_id, target, DeletionTargetStatus::Deleted, None)
                    .await?;
            }
            match surface.verify_absent(&job.subject_id).await {
                Ok(true) => {
                    if !already_verified {
                        self.repository
                            .set_target_status(job_id, target, DeletionTargetStatus::Verified, None)
                            .await?;
                    }
                }
                Ok(false) => {
                    let error = PrivacyError::VerificationFailed(target);
                    if !was_completed && !already_verified {
                        self.fail(job_id, target, error.code()).await?;
                    }
                    return Err(error);
                }
                Err(error) => {
                    if !was_completed && !already_verified {
                        self.fail(job_id, target, error.code()).await?;
                    }
                    return Err(error);
                }
            }
        }

        if was_completed {
            // Terminal rows are evidence, not authority. Re-read the durable
            // targets and return success only after every real surface has
            // just been deleted idempotently and independently proved absent.
            let reverified = self.repository.load(job_id).await?;
            return if reverified.all_targets_verified() {
                Ok(reverified)
            } else {
                Err(PrivacyError::InvalidPersistedState)
            };
        }

        self.repository
            .set_status(job_id, DeletionJobStatus::Verifying, None)
            .await?;
        let verified = self.repository.load(job_id).await?;
        if !verified.all_targets_verified() {
            self.repository
                .finish_execution(
                    job_id,
                    &job.subject_id,
                    DeletionJobStatus::Failed,
                    Some("DELETION_VERIFICATION_INCOMPLETE"),
                )
                .await?;
            return Err(PrivacyError::InvalidPersistedState);
        }
        self.repository
            .finish_execution(job_id, &job.subject_id, DeletionJobStatus::Completed, None)
            .await?;
        self.repository.load(job_id).await
    }

    async fn fail(
        &self,
        job_id: &str,
        target: DeletionTarget,
        code: &str,
    ) -> Result<(), PrivacyError> {
        self.repository
            .set_target_status(job_id, target, DeletionTargetStatus::Failed, Some(code))
            .await?;
        let subject_id = self.repository.load(job_id).await?.subject_id;
        self.repository
            .finish_execution(job_id, &subject_id, DeletionJobStatus::Failed, Some(code))
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_queue_message_is_classified_from_its_data_subject() {
        let payload = br#"{"data_subject_id":"victim_subject","payload":{"private":"value"}}"#;
        assert_eq!(
            retained_message_subject_digest(None, payload).unwrap(),
            format!("sha256:{}", sha256_hex(b"victim_subject"))
        );
    }

    #[test]
    fn queue_absence_proof_rejects_unclassified_or_mismatched_messages() {
        assert_eq!(
            retained_message_subject_digest(None, br#"{"payload":"unclassified"}"#),
            Err(PrivacyError::InvalidPersistedState)
        );
        assert_eq!(
            retained_message_subject_digest(
                Some("sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
                br#"{"data_subject_id":"victim_subject"}"#,
            ),
            Err(PrivacyError::InvalidPersistedState)
        );
    }
}
