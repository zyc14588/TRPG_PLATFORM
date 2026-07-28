use std::collections::HashSet;
use std::fmt;

use async_trait::async_trait;
use sha2::{Digest, Sha256};
use trpg_shared_kernel::{EntityId, KernelResult, PrincipalScope, Visibility, VisibilityKind};
use url::Url;
use zeroize::Zeroizing;

use crate::secret::SecretReference;

pub const MAX_CLOUD_CONTEXT_BYTES: u64 = 64 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderBoundary {
    Local,
    Cloud,
}

impl ProviderBoundary {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Cloud => "cloud",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConsentVisibilityScope {
    PublicOnly,
    SubjectPrivate,
}

impl ConsentVisibilityScope {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PublicOnly => "public_only",
            Self::SubjectPrivate => "subject_private",
        }
    }

    pub fn parse(value: &str) -> KernelResult<Self> {
        match value {
            "public_only" => Ok(Self::PublicOnly),
            "subject_private" => Ok(Self::SubjectPrivate),
            _ => Err(trpg_shared_kernel::TrpgError::PolicyEvidenceUntrusted),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CloudConsentQuery {
    pub subject_id: EntityId,
    pub target_provider: EntityId,
    pub purpose: EntityId,
    pub policy_version: EntityId,
    pub now_unix_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PersistedCloudConsent {
    consent_id: EntityId,
    subject_id: EntityId,
    target_provider: EntityId,
    purpose: EntityId,
    policy_version: EntityId,
    visibility_scope: ConsentVisibilityScope,
    expires_at_unix_ms: u64,
}

impl PersistedCloudConsent {
    /// Infrastructure adapters use this only after loading a granted consent
    /// from durable storage. Application callers never receive a constructor
    /// for `CloudEgressAuthorization` itself.
    #[allow(clippy::too_many_arguments)]
    pub fn loaded_from_repository(
        consent_id: EntityId,
        subject_id: EntityId,
        target_provider: EntityId,
        purpose: EntityId,
        policy_version: EntityId,
        visibility_scope: ConsentVisibilityScope,
        expires_at_unix_ms: u64,
    ) -> Self {
        Self {
            consent_id,
            subject_id,
            target_provider,
            purpose,
            policy_version,
            visibility_scope,
            expires_at_unix_ms,
        }
    }

    pub fn consent_id(&self) -> &EntityId {
        &self.consent_id
    }

    pub const fn visibility_scope(&self) -> ConsentVisibilityScope {
        self.visibility_scope
    }

    pub const fn expires_at_unix_ms(&self) -> u64 {
        self.expires_at_unix_ms
    }

    fn matches(&self, query: &CloudConsentQuery) -> bool {
        self.subject_id == query.subject_id
            && self.target_provider == query.target_provider
            && self.purpose == query.purpose
            && self.policy_version == query.policy_version
            && self.expires_at_unix_ms > query.now_unix_ms
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct CloudContextFact {
    fact_id: EntityId,
    visibility: Visibility,
    serialized_content: Zeroizing<Vec<u8>>,
}

impl fmt::Debug for CloudContextFact {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CloudContextFact")
            .field("fact_id", &self.fact_id)
            .field("visibility", &self.visibility)
            .field("serialized_bytes", &self.serialized_content.len())
            .field("serialized_content", &"[REDACTED]")
            .finish()
    }
}

impl CloudContextFact {
    /// Builds cloud context only from an integrity-verified, target-bound
    /// canonical fact capability. Callers cannot relabel the same bytes from
    /// KeeperOnly to Public or reuse evidence for another fact identifier.
    pub fn from_committed_fact(
        evidence: &trpg_domain_core::CommittedFactEvidence,
        serialized_content: impl Into<Vec<u8>>,
    ) -> KernelResult<Self> {
        let serialized_content = serialized_content.into();
        if serialized_content.is_empty()
            || serialized_content.len() > usize::try_from(MAX_CLOUD_CONTEXT_BYTES).unwrap_or(0)
        {
            return Err(trpg_shared_kernel::TrpgError::PolicyEvidenceUntrusted);
        }
        Ok(Self {
            fact_id: EntityId::new(evidence.target_fact_id().as_str())?,
            visibility: Visibility::try_from_parts(
                evidence.visibility().label().as_str(),
                evidence.visibility().subject_id().map(EntityId::as_str),
            )?,
            serialized_content: Zeroizing::new(serialized_content),
        })
    }

    pub fn fact_id(&self) -> &EntityId {
        &self.fact_id
    }

    pub fn visibility(&self) -> &Visibility {
        &self.visibility
    }

    pub fn serialized_bytes(&self) -> u64 {
        u64::try_from(self.serialized_content.len()).unwrap_or(u64::MAX)
    }

    /// Exposes exact bytes only inside the provider-adapter call that consumes
    /// a matching authorization.
    pub fn expose_serialized_to<R>(&self, consumer: impl FnOnce(&[u8]) -> R) -> R {
        consumer(self.serialized_content.as_slice())
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct CloudEgressRequest {
    pub snapshot_id: EntityId,
    pub audit_id: EntityId,
    pub subject_id: EntityId,
    pub source_provider: EntityId,
    pub target_provider: EntityId,
    pub source_endpoint: String,
    pub target_endpoint: String,
    pub model_id: EntityId,
    pub source_credential: SecretReference,
    pub target_credential: SecretReference,
    pub source_boundary: ProviderBoundary,
    pub target_boundary: ProviderBoundary,
    pub fallback_policy: EntityId,
    pub privacy_boundary: EntityId,
    pub purpose: EntityId,
    pub policy_version: EntityId,
    pub notice_reference: Option<EntityId>,
    pub target_audience: PrincipalScope,
    pub context: Vec<CloudContextFact>,
}

impl fmt::Debug for CloudEgressRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CloudEgressRequest")
            .field("snapshot_id", &self.snapshot_id)
            .field("audit_id", &self.audit_id)
            .field("subject_id", &self.subject_id)
            .field("source_provider", &self.source_provider)
            .field("target_provider", &self.target_provider)
            .field("source_endpoint", &"[REDACTED_ENDPOINT]")
            .field("target_endpoint", &"[REDACTED_ENDPOINT]")
            .field("model_id", &self.model_id)
            .field("source_credential", &self.source_credential)
            .field("target_credential", &self.target_credential)
            .field("source_boundary", &self.source_boundary)
            .field("target_boundary", &self.target_boundary)
            .field("fallback_policy", &self.fallback_policy)
            .field("privacy_boundary", &self.privacy_boundary)
            .field("purpose", &self.purpose)
            .field("policy_version", &self.policy_version)
            .field("notice_reference", &self.notice_reference)
            .field("target_audience", &self.target_audience)
            .field("context", &self.context)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloudEgressDecision {
    Allow,
    Deny,
}

impl CloudEgressDecision {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Deny => "deny",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloudEgressDenial {
    InvalidRoute,
    ConsentRequired,
    ConsentMismatch,
    NoticeRequired,
    ContextNotMinimized,
    ContextAudienceDenied,
    RestrictedContext,
    ConsentChanged,
}

impl CloudEgressDenial {
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidRoute => "CLOUD_EGRESS_INVALID_ROUTE",
            Self::ConsentRequired => "CLOUD_EGRESS_CONSENT_REQUIRED",
            Self::ConsentMismatch => "CLOUD_EGRESS_CONSENT_MISMATCH",
            Self::NoticeRequired => "CLOUD_EGRESS_NOTICE_REQUIRED",
            Self::ContextNotMinimized => "CLOUD_EGRESS_CONTEXT_NOT_MINIMIZED",
            Self::ContextAudienceDenied => "CLOUD_EGRESS_CONTEXT_AUDIENCE_DENIED",
            Self::RestrictedContext => "CLOUD_EGRESS_RESTRICTED_CONTEXT",
            Self::ConsentChanged => "CLOUD_EGRESS_CONSENT_CHANGED",
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct CloudRouteSnapshotRecord {
    pub snapshot_id: EntityId,
    pub subject_id: EntityId,
    pub consent_id: Option<EntityId>,
    pub source_provider: EntityId,
    pub target_provider: EntityId,
    pub source_endpoint: String,
    pub target_endpoint: String,
    pub model_id: EntityId,
    pub source_credential_id: String,
    pub source_credential_version: u64,
    pub target_credential_id: String,
    pub target_credential_version: u64,
    pub fallback_policy: EntityId,
    pub privacy_boundary: EntityId,
    pub consent_expires_at_unix_ms: Option<u64>,
    pub purpose: EntityId,
    pub policy_version: EntityId,
    pub notice_reference: Option<EntityId>,
    pub context_manifest_hash: String,
    pub allowed_fact_ids: Vec<EntityId>,
    pub decision: CloudEgressDecision,
    pub denial_code: Option<&'static str>,
    pub created_at_unix_ms: u64,
}

impl fmt::Debug for CloudRouteSnapshotRecord {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CloudRouteSnapshotRecord")
            .field("snapshot_id", &self.snapshot_id)
            .field("subject_id", &self.subject_id)
            .field("consent_id", &self.consent_id)
            .field("source_provider", &self.source_provider)
            .field("target_provider", &self.target_provider)
            .field("source_endpoint", &"[REDACTED_ENDPOINT]")
            .field("target_endpoint", &"[REDACTED_ENDPOINT]")
            .field("model_id", &self.model_id)
            .field("source_credential_id", &"[REDACTED]")
            .field("source_credential_version", &self.source_credential_version)
            .field("target_credential_id", &"[REDACTED]")
            .field("target_credential_version", &self.target_credential_version)
            .field("fallback_policy", &self.fallback_policy)
            .field("privacy_boundary", &self.privacy_boundary)
            .field(
                "consent_expires_at_unix_ms",
                &self.consent_expires_at_unix_ms,
            )
            .field("purpose", &self.purpose)
            .field("policy_version", &self.policy_version)
            .field("notice_reference", &self.notice_reference)
            .field("context_manifest_hash", &self.context_manifest_hash)
            .field("allowed_fact_ids", &self.allowed_fact_ids)
            .field("decision", &self.decision)
            .field("denial_code", &self.denial_code)
            .field("created_at_unix_ms", &self.created_at_unix_ms)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct CloudEgressAuditRecord {
    pub audit_id: EntityId,
    pub snapshot_id: EntityId,
    pub subject_id: EntityId,
    pub source_provider: EntityId,
    pub target_provider: EntityId,
    pub source_endpoint: String,
    pub target_endpoint: String,
    pub model_id: EntityId,
    pub source_credential_id: String,
    pub source_credential_version: u64,
    pub target_credential_id: String,
    pub target_credential_version: u64,
    pub fallback_policy: EntityId,
    pub privacy_boundary: EntityId,
    pub decision: CloudEgressDecision,
    pub denial_code: Option<&'static str>,
    pub context_manifest_hash: String,
    pub created_at_unix_ms: u64,
}
