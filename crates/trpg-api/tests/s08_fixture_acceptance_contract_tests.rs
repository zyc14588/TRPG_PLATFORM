mod common;

use trpg_api::contract_core::{
    append_api_contract_event, replay_visible_deltas, validate_domain_nats_subject,
    validate_primary_adapter_boundaries, ApiRealtimeEventPayload, REALTIME_REPLAYABLE_EVENTS,
};
use trpg_api::{api_and_transport, EntityId, EventStore, PrincipalScope, TrpgError, Visibility};

const S08_STAGE_FIXTURE: &str =
    include_str!("../../../fixtures/stages/S08_stage_acceptance_fixture.v1.json.md");
const S08_DETAILED_FIXTURE: &str =
    include_str!("../../../fixtures/stages/detailed/S08_api_ws_nats_expected.current.json.md");
const API_WS_SAMPLES: &str = include_str!("../../../test-data/api_ws_contract_samples.md");
const VISIBILITY_LEAKAGE_CASES: &str =
    include_str!("../../../test-data/visibility_leakage_cases.md");

#[test]
fn s08_detailed_fixture_is_bound_to_trpg_api_automated_assertions() {
    assert!(S08_STAGE_FIXTURE.contains("\"stage\": \"S08\""));
    for fixture_value in [
        "POST",
        "/campaigns/{id}/actions",
        "idem_001",
        "player_action",
        "corr_001",
        "campaign_001",
        "campaign.campaign_001.event.created",
        "cargo test -p trpg-api --test s08_fixture_acceptance_contract_tests --all-features",
    ] {
        assert!(S08_DETAILED_FIXTURE.contains(fixture_value));
    }

    for event_type in REALTIME_REPLAYABLE_EVENTS {
        assert!(S08_DETAILED_FIXTURE.contains(event_type.name()));
    }
    for record in ["ApiAuditRecord", "RealtimeDeliveryRecord"] {
        assert!(S08_DETAILED_FIXTURE.contains(record));
    }
    for error in [
        "IDEMPOTENCY_KEY_REQUIRED",
        "REALTIME_VISIBILITY_VIOLATION",
        "NATS_SUBJECT_CONTRACT_VIOLATION",
        "IDEMPOTENCY_CONTRACT_BROKEN",
    ] {
        assert!(S08_DETAILED_FIXTURE.contains(error));
    }

    validate_primary_adapter_boundaries().unwrap();
    validate_domain_nats_subject("campaign.campaign_001.event.created").unwrap();
}

#[test]
fn s08_test_data_keeps_api_ws_nats_and_visibility_contracts_assertable() {
    for subject in [
        "trpg.game.event.appended",
        "trpg.projection.updated",
        "trpg.agent.run.completed",
        "trpg.audit.recorded",
    ] {
        assert!(API_WS_SAMPLES.contains(subject));
        validate_domain_nats_subject(subject).unwrap();
    }

    for private_label in ["keeper_only", "private_to_player", "ai_internal"] {
        assert!(VISIBILITY_LEAKAGE_CASES.contains(private_label));
    }
    assert!(validate_domain_nats_subject("campaign.campaign_001.*").is_err());
    assert!(validate_domain_nats_subject("campaign.legacy.v6.event").is_err());
}

#[test]
fn s08_private_realtime_fixture_path_is_event_store_filtered() {
    let api_contract = api_and_transport::contract();
    let authority = common::human_contract();
    let mut store: EventStore<ApiRealtimeEventPayload> = EventStore::default();
    let player_a = EntityId::new("player_a").unwrap();
    let player_b = EntityId::new("player_b").unwrap();
    let mut command = common::command_for(&api_contract, 0, "idem_001");
    command.visibility = Visibility::private_to_player(player_a.clone());

    append_api_contract_event(&mut store, &authority, &command, &api_contract).unwrap();
    assert_eq!(
        replay_visible_deltas(&store, &PrincipalScope::Player(player_a))
            .unwrap()
            .len(),
        1
    );
    assert!(
        replay_visible_deltas(&store, &PrincipalScope::Player(player_b))
            .unwrap()
            .is_empty()
    );
    assert!(replay_visible_deltas(&store, &PrincipalScope::Public)
        .unwrap()
        .is_empty());
}

#[test]
fn s08_missing_idempotency_fixture_error_is_enforced() {
    let api_contract = api_and_transport::contract();
    let authority = common::human_contract();
    let mut store: EventStore<ApiRealtimeEventPayload> = EventStore::default();
    let command = common::command_for(&api_contract, 0, "");

    assert_eq!(
        append_api_contract_event(&mut store, &authority, &command, &api_contract).unwrap_err(),
        TrpgError::MissingIdempotencyKey
    );
    assert!(store.events().is_empty());
}
