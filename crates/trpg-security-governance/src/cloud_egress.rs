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

impl fmt::Debug for CloudEgressAuditRecord {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CloudEgressAuditRecord")
            .field("audit_id", &self.audit_id)
            .field("snapshot_id", &self.snapshot_id)
            .field("subject_id", &self.subject_id)
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
            .field("decision", &self.decision)
            .field("denial_code", &self.denial_code)
            .field("context_manifest_hash", &self.context_manifest_hash)
            .field("created_at_unix_ms", &self.created_at_unix_ms)
            .finish()
    }
}

#[async_trait]
pub trait CloudEgressLedger: Send + Sync {
    /// Returns time from a trusted infrastructure clock (for PostgreSQL,
    /// `clock_timestamp()`), never from the authorization caller.
    async fn trusted_now_unix_ms(&self) -> KernelResult<u64>;

    /// Confirms the notice reference is durable and bound to the same subject,
    /// privacy policy, and canonical evidence.
    async fn notice_is_recorded(
        &self,
        notice_reference: &EntityId,
        subject_id: &EntityId,
        policy_version: &EntityId,
    ) -> KernelResult<bool>;

    async fn load_active_consent(
        &self,
        query: &CloudConsentQuery,
    ) -> KernelResult<Option<PersistedCloudConsent>>;

    /// Persists the route snapshot and audit together. For an allow candidate,
    /// the implementation must revalidate the consent in the same transaction.
    /// `false` means the candidate was atomically downgraded to a denied audit.
    async fn record_route_decision(
        &self,
        snapshot: CloudRouteSnapshotRecord,
        audit: CloudEgressAuditRecord,
    ) -> KernelResult<bool>;
}

#[derive(PartialEq, Eq)]
pub struct CloudEgressAuthorization {
    snapshot_id: EntityId,
    source_provider: EntityId,
    target_provider: EntityId,
    source_endpoint: String,
    target_endpoint: String,
    model_id: EntityId,
    subject_id: EntityId,
    purpose: EntityId,
    policy_version: EntityId,
    consent_id: EntityId,
    consent_expires_at_unix_ms: u64,
    notice_reference: EntityId,
    source_credential: SecretReference,
    target_credential: SecretReference,
    fallback_policy: EntityId,
    privacy_boundary: EntityId,
    context_manifest_hash: String,
    allowed_fact_ids: Vec<EntityId>,
}

impl fmt::Debug for CloudEgressAuthorization {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CloudEgressAuthorization")
            .field("snapshot_id", &self.snapshot_id)
            .field("source_provider", &self.source_provider)
            .field("target_provider", &self.target_provider)
            .field("source_endpoint", &"[REDACTED_ENDPOINT]")
            .field("target_endpoint", &"[REDACTED_ENDPOINT]")
            .field("model_id", &self.model_id)
            .field("subject_id", &self.subject_id)
            .field("purpose", &self.purpose)
            .field("policy_version", &self.policy_version)
            .field("consent_id", &self.consent_id)
            .field(
                "consent_expires_at_unix_ms",
                &self.consent_expires_at_unix_ms,
            )
            .field("notice_reference", &self.notice_reference)
            .field("source_credential", &self.source_credential)
            .field("target_credential", &self.target_credential)
            .field("fallback_policy", &self.fallback_policy)
            .field("privacy_boundary", &self.privacy_boundary)
            .field("context_manifest_hash", &self.context_manifest_hash)
            .field("allowed_fact_count", &self.allowed_fact_ids.len())
            .finish()
    }
}

impl CloudEgressAuthorization {
    pub fn snapshot_id(&self) -> &EntityId {
        &self.snapshot_id
    }

    pub fn context_manifest_hash(&self) -> &str {
        &self.context_manifest_hash
    }

    pub fn allows_fact(&self, fact_id: &EntityId) -> bool {
        self.allowed_fact_ids
            .iter()
            .any(|allowed| allowed == fact_id)
    }

    /// Binds the one-shot route authorization to the exact context manifest
    /// that was minimized, audience-checked, persisted, and audited. A token
    /// minted for one set (or order) of facts cannot authorize another model
    /// payload merely because the provider route is the same.
    pub fn permits_context(&self, attempt: CloudEgressAttempt<'_>) -> bool {
        self.source_provider.as_str() == attempt.source_provider
            && self.target_provider.as_str() == attempt.target_provider
            && self.source_endpoint == attempt.source_endpoint
            && self.target_endpoint == attempt.target_endpoint
            && self.model_id.as_str() == attempt.model_id
            && &self.source_credential == attempt.source_credential
            && &self.target_credential == attempt.target_credential
            && self.fallback_policy.as_str() == attempt.fallback_policy
            && self.privacy_boundary.as_str() == attempt.privacy_boundary
            && self.context_manifest_hash == context_manifest_hash(attempt.context)
            && self.allowed_fact_ids.len() == attempt.context.len()
            && self
                .allowed_fact_ids
                .iter()
                .zip(attempt.context)
                .all(|(allowed, fact)| allowed == &fact.fact_id)
    }

    /// Rechecks consent, expiry, and notice against the trusted ledger at the
    /// last boundary before credentials or context bytes are exposed.
    pub async fn revalidate_for_send(&self, ledger: &impl CloudEgressLedger) -> KernelResult<bool> {
        let now_unix_ms = ledger.trusted_now_unix_ms().await?;
        if now_unix_ms >= self.consent_expires_at_unix_ms {
            return Ok(false);
        }
        let query = CloudConsentQuery {
            subject_id: self.subject_id.clone(),
            target_provider: self.target_provider.clone(),
            purpose: self.purpose.clone(),
            policy_version: self.policy_version.clone(),
            now_unix_ms,
        };
        let consent = ledger.load_active_consent(&query).await?;
        let consent_matches = consent.as_ref().is_some_and(|value| {
            value.matches(&query)
                && value.consent_id() == &self.consent_id
                && value.expires_at_unix_ms() == self.consent_expires_at_unix_ms
        });
        Ok(consent_matches
            && ledger
                .notice_is_recorded(
                    &self.notice_reference,
                    &self.subject_id,
                    &self.policy_version,
                )
                .await?)
    }
}

/// Exact provider-route and context tuple presented at the final send
/// boundary. Grouping it as one value makes it difficult for callers to omit
/// a binding when the route contract evolves.
pub struct CloudEgressAttempt<'a> {
    pub source_provider: &'a str,
    pub target_provider: &'a str,
    pub source_endpoint: &'a str,
    pub target_endpoint: &'a str,
    pub model_id: &'a str,
    pub source_credential: &'a SecretReference,
    pub target_credential: &'a SecretReference,
    pub fallback_policy: &'a str,
    pub privacy_boundary: &'a str,
    pub context: &'a [CloudContextFact],
}

#[derive(Debug, PartialEq, Eq)]
pub enum CloudEgressOutcome {
    Authorized(Box<CloudEgressAuthorization>),
    Denied {
        snapshot_id: EntityId,
        reason: CloudEgressDenial,
    },
}

pub async fn authorize_cloud_egress(
    ledger: &impl CloudEgressLedger,
    request: CloudEgressRequest,
) -> KernelResult<CloudEgressOutcome> {
    let now_unix_ms = ledger.trusted_now_unix_ms().await?;
    let query = CloudConsentQuery {
        subject_id: request.subject_id.clone(),
        target_provider: request.target_provider.clone(),
        purpose: request.purpose.clone(),
        policy_version: request.policy_version.clone(),
        now_unix_ms,
    };
    let consent = ledger.load_active_consent(&query).await?;
    let manifest_hash = context_manifest_hash(&request.context);

    let mut denial = if request.source_boundary != ProviderBoundary::Local
        || request.target_boundary != ProviderBoundary::Cloud
        || !valid_local_source_endpoint(&request.source_endpoint)
        || !valid_cloud_target_endpoint(&request.target_endpoint)
        || request.fallback_policy.as_str() != "explicit_audited_only"
        || request.privacy_boundary.as_str() != "explicit_consent_no_silent_fallback"
    {
        Some(CloudEgressDenial::InvalidRoute)
    } else if request.notice_reference.is_none()
        || !ledger
            .notice_is_recorded(
                request
                    .notice_reference
                    .as_ref()
                    .expect("notice presence checked"),
                &request.subject_id,
                &request.policy_version,
            )
            .await?
    {
        Some(CloudEgressDenial::NoticeRequired)
    } else if consent.is_none() {
        Some(CloudEgressDenial::ConsentRequired)
    } else {
        None
    };

    if denial.is_none() && !consent.as_ref().is_some_and(|value| value.matches(&query)) {
        denial = Some(CloudEgressDenial::ConsentMismatch);
    }

    let mut allowed_fact_ids = Vec::with_capacity(request.context.len());
    if denial.is_none() {
        denial = evaluate_context(
            &request,
            consent.as_ref().expect("consent checked above"),
            &mut allowed_fact_ids,
        );
    }

    let candidate_decision = if denial.is_none() {
        CloudEgressDecision::Allow
    } else {
        CloudEgressDecision::Deny
    };
    let candidate_denial_code = denial.map(CloudEgressDenial::code);
    let snapshot = CloudRouteSnapshotRecord {
        snapshot_id: request.snapshot_id.clone(),
        subject_id: request.subject_id.clone(),
        consent_id: consent.as_ref().map(|value| value.consent_id.clone()),
        source_provider: request.source_provider.clone(),
        target_provider: request.target_provider.clone(),
        source_endpoint: request.source_endpoint.clone(),
        target_endpoint: request.target_endpoint.clone(),
        model_id: request.model_id.clone(),
        source_credential_id: request.source_credential.secret_id().to_owned(),
        source_credential_version: request.source_credential.version(),
        target_credential_id: request.target_credential.secret_id().to_owned(),
        target_credential_version: request.target_credential.version(),
        fallback_policy: request.fallback_policy.clone(),
        privacy_boundary: request.privacy_boundary.clone(),
        consent_expires_at_unix_ms: consent
            .as_ref()
            .map(PersistedCloudConsent::expires_at_unix_ms),
        purpose: request.purpose.clone(),
        policy_version: request.policy_version.clone(),
        notice_reference: request.notice_reference.clone(),
        context_manifest_hash: manifest_hash.clone(),
        allowed_fact_ids: allowed_fact_ids.clone(),
        decision: candidate_decision,
        denial_code: candidate_denial_code,
        created_at_unix_ms: now_unix_ms,
    };
    let audit = CloudEgressAuditRecord {
        audit_id: request.audit_id,
        snapshot_id: request.snapshot_id.clone(),
        subject_id: request.subject_id.clone(),
        source_provider: request.source_provider.clone(),
        target_provider: request.target_provider.clone(),
        source_endpoint: request.source_endpoint.clone(),
        target_endpoint: request.target_endpoint.clone(),
        model_id: request.model_id.clone(),
        source_credential_id: request.source_credential.secret_id().to_owned(),
        source_credential_version: request.source_credential.version(),
        target_credential_id: request.target_credential.secret_id().to_owned(),
        target_credential_version: request.target_credential.version(),
        fallback_policy: request.fallback_policy.clone(),
        privacy_boundary: request.privacy_boundary.clone(),
        decision: candidate_decision,
        denial_code: candidate_denial_code,
        context_manifest_hash: manifest_hash.clone(),
        created_at_unix_ms: now_unix_ms,
    };
    let persisted_as_candidate = ledger.record_route_decision(snapshot, audit).await?;

    if let Some(reason) = denial {
        return Ok(CloudEgressOutcome::Denied {
            snapshot_id: request.snapshot_id,
            reason,
        });
    }
    if !persisted_as_candidate {
        return Ok(CloudEgressOutcome::Denied {
            snapshot_id: request.snapshot_id,
            reason: CloudEgressDenial::ConsentChanged,
        });
    }

    let consent = consent.expect("authorized cloud route has durable consent");
    Ok(CloudEgressOutcome::Authorized(Box::new(
        CloudEgressAuthorization {
            snapshot_id: request.snapshot_id,
            source_provider: request.source_provider,
            target_provider: request.target_provider,
            source_endpoint: request.source_endpoint,
            target_endpoint: request.target_endpoint,
            model_id: request.model_id,
            subject_id: request.subject_id,
            purpose: request.purpose,
            policy_version: request.policy_version,
            consent_id: consent.consent_id,
            consent_expires_at_unix_ms: consent.expires_at_unix_ms,
            notice_reference: request
                .notice_reference
                .expect("authorized cloud route has durable notice"),
            source_credential: request.source_credential,
            target_credential: request.target_credential,
            fallback_policy: request.fallback_policy,
            privacy_boundary: request.privacy_boundary,
            context_manifest_hash: manifest_hash,
            allowed_fact_ids,
        },
    )))
}

fn evaluate_context(
    request: &CloudEgressRequest,
    consent: &PersistedCloudConsent,
    allowed_fact_ids: &mut Vec<EntityId>,
) -> Option<CloudEgressDenial> {
    if request.context.is_empty() {
        return Some(CloudEgressDenial::ContextNotMinimized);
    }
    let mut total_bytes = 0_u64;
    let mut seen = HashSet::new();
    for fact in &request.context {
        total_bytes = match total_bytes.checked_add(fact.serialized_bytes()) {
            Some(total) if total <= MAX_CLOUD_CONTEXT_BYTES => total,
            _ => return Some(CloudEgressDenial::ContextNotMinimized),
        };
        if !seen.insert(fact.fact_id.clone()) {
            return Some(CloudEgressDenial::ContextNotMinimized);
        }
        if !fact.visibility.can_view(&request.target_audience) {
            return Some(CloudEgressDenial::ContextAudienceDenied);
        }
        let allowed = match fact.visibility.label().kind() {
            VisibilityKind::Public => true,
            VisibilityKind::SpectatorVisible => false,
            VisibilityKind::PrivateToPlayer | VisibilityKind::InvestigatorPrivate => {
                consent.visibility_scope == ConsentVisibilityScope::SubjectPrivate
                    && fact.visibility.subject_id() == Some(&request.subject_id)
            }
            VisibilityKind::PartyVisible
            | VisibilityKind::PrivateToGroup
            | VisibilityKind::KeeperOnly
            | VisibilityKind::AiInternal
            | VisibilityKind::SystemOnly
            | VisibilityKind::SpectatorHidden
            | VisibilityKind::SystemPrivate => false,
        };
        if !allowed {
            return Some(CloudEgressDenial::RestrictedContext);
        }
        allowed_fact_ids.push(fact.fact_id.clone());
    }
    None
}

fn context_manifest_hash(context: &[CloudContextFact]) -> String {
    let mut digest = Sha256::new();
    digest.update(b"trpg-cloud-context-manifest-v2\0");
    for fact in context {
        update_length_prefixed(&mut digest, fact.fact_id.as_str().as_bytes());
        update_length_prefixed(&mut digest, fact.visibility.label().as_str().as_bytes());
        update_length_prefixed(
            &mut digest,
            fact.visibility
                .subject_id()
                .map(EntityId::as_str)
                .unwrap_or("")
                .as_bytes(),
        );
        digest.update(fact.serialized_bytes().to_be_bytes());
        fact.expose_serialized_to(|bytes| update_length_prefixed(&mut digest, bytes));
    }
    format!("{:x}", digest.finalize())
}

fn valid_local_source_endpoint(value: &str) -> bool {
    let Ok(url) = Url::parse(value) else {
        return false;
    };
    matches!(url.scheme(), "http" | "https")
        && url
            .host_str()
            .is_some_and(|host| matches!(host, "localhost" | "127.0.0.1" | "::1"))
        && url.username().is_empty()
        && url.password().is_none()
        && url.query().is_none()
        && url.fragment().is_none()
}

fn valid_cloud_target_endpoint(value: &str) -> bool {
    let Ok(url) = Url::parse(value) else {
        return false;
    };
    url.scheme() == "https"
        && url.host_str().is_some()
        && url.username().is_empty()
        && url.password().is_none()
        && url.query().is_none()
        && url.fragment().is_none()
}

fn update_length_prefixed(digest: &mut Sha256, value: &[u8]) {
    digest.update((value.len() as u64).to_be_bytes());
    digest.update(value);
}
