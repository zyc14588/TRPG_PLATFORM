mod common;

use trpg_extension_sdk::sdk::{
    append_sdk_event, contract, ExtensionSdkManifest, SdkCommand, SdkService,
};

#[test]
fn sdk_records_governed_event() {
    common::assert_extension_contract(
        contract(),
        SdkCommand::record("sdk contract registry"),
        append_sdk_event,
    );
}

#[test]
fn sdk_registry_covers_batch_044_primary_contracts() {
    let manifest = ExtensionSdkManifest::current();
    let contracts = trpg_extension_sdk::extension_contracts();

    assert!(manifest.supports_current_contract_set());
    assert_eq!(contracts.len(), 8);
    assert!(contracts
        .iter()
        .all(|contract| contract.uses_current_safe_names()));
}

#[test]
fn sdk_service_records_contract_registry_event() {
    let authority = common::authority_contract();
    let mut store = trpg_extension_sdk::ExtensionEventStore::default();
    let command = common::governed_command(
        SdkCommand::record("sdk service"),
        0,
        "idem_sdk_service",
        trpg_extension_sdk::Visibility::new(trpg_extension_sdk::VisibilityLabel::SystemOnly),
    );

    let execution = SdkService::default()
        .execute(&mut store, &authority, &command)
        .expect("sdk registry records event");

    assert_eq!(execution.event.event_type, contract().event_type);
    assert!(execution.external_contract.uses_current_safe_names());
}
