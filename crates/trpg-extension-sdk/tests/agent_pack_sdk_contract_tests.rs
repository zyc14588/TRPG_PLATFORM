mod common;

use trpg_extension_sdk::agent_pack_sdk::{
    append_agent_pack_sdk_event, contract, AgentPackManifest, AgentPackSdkCommand,
    AgentPackSdkService,
};

#[test]
fn agent_pack_sdk_records_governed_event() {
    common::assert_extension_contract(
        contract(),
        AgentPackSdkCommand::record("agent pack registration"),
        append_agent_pack_sdk_event,
    );
}

#[test]
fn agent_pack_sdk_requires_gateway_and_level_four_provider_for_keeper() {
    let manifest = AgentPackManifest::fixture();

    assert!(manifest.can_run_keeper_orchestrator());
    assert!(!manifest.direct_model_access_allowed());
}

#[test]
fn agent_pack_sdk_service_records_observability() {
    let authority = common::authority_contract();
    let mut store = trpg_extension_sdk::ExtensionEventStore::default();
    let command = common::governed_command(
        AgentPackSdkCommand::record("agent pack service"),
        0,
        "idem_agent_pack_service",
        trpg_extension_sdk::Visibility::new(trpg_extension_sdk::VisibilityLabel::SystemOnly),
    );

    let execution = AgentPackSdkService::default()
        .execute(&mut store, &authority, &command)
        .expect("policy-approved agent pack records event");

    assert_eq!(execution.event.event_type, contract().event_type);
    assert!(execution.external_contract.uses_current_safe_names());
    assert_eq!(
        execution.observability.correlation_id,
        command.correlation_id.as_str()
    );
}
