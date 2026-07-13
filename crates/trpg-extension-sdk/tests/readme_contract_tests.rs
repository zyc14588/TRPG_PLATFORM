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
            "evidence/batches/BATCH-044/readme.md",
            vec![trpg_extension_sdk::ExtensionCapability::ReadProjection],
        ),
        append_readme_event,
    );
}

#[test]
fn readme_registry_keeps_all_extension_names_current_safe() {
    let contracts = trpg_extension_sdk::all_extension_contracts();

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
            "evidence/batches/BATCH-044/readme.md",
            vec![trpg_extension_sdk::ExtensionCapability::ReadProjection],
        ),
        append_readme_event,
    );
}
