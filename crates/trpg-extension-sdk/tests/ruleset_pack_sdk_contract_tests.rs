mod common;

use trpg_extension_sdk::ruleset_pack_sdk::{
    append_ruleset_pack_sdk_event, contract, RulesetPackManifest, RulesetPackSdkCommand,
};
use trpg_extension_sdk::{ExtensionCapability, ExtensionCapabilityGrantSet};

#[test]
fn ruleset_pack_sdk_records_governed_event() {
    common::assert_extension_contract(
        contract(),
        RulesetPackSdkCommand::record("ruleset pack registration"),
        append_ruleset_pack_sdk_event,
    );
}

#[test]
fn ruleset_pack_sdk_keeps_official_dice_server_side() {
    let manifest = RulesetPackManifest::new("coc7", "7e");

    assert!(manifest.official_dice_must_be_server_generated());
    assert!(!manifest.pack_can_forge_dice());
    assert_eq!(
        ExtensionCapabilityGrantSet::with_grants(&[ExtensionCapability::ForgeDice])
            .expect_err("ruleset pack cannot forge dice")
            .code(),
        "EXTENSION_DICE_FORGE_FORBIDDEN"
    );
}
