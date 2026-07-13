use trpg_extension_sdk::{ExtensionSdkError, TrpgError, WireErrorCode};

#[test]
fn every_extension_error_code_is_in_the_canonical_wire_registry() {
    let extension_codes = [
        WireErrorCode::ExtensionStateWriteForbidden,
        WireErrorCode::ExtensionDirectLlmForbidden,
        WireErrorCode::ExtensionDatabaseWriteForbidden,
        WireErrorCode::ExtensionToolGateBypassForbidden,
        WireErrorCode::ExtensionAuthorityContractForbidden,
        WireErrorCode::ExtensionDiceForgeForbidden,
        WireErrorCode::ExtensionVisibilityLeakForbidden,
        WireErrorCode::ExtensionCapabilityDenied,
        WireErrorCode::ExtensionToolGrantDenied,
        WireErrorCode::ExtensionOpenFgaDenied,
        WireErrorCode::ExtensionOpaDenied,
        WireErrorCode::ExtensionAuditRequired,
        WireErrorCode::ExtensionCompatibilityFieldsMissing,
        WireErrorCode::ExtensionCompatibilityRejected,
    ];

    for code in extension_codes {
        assert_eq!(WireErrorCode::lookup(code.as_str()), Ok(code));
        assert_eq!(
            ExtensionSdkError::ForbiddenCapability(code).wire_code(),
            code
        );
        assert_eq!(
            ExtensionSdkError::CompatibilityRejected(code).code(),
            code.as_str()
        );
    }

    let kernel = ExtensionSdkError::Kernel(TrpgError::PolicyDenied);
    assert_eq!(kernel.wire_code(), WireErrorCode::PolicyDenied);
    assert_eq!(
        WireErrorCode::lookup(kernel.code()),
        Ok(WireErrorCode::PolicyDenied)
    );
}
