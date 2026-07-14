mod common;

use trpg_extension_sdk::tool_provider_sdk::{
    append_tool_provider_sdk_event, authorize_tool_invocation, contract, ToolProviderManifest,
    ToolProviderSdkCommand, ALLOWED_CAPABILITIES,
};
use trpg_extension_sdk::ExtensionPolicyGate;

#[test]
fn tool_provider_sdk_records_governed_event() {
    common::assert_extension_contract(
        contract(),
        ToolProviderSdkCommand::record("tool provider registration"),
        append_tool_provider_sdk_event,
    );
}

#[test]
fn tool_provider_sdk_requires_visibility_and_provenance_results() {
    let manifest =
        ToolProviderManifest::new("coc7_sample_tool_provider", "tool_schema.v1", true, true);

    assert!(manifest.is_governed_tool_provider());
}

#[test]
fn tool_provider_sdk_enforces_grant_openfga_opa_and_audit() {
    assert!(authorize_tool_invocation(&ExtensionPolicyGate::allow(ALLOWED_CAPABILITIES)).is_ok());

    for gate in [
        ExtensionPolicyGate::deny_tool_grant(ALLOWED_CAPABILITIES),
        ExtensionPolicyGate::deny_openfga(ALLOWED_CAPABILITIES),
        ExtensionPolicyGate::deny_opa(ALLOWED_CAPABILITIES),
        ExtensionPolicyGate::without_audit(ALLOWED_CAPABILITIES),
    ] {
        let error = authorize_tool_invocation(&gate).expect_err("tool gate must deny");
        assert!(error.code().starts_with("EXTENSION_"));
    }
}
