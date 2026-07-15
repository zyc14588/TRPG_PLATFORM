mod common;

use trpg_extension_sdk::adr_0008_plugin_boundaries::{
    append_adr_0008_plugin_boundaries_event, contract, Adr0008PluginBoundariesCommand,
    PluginBoundaryPolicy,
};
use trpg_extension_sdk::ExtensionCapability;

#[test]
fn adr_0008_plugin_boundaries_records_governed_event() {
    common::assert_extension_contract(
        contract(),
        Adr0008PluginBoundariesCommand::record("plugin boundary policy"),
        append_adr_0008_plugin_boundaries_event,
    );
}

#[test]
fn adr_0008_plugin_boundaries_forbid_internal_bypasses() {
    let policy = PluginBoundaryPolicy::current();

    for capability in [
        ExtensionCapability::DirectLlm,
        ExtensionCapability::DatabaseWrite,
        ExtensionCapability::AppendEventStore,
        ExtensionCapability::InternalToolGateAccess,
        ExtensionCapability::ModifyAuthorityContract,
        ExtensionCapability::RevealRestrictedVisibility,
    ] {
        assert_eq!(
            policy
                .validate(capability)
                .expect_err("boundary policy rejects forbidden capability")
                .code(),
            capability.denial_code().as_str()
        );
    }

    assert!(policy.validate(ExtensionCapability::ReadProjection).is_ok());
}
