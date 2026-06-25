use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LicenseStatus {
    ApprovedOpen,
    ApprovedUserProvided,
    ApprovedOfficialSrd,
    PendingReview,
    RejectedUnknownLicense,
    RejectedCopyrightRisk,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LicenseCheck {
    pub status: LicenseStatus,
    pub reason: String,
}

pub fn check_declared_license(
    license_name: Option<&str>,
    declared_has_rights: bool,
) -> LicenseCheck {
    match (license_name, declared_has_rights) {
        (Some(name), _) if name.contains("CC") || name.eq_ignore_ascii_case("ORC") => {
            LicenseCheck {
                status: LicenseStatus::ApprovedOpen,
                reason: "recognized open license".to_owned(),
            }
        }
        (_, true) => LicenseCheck {
            status: LicenseStatus::ApprovedUserProvided,
            reason: "user declared rights".to_owned(),
        },
        _ => LicenseCheck {
            status: LicenseStatus::PendingReview,
            reason: "license missing or unclear".to_owned(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_license_goes_pending_review() {
        let check = check_declared_license(None, false);
        assert_eq!(check.status, LicenseStatus::PendingReview);
    }
}
