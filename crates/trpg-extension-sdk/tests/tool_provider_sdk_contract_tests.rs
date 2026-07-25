mod common;

use trpg_extension_sdk::tool_provider_sdk::{
    append_tool_provider_sdk_event, contract, ToolProviderManifest, ToolProviderSdkCommand,
    ALLOWED_CAPABILITIES,
};
use trpg_extension_sdk::{ExtensionCapabilityGrantSet, ExtensionPolicyGate};

#[test]
fn tool_provider_sdk_records_governed_event() {
    common::assert_extension_contract(
        contract(),
        ToolProviderSdkCommand::record("tool provider registration"),
        append_tool_provider_sdk_event,
    );
}

#[test]
fn tool_provider_sdk_requires_host_owned_visibility_and_provenance() {
    let governed =
        ToolProviderManifest::new("coc7_sample_tool_provider", "tool_schema.v1", false, false);
    let self_classifying =
        ToolProviderManifest::new("self_classifying_provider", "tool_schema.v1", true, false);
    let self_attesting =
        ToolProviderManifest::new("self_attesting_provider", "tool_schema.v1", false, true);

    assert!(governed.is_governed_tool_provider());
    assert!(!self_classifying.is_governed_tool_provider());
    assert!(!self_attesting.is_governed_tool_provider());
}

#[test]
fn tool_provider_sdk_capabilities_are_default_deny() {
    assert!(ExtensionPolicyGate::default_deny(ALLOWED_CAPABILITIES)
        .authorize()
        .is_err());
    let grants = ExtensionCapabilityGrantSet::with_grants(ALLOWED_CAPABILITIES).unwrap();
    assert!(
        ExtensionPolicyGate::with_capability_grants(grants, ALLOWED_CAPABILITIES)
            .unwrap()
            .authorize()
            .is_ok()
    );
}
