use std::collections::HashSet;

use trpg_contracts::WireErrorCode;

#[test]
fn every_wire_error_code_is_unique_and_screaming_snake_case() {
    let mut codes = HashSet::new();
    for code in WireErrorCode::ALL {
        assert!(code.is_screaming_snake_case(), "invalid code: {code}");
        assert!(codes.insert(code.as_str()), "duplicate code: {code}");
    }
}

#[test]
fn tool_permission_denied_uses_the_canonical_wire_spelling() {
    assert_eq!(
        WireErrorCode::ToolPermissionDenied.as_str(),
        "TOOL_PERMISSION_DENIED"
    );
    assert_ne!(
        WireErrorCode::ToolPermissionDenied.as_str(),
        "ToolPermissionDenied"
    );
}
