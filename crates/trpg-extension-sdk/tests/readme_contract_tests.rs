mod common;

use trpg_extension_sdk::readme::{append_readme_event, contract};
use trpg_extension_sdk::{ExtensionCommand, ExtensionOperation};

#[test]
fn readme_records_governed_event() {
    common::assert_extension_contract(
        contract(),
        ExtensionCommand::record(
            ExtensionOperation::Readme,
            "readme contract",
            "extensions/readme",
            vec![trpg_extension_sdk::ExtensionCapability::ReadProjection],
        ),
        append_readme_event,
    );
}

#[test]
fn readme_registry_keeps_all_batch_044_names_current_safe() {
    let contracts = trpg_extension_sdk::extension_contracts();

    assert_eq!(contracts.len(), 8);
    assert!(contracts
        .iter()
        .all(|contract| contract.uses_current_safe_names()));
}

#[test]
fn readme_redacts_restricted_visibility_on_replay_outputs() {
    common::assert_visibility_redaction(
        ExtensionCommand::record(
            ExtensionOperation::Readme,
            "visibility replay",
            "extensions/readme",
            vec![trpg_extension_sdk::ExtensionCapability::ReadProjection],
        ),
        append_readme_event,
    );
}
