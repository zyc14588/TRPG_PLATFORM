use crate::readme::redact_for_observability;
use trpg_security_governance::{
    evaluate_derived_visibility, DerivationRequest, DerivedObject, RedactionOutcome,
};
use trpg_shared_kernel::{
    validate_command_envelope, CommandEnvelope, EntityId, EventEnvelope, EventStore, KernelResult,
    PrincipalScope, TrpgError,
};

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
    pub export_intent: ExportIntent,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExportIntent {
    ReviewOnly,
    ExportTo(ExportAudience),
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub enum ExportAudience {
    Public,
    Party,
    Player(EntityId),
}

impl ExportAudience {
    fn principal(&self) -> PrincipalScope {
        match self {
            Self::Public => PrincipalScope::Public,
            Self::Party => PrincipalScope::PartyMember,
            Self::Player(player_id) => PrincipalScope::Player(player_id.clone()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub enum ExportDisposition {
    ReviewOnly,
    Authorized { audience: ExportAudience },
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub enum SecurityPrivacyCopyrightEvent {
    SecurityPrivacyCopyrightReviewed {
        asset_id: String,
        license_tag: String,
        detail: String,
        export: ExportDisposition,
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
        validate_command_envelope(command)?;
        let export = derive_export_disposition(command)?;

        repository.append(
            command,
            SECURITY_PRIVACY_COPYRIGHT_REVIEWED_EVENT,
            SecurityPrivacyCopyrightEvent::SecurityPrivacyCopyrightReviewed {
                asset_id: command.payload.asset_id.clone(),
                license_tag: command.payload.license_tag.clone(),
                detail: redact_for_observability(&command.visibility, &command.payload.detail),
                export,
            },
        )
    }
}

fn derive_export_disposition(
    command: &CommandEnvelope<ReviewSecurityPrivacyCopyrightPolicy>,
) -> KernelResult<ExportDisposition> {
    let ExportIntent::ExportTo(audience) = &command.payload.export_intent else {
        return Ok(ExportDisposition::ReviewOnly);
    };
    let sources = [command.visibility.clone()];
    let target = audience.principal();
    let decision = evaluate_derived_visibility(DerivationRequest {
        sources: &sources,
        processor: &PrincipalScope::System,
        target_audience: &target,
        target: DerivedObject::PlayerExport,
    });
    if decision.outcome != RedactionOutcome::Visible {
        return Err(SecurityPrivacyCopyrightError::RestrictedVisibilityExportDenied.into());
    }
    Ok(ExportDisposition::Authorized {
        audience: audience.clone(),
    })
}

pub fn review_security_privacy_copyright_policy(
    repository: &mut SecurityPrivacyCopyrightRepository,
    command: &CommandEnvelope<ReviewSecurityPrivacyCopyrightPolicy>,
) -> KernelResult<SecurityPrivacyCopyrightEventEnvelope> {
    SecurityPrivacyCopyrightService::review_security_privacy_copyright_policy(repository, command)
}
