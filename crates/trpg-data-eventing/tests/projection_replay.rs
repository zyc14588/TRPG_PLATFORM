use trpg_data_eventing::{
    append_data_event, rebuild_projection_from_events, replay_visible_data_events,
    DataEventOperation, DataEventPayload, DataEventWrite, EventStore, FactProvenance,
    PrincipalScope, ProvenanceKind, Visibility, VisibilityLabel,
};
use trpg_data_eventing::{ActorRole, AuthorityContract, AuthorityMode, CommandEnvelope, EntityId};

const S03_DETAILED_FIXTURE: &str = include_str!(
    "../../../fixtures/stages/detailed/S03_event_store_projection_hash.current.json.md"
);
const EVENT_STREAM_CASES: &str = include_str!("../../../test-data/event_store_stream_cases.md");

#[test]
fn projection_replay_hash_is_stable_and_event_store_derived() {
    assert!(S03_DETAILED_FIXTURE.contains("\"projection_hash_stable\""));
    assert!(S03_DETAILED_FIXTURE.contains("\"OutboxMessage\""));
    assert!(EVENT_STREAM_CASES.contains("\"SessionSummaryCreated\""));

    let contract = AuthorityContract::new("campaign_projection_001", AuthorityMode::AiKp, 1)
        .expect("valid authority contract");
    let mut store: EventStore<DataEventPayload> = EventStore::default();

    for (index, event_type) in ["CampaignCreated", "AuthorityContractLocked", "SceneStarted"]
        .iter()
        .enumerate()
    {
        append_data_event(
            &mut store,
            &contract,
            &governed_command(index as u64, format!("idem_projection_{index}")),
            DataEventWrite::new(
                "projection_replay",
                event_type,
                DataEventOperation::ProjectionRebuild,
                &["projection_checkpoint", "event_outbox"],
            ),
        )
        .unwrap();
    }

    let first = rebuild_projection_from_events(store.events());
    let second = rebuild_projection_from_events(store.events());
    assert_eq!(first, second);
    assert_eq!(first.event_count, 3);
    assert_eq!(first.last_sequence, 3);
    assert_ne!(first.projection_hash, "0000000000000000");

    append_data_event(
        &mut store,
        &contract,
        &governed_command(3, "idem_projection_3"),
        DataEventWrite::new(
            "projection_replay",
            "EventsAppended",
            DataEventOperation::ProjectionRebuild,
            &["projection_checkpoint", "event_outbox"],
        ),
    )
    .unwrap();
    let after_append = rebuild_projection_from_events(store.events());
    assert_ne!(first.projection_hash, after_append.projection_hash);
    assert_eq!(after_append.last_sequence, 4);
}

#[test]
fn projection_replay_redacts_private_keeper_and_ai_internal_events() {
    let contract = AuthorityContract::new("campaign_visibility_001", AuthorityMode::AiKp, 1)
        .expect("valid authority contract");
    let mut store: EventStore<DataEventPayload> = EventStore::default();
    let player_a = EntityId::new("player_a").unwrap();
    let player_b = EntityId::new("player_b").unwrap();

    append_visible_event(
        &mut store,
        &contract,
        0,
        "idem_private_player_a",
        "PrivateSceneDeltaRecorded",
        Visibility::private_to_player(player_a.clone()),
    );
    append_visible_event(
        &mut store,
        &contract,
        1,
        "idem_keeper_note",
        "KeeperOnlyProjectionRecorded",
        Visibility::new(VisibilityLabel::KeeperOnly),
    );
    append_visible_event(
        &mut store,
        &contract,
        2,
        "idem_ai_internal",
        "AiInternalProjectionRecorded",
        Visibility::new(VisibilityLabel::AiInternal),
    );

    assert_eq!(
        replay_visible_data_events(&store, &PrincipalScope::Player(player_a)).len(),
        1
    );
    assert!(replay_visible_data_events(&store, &PrincipalScope::Player(player_b)).is_empty());
    assert_eq!(
        replay_visible_data_events(&store, &PrincipalScope::Keeper).len(),
        2
    );
    assert_eq!(
        replay_visible_data_events(&store, &PrincipalScope::System).len(),
        3
    );
    assert!(replay_visible_data_events(&store, &PrincipalScope::Public).is_empty());
}

fn append_visible_event(
    store: &mut EventStore<DataEventPayload>,
    contract: &AuthorityContract,
    expected_version: u64,
    idempotency_key: &'static str,
    event_type: &'static str,
    visibility: Visibility,
) {
    append_data_event(
        store,
        contract,
        &governed_command_with_visibility(expected_version, idempotency_key, visibility),
        DataEventWrite::new(
            "projection_replay",
            event_type,
            DataEventOperation::ProjectionRebuild,
            &["projection_checkpoint"],
        ),
    )
    .unwrap();
}

fn governed_command(
    expected_version: u64,
    idempotency_key: impl Into<String>,
) -> CommandEnvelope<()> {
    governed_command_with_visibility(
        expected_version,
        idempotency_key,
        Visibility::new(VisibilityLabel::SystemOnly),
    )
}

fn governed_command_with_visibility(
    expected_version: u64,
    idempotency_key: impl Into<String>,
    visibility: Visibility,
) -> CommandEnvelope<()> {
    let idempotency_key = idempotency_key.into();
    let mut command = CommandEnvelope::governed((), ActorRole::Workflow, AuthorityMode::AiKp);
    command.command_id = EntityId::new(format!("command_{idempotency_key}")).unwrap();
    command.idempotency_key = idempotency_key.clone();
    command.expected_version = expected_version;
    command.visibility = visibility;
    command.fact_provenance = FactProvenance::new(
        ProvenanceKind::SystemFixture,
        "fact_projection",
        "fixture_s03",
    )
    .unwrap();
    command.correlation_id = EntityId::new(format!("corr_{idempotency_key}")).unwrap();
    command.causation_id = EntityId::new(format!("cause_{idempotency_key}")).unwrap();
    command
}
