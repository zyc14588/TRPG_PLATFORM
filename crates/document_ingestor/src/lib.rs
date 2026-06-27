pub use rag_core::{LicenseDecision as LicenseCheck, LicenseStatus};

pub fn check_declared_license(
    license_name: Option<&str>,
    declared_has_rights: bool,
) -> LicenseCheck {
    rag_core::check_declared_license(license_name, declared_has_rights)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_license_goes_pending_review() {
        let check = check_declared_license(None, false);
        assert_eq!(check.status, LicenseStatus::PendingReview);
    }

    #[test]
    fn recognized_open_license_is_allowed() {
        let check = check_declared_license(Some("CC-BY-4.0"), false);
        assert_eq!(check.status, LicenseStatus::Allowed);
    }
}
