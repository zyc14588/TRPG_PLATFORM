use crate::config_model::LocalModelCertification;
use crate::shared_kernel::{KernelResult, TrpgError};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LicensePolicy {
    Permissive,
    ReciprocalReviewRequired,
    ProprietaryReviewRequired,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReferenceUse {
    RuntimeDependency,
    DevelopmentTool,
    ResearchOnly,
    ModelProvider,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReferenceEntry {
    pub name: String,
    pub provenance_url: String,
    pub license_policy: LicensePolicy,
    pub intended_use: ReferenceUse,
    pub reviewed: bool,
}

pub fn validate_reference_entry(entry: &ReferenceEntry) -> KernelResult<()> {
    if entry.name.trim().is_empty() || entry.provenance_url.trim().is_empty() {
        return Err(TrpgError::OpenSourceReferenceViolation(
            "reference entries require name and provenance URL",
        ));
    }

    if entry.license_policy != LicensePolicy::Permissive && !entry.reviewed {
        return Err(TrpgError::OpenSourceReferenceViolation(
            "non-permissive references require review before implementation use",
        ));
    }

    Ok(())
}

pub fn validate_local_model_for_ai_keeper(
    certification: LocalModelCertification,
) -> KernelResult<()> {
    if certification < LocalModelCertification::Level4 {
        return Err(TrpgError::OpenSourceReferenceViolation(
            "local model requires Level 4 certification for AI Keeper orchestration",
        ));
    }

    Ok(())
}
