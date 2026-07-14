use trpg_api::contract_core::{
    append_api_contract_event, event_visibility_label, rebuild_api_projection,
    replay_visible_deltas, validate_api_contract, ApiCommandPayload, ApiRealtimeContract,
    ApiRealtimeEventPayload,
};
use trpg_api::{
    ActorRole, AuthorityContract, AuthorityMode, CommandEnvelope, EntityId, EventStore,
    PrincipalScope, Visibility, VisibilityLabel,
};

pub fn human_contract() -> AuthorityContract {
    trpg_test_support::authority_contract("camp_b029", AuthorityMode::HumanKp, 1).unwrap()
}

pub fn command_for(
    api_contract: &ApiRealtimeContract,
    expected_version: u64,
    idempotency_key: &str,
) -> CommandEnvelope<ApiCommandPayload> {
    let payload = ApiCommandPayload {
        module_name: api_contract.module_name,
        operation: api_contract.operation,
        request_summary: "B029 governed API realtime contract check",
    };
    let authority = human_contract();
    let mut command =
        trpg_test_support::governed_command_for_contract(&authority, payload, ActorRole::Workflow);
    command.command_id = EntityId::new(format!("command_{}", api_contract.module_name)).unwrap();
    command.idempotency_key = idempotency_key.to_owned();
    command.expected_version = expected_version;
    command
}

#[allow(dead_code)]
pub fn assert_contract_governance(api_contract: ApiRealtimeContract, expected_prompt_id: &str) {
    trpg_test_support::assert_normalized_prompt_binding(
        "trpg-api",
        api_contract.module_name,
        expected_prompt_id,
    );
    validate_api_contract(&api_contract).unwrap();
    assert!(api_contract.uses_current_safe_names());

    let authority = human_contract();
    let mut store: EventStore<ApiRealtimeEventPayload> = EventStore::default();
    let mut command = command_for(&api_contract, 0, "idem_b029_contract");
    command.visibility = Visibility::new(VisibilityLabel::KeeperOnly);

    let event = append_api_contract_event(&mut store, &authority, &command, &api_contract).unwrap();

    assert_eq!(event.event_type, api_contract.event_type);
    assert_eq!(event.payload.module_name, api_contract.module_name);
    assert_eq!(event_visibility_label(&event), &VisibilityLabel::KeeperOnly);
    assert_eq!(event.fact_provenance.reference.as_str(), "fact_001");
    assert_eq!(event.fact_provenance.recorded_by.as_str(), "rules_001");
    assert_eq!(event.correlation_id.as_str(), "corr_001");
    assert_eq!(event.causation_id.as_str(), "cause_001");

    assert!(replay_visible_deltas(&store, &PrincipalScope::Public).is_empty());
    assert_eq!(
        replay_visible_deltas(&store, &PrincipalScope::Keeper).len(),
        1
    );

    let projection = rebuild_api_projection(store.events());
    assert_eq!(projection.event_count, 1);
    assert_eq!(projection.last_sequence, 1);
    assert!(projection.modules.contains(&api_contract.module_name));
}
