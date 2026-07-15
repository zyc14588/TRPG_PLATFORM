use crate::readme::redact_for_observability;
use trpg_shared_kernel::{CommandEnvelope, EventEnvelope, EventStore, KernelResult, TrpgError};

pub const SECURITY_PRIVACY_COPYRIGHT_REVIEWED_EVENT: &str =
    "platform.security_privacy_copyright.reviewed";
pub const DATA_DELETION_REQUESTED_EVENT: &str =
    "platform.security_privacy_copyright.data_deletion_requested";
pub const SECURITY_PRIVACY_COPYRIGHT_METRIC_MODULE: &str = "security_privacy_copyright";
pub const SECURITY_PRIVACY_COPYRIGHT_REQUIRED_METRICS: &[&str] = &[
    "trpg_command_total",
    "trpg_event_append_latency_ms",
    "trpg_policy_deny_total",
    "trpg_visibility_redaction_total",
    "trpg_data_deletion_request_total",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SecurityPolicyDecision {
    Permit,
    Deny,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SecurityPolicyGate {
    pub openfga: SecurityPolicyDecision,
    pub opa: SecurityPolicyDecision,
    pub tool_grant: SecurityPolicyDecision,
}

impl SecurityPolicyGate {
    pub fn permit_all() -> Self {
        Self {
            openfga: SecurityPolicyDecision::Permit,
            opa: SecurityPolicyDecision::Permit,
            tool_grant: SecurityPolicyDecision::Permit,
        }
    }

    fn permits(&self) -> bool {
        self.openfga == SecurityPolicyDecision::Permit
            && self.opa == SecurityPolicyDecision::Permit
            && self.tool_grant == SecurityPolicyDecision::Permit
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewSecurityPrivacyCopyrightPolicy {
    pub asset_id: String,
    pub license_tag: String,
    pub detail: String,
    pub contains_restricted_visibility: bool,
    pub export_allowed: bool,
    pub policy_gate: SecurityPolicyGate,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RequestDataDeletion {
    pub subject_id: String,
    pub retention_policy: String,
    pub reason: String,
    pub policy_gate: SecurityPolicyGate,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub enum SecurityPrivacyCopyrightEvent {
    SecurityPrivacyCopyrightReviewed {
        asset_id: String,
        license_tag: String,
        detail: String,
        export_allowed: bool,
    },
    DataDeletionRequested {
        subject_id: String,
        retention_policy: String,
        reason: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SecurityPrivacyCopyrightError {
    AssetIdRequired,
    LicenseTagRequired,
    SubjectIdRequired,
    RetentionPolicyRequired,
    RestrictedVisibilityExportDenied,
    PolicyDenied,
}

impl From<SecurityPrivacyCopyrightError> for TrpgError {
    fn from(error: SecurityPrivacyCopyrightError) -> Self {
        match error {
            SecurityPrivacyCopyrightError::AssetIdRequired => {
                TrpgError::InvalidConfiguration("asset_id_required")
            }
            SecurityPrivacyCopyrightError::LicenseTagRequired => {
                TrpgError::InvalidConfiguration("license_tag_required")
            }
            SecurityPrivacyCopyrightError::SubjectIdRequired => {
                TrpgError::InvalidConfiguration("subject_id_required")
            }
            SecurityPrivacyCopyrightError::RetentionPolicyRequired => {
                TrpgError::InvalidConfiguration("retention_policy_required")
            }
            SecurityPrivacyCopyrightError::RestrictedVisibilityExportDenied => {
                TrpgError::VisibilityDenied
            }
            SecurityPrivacyCopyrightError::PolicyDenied => TrpgError::PolicyDenied,
        }
    }
}

pub type SecurityPrivacyCopyrightEventEnvelope = EventEnvelope<SecurityPrivacyCopyrightEvent>;
pub type SecurityPrivacyCopyrightRepository = EventStore<SecurityPrivacyCopyrightEvent>;

pub struct SecurityPrivacyCopyrightService;

impl SecurityPrivacyCopyrightService {
    pub fn review_security_privacy_copyright_policy(
        repository: &mut SecurityPrivacyCopyrightRepository,
        command: &CommandEnvelope<ReviewSecurityPrivacyCopyrightPolicy>,
    ) -> KernelResult<SecurityPrivacyCopyrightEventEnvelope> {
        if command.payload.asset_id.trim().is_empty() {
            return Err(SecurityPrivacyCopyrightError::AssetIdRequired.into());
        }
        if command.payload.license_tag.trim().is_empty() {
            return Err(SecurityPrivacyCopyrightError::LicenseTagRequired.into());
        }
        if !command.payload.policy_gate.permits() {
            return Err(SecurityPrivacyCopyrightError::PolicyDenied.into());
        }
        if command.payload.contains_restricted_visibility && command.payload.export_allowed {
            return Err(SecurityPrivacyCopyrightError::RestrictedVisibilityExportDenied.into());
        }

        repository.append(
            command,
            SECURITY_PRIVACY_COPYRIGHT_REVIEWED_EVENT,
            SecurityPrivacyCopyrightEvent::SecurityPrivacyCopyrightReviewed {
                asset_id: command.payload.asset_id.clone(),
                license_tag: command.payload.license_tag.clone(),
                detail: redact_for_observability(&command.visibility, &command.payload.detail),
                export_allowed: command.payload.export_allowed,
            },
        )
    }

    pub fn request_data_deletion(
        repository: &mut SecurityPrivacyCopyrightRepository,
        command: &CommandEnvelope<RequestDataDeletion>,
    ) -> KernelResult<SecurityPrivacyCopyrightEventEnvelope> {
        if command.payload.subject_id.trim().is_empty() {
            return Err(SecurityPrivacyCopyrightError::SubjectIdRequired.into());
        }
        if command.payload.retention_policy.trim().is_empty() {
            return Err(SecurityPrivacyCopyrightError::RetentionPolicyRequired.into());
        }
        if !command.payload.policy_gate.permits() {
            return Err(SecurityPrivacyCopyrightError::PolicyDenied.into());
        }

        repository.append(
            command,
            DATA_DELETION_REQUESTED_EVENT,
            SecurityPrivacyCopyrightEvent::DataDeletionRequested {
                subject_id: command.payload.subject_id.clone(),
                retention_policy: command.payload.retention_policy.clone(),
                reason: redact_for_observability(&command.visibility, &command.payload.reason),
            },
        )
    }
}

pub fn review_security_privacy_copyright_policy(
    repository: &mut SecurityPrivacyCopyrightRepository,
    command: &CommandEnvelope<ReviewSecurityPrivacyCopyrightPolicy>,
) -> KernelResult<SecurityPrivacyCopyrightEventEnvelope> {
    SecurityPrivacyCopyrightService::review_security_privacy_copyright_policy(repository, command)
}

pub fn request_data_deletion(
    repository: &mut SecurityPrivacyCopyrightRepository,
    command: &CommandEnvelope<RequestDataDeletion>,
) -> KernelResult<SecurityPrivacyCopyrightEventEnvelope> {
    SecurityPrivacyCopyrightService::request_data_deletion(repository, command)
}
