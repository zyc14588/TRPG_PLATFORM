mod common;

use trpg_extension_sdk::plugin_sdk::{
    append_plugin_sdk_event, contract, PluginManifest, PluginSdkCommand, PluginSdkService,
    ALLOWED_CAPABILITIES,
};
use trpg_extension_sdk::{
    ExtensionCapability, ExtensionCapabilityGrantSet, ExtensionPolicyGate, ExtensionSdkError,
};

#[test]
fn plugin_sdk_records_governed_event() {
    common::assert_extension_contract(
        contract(),
        PluginSdkCommand::record("plugin registration"),
        append_plugin_sdk_event,
    );
}

#[test]
fn plugin_sdk_default_denies_capabilities_until_granted() {
    let plugin = PluginManifest::new("coc7_plugin", vec![ExtensionCapability::InvokeGrantedTool]);
    let grants = ExtensionCapabilityGrantSet::default();

    let error = plugin
        .register(&grants)
        .expect_err("plugin capabilities are default deny");

    assert_eq!(error.code(), "EXTENSION_CAPABILITY_DENIED");
}

#[test]
fn plugin_sdk_rejects_direct_llm_and_event_store_write() {
    for capability in [
        ExtensionCapability::DirectLlm,
        ExtensionCapability::AppendEventStore,
        ExtensionCapability::DatabaseWrite,
    ] {
        let error = ExtensionCapabilityGrantSet::with_grants(&[capability])
            .expect_err("forbidden plugin capability cannot be granted");
        assert_eq!(
            error,
            ExtensionSdkError::ForbiddenCapability(capability.denial_code())
        );
    }
}

#[test]
fn plugin_sdk_service_requires_tool_grant_policy_and_audit() {
    let authority = common::authority_contract();
    let mut store = trpg_extension_sdk::ExtensionEventStore::default();
    let command = common::governed_command(
        PluginSdkCommand::record("plugin deny"),
        0,
        "idem_plugin_denied",
        trpg_extension_sdk::Visibility::new(trpg_extension_sdk::VisibilityLabel::SystemOnly),
    );

    for gate in [
        ExtensionPolicyGate::default_deny(ALLOWED_CAPABILITIES),
        ExtensionPolicyGate::deny_tool_grant(ALLOWED_CAPABILITIES),
        ExtensionPolicyGate::deny_openfga(ALLOWED_CAPABILITIES),
        ExtensionPolicyGate::deny_opa(ALLOWED_CAPABILITIES),
        ExtensionPolicyGate::without_audit(ALLOWED_CAPABILITIES),
    ] {
        let error = PluginSdkService::new(gate)
            .execute(&mut store, &authority, &command)
            .expect_err("policy gate failure blocks plugin event");
        assert!(error.code().starts_with("EXTENSION_"));
    }

    assert!(store.events().is_empty());
}
