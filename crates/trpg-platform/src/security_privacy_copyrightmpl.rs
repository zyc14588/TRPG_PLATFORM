use crate::readme::redact_for_observability;
use trpg_shared_kernel::{CommandEnvelope, EventEnvelope, EventStore, KernelResult, TrpgError};

pub const SECURITY_PRIVACY_COPYRIGHT_REVIEWED_EVENT: &str =
    "platform.security_privacy_copyrightmpl.reviewed";
pub const SECURITY_PRIVACY_COPYRIGHTMPL_METRIC_MODULE: &str = "security_privacy_copyrightmpl";
pub const SECURITY_PRIVACY_COPYRIGHTMPL_REQUIRED_METRICS: &[&str] = &[
    "trpg_command_total",
    "trpg_event_append_latency_ms",
    "trpg_policy_deny_total",
    "trpg_projection_lag_events",
    "trpg_visibility_redaction_total",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewSecurityPrivacyCopyrightPolicy {
    pub asset_id: String,
    pub license_tag: String,
    pub detail: String,
    pub contains_restricted_visibility: bool,
    pub export_allowed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SecurityPrivacyCopyrightEvent {
    SecurityPrivacyCopyrightReviewed {
        asset_id: String,
        license_tag: String,
        detail: String,
        export_allowed: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SecurityPrivacyCopyrightError {
    AssetIdRequired,
    LicenseTagRequired,
    RestrictedVisibilityExportDenied,
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
            SecurityPrivacyCopyrightError::RestrictedVisibilityExportDenied => {
                TrpgError::VisibilityDenied
            }
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
}

pub fn review_security_privacy_copyright_policy(
    repository: &mut SecurityPrivacyCopyrightRepository,
    command: &CommandEnvelope<ReviewSecurityPrivacyCopyrightPolicy>,
) -> KernelResult<SecurityPrivacyCopyrightEventEnvelope> {
    SecurityPrivacyCopyrightService::review_security_privacy_copyright_policy(repository, command)
}
