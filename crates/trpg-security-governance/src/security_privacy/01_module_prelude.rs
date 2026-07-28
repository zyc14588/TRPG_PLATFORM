use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::cloud_egress::{
    CloudConsentQuery, CloudEgressAuditRecord, CloudEgressDecision, CloudEgressDenial,
    CloudEgressLedger, CloudRouteSnapshotRecord, ConsentVisibilityScope, PersistedCloudConsent,
};
use crate::{
    evaluate_security_governance, SecurityGovernanceCommand, SecurityGovernanceEventEnvelope,
    SecurityGovernanceRepository,
};
use async_trait::async_trait;
use percent_encoding::percent_decode_str;
use redis::aio::ConnectionManager;
use s3::{creds::Credentials, serde_types::ObjectIdentifier, Bucket, Region};
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::migrate::Migrator;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions, PgSslMode};
use sqlx::{PgPool, Row};
use trpg_shared_kernel::{CommandEnvelope, EntityId, KernelResult, TrpgError};
use url::Url;

pub const MODULE: &str = "security_governance::security_privacy";

pub fn evaluate(
    repository: &mut SecurityGovernanceRepository,
    command: &CommandEnvelope<SecurityGovernanceCommand>,
) -> KernelResult<SecurityGovernanceEventEnvelope> {
    evaluate_security_governance(MODULE, repository, command)
}

pub const REQUIRED_DELETION_TARGETS: [DeletionTarget; 7] = [
    DeletionTarget::Database,
    DeletionTarget::RagIndex,
    DeletionTarget::ObjectStorage,
    DeletionTarget::Cache,
    DeletionTarget::Export,
    DeletionTarget::BackupKey,
    DeletionTarget::Queue,
];
const DELETION_EXECUTION_LEASE_SECONDS: i32 = 300;
const DELETION_LEASE_EXPIRED_CODE: &str = "DELETION_LEASE_EXPIRED";
pub const MAX_DELETION_LEASE_RECOVERIES: i64 = 3;

/// The workspace migration tree is the sole executable schema source.
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
    DeletionEvidenceMismatch,
    LegacyTwoPhaseDisabled,
    JobAlreadyRunning,
    ExecutionLeaseExpired,
    LeaseRecoveryExhausted,
    DeletionInProgress,
    ProtectedCanonicalIdentity,
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
            Self::DeletionEvidenceMismatch => "DELETION_CANONICAL_EVIDENCE_MISMATCH",
            Self::LegacyTwoPhaseDisabled => "DELETION_TWO_PHASE_REQUEST_DISABLED",
            Self::JobAlreadyRunning => "DELETION_JOB_ALREADY_RUNNING",
            Self::ExecutionLeaseExpired => "DELETION_EXECUTION_LEASE_EXPIRED",
            Self::LeaseRecoveryExhausted => "DELETION_LEASE_RECOVERY_EXHAUSTED",
            Self::DeletionInProgress => "DELETION_IN_PROGRESS",
            Self::ProtectedCanonicalIdentity => "DELETION_CANONICAL_IDENTITY_REQUIRES_FORK",
        }
    }
}

impl fmt::Display for PrivacyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

impl std::error::Error for PrivacyError {}

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
