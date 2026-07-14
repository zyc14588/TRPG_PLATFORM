use trpg_data_eventing::{
    append_data_event, rebuild_projection_from_events, replay_visible_data_events,
    DataEventOperation, DataEventPayload, DataEventWrite, EventStore, FactProvenance,
    FormalWritePath, OutboxMessage, PrincipalScope, ProjectionCheckpoint, ProvenanceKind,
    TrpgError, Visibility, VisibilityLabel,
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
    let expected_hash = fixture_string_after("\"hash\": \"");

    let contract =
        trpg_test_support::authority_contract("campaign_projection_001", AuthorityMode::AiKp, 1)
            .expect("valid authority contract");
    let mut store: EventStore<DataEventPayload> = EventStore::default();

    for (index, event_type) in ["CampaignCreated", "AuthorityContractLocked", "SceneStarted"]
        .iter()
        .enumerate()
    {
        append_data_event(
            &mut store,
            &contract,
            &governed_command(&contract, index as u64, format!("idem_projection_{index}")),
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
    assert_eq!(first.projection_hash, expected_hash);

    let outbox = OutboxMessage::from(store.events().first().unwrap());
    assert_eq!(outbox.event_id, 1);
    assert_eq!(outbox.correlation_id.as_str(), "corr_idem_projection_0");
    assert_eq!(outbox.causation_id.as_str(), "cause_idem_projection_0");

    let checkpoint =
        ProjectionCheckpoint::from_snapshot(EntityId::new("campaign_001").unwrap(), &first);
    assert_eq!(checkpoint.stream_id.as_str(), "campaign_001");
    assert_eq!(checkpoint.version, 3);
    assert_eq!(checkpoint.projection_hash, expected_hash);

    let wrong_version = append_data_event(
        &mut store,
        &contract,
        &governed_command(&contract, 2, "idem_wrong_expected_version"),
        DataEventWrite::new(
            "projection_replay",
            "EventsAppended",
            DataEventOperation::ProjectionRebuild,
            &["projection_checkpoint", "event_outbox"],
        ),
    )
    .unwrap_err();
    assert_eq!(
        s03_fixture_error_code(&wrong_version),
        fixture_case_error("wrong_expected_version")
    );

    let duplicate = append_data_event(
        &mut store,
        &contract,
        &governed_command(&contract, 3, "idem_projection_0"),
        DataEventWrite::new(
            "projection_replay",
            "EventsAppended",
            DataEventOperation::ProjectionRebuild,
            &["projection_checkpoint", "event_outbox"],
        ),
    )
    .unwrap_err();
    assert_eq!(
        s03_fixture_error_code(&duplicate),
        fixture_case_error("duplicate_idempotency_key")
    );

    let mut mutable_update = governed_command(&contract, 3, "idem_mutable_event_update");
    mutable_update.write_path = FormalWritePath::DirectBusiness;
    let append_only_error = append_data_event(
        &mut store,
        &contract,
        &mutable_update,
        DataEventWrite::new(
            "projection_replay",
            "MutableEventUpdate",
            DataEventOperation::ProjectionRebuild,
            &["projection_checkpoint"],
        ),
    )
    .unwrap_err();
    assert_eq!(
        s03_fixture_error_code(&append_only_error),
        fixture_failure_error("mutable_event_update")
    );
    assert_eq!(store.events().len(), 3);

    append_data_event(
        &mut store,
        &contract,
        &governed_command(&contract, 3, "idem_projection_3"),
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

fn fixture_string_after(marker: &str) -> String {
    let start = S03_DETAILED_FIXTURE
        .find(marker)
        .expect("fixture marker exists")
        + marker.len();
    let end = S03_DETAILED_FIXTURE[start..]
        .find('"')
        .expect("fixture string closes")
        + start;
    S03_DETAILED_FIXTURE[start..end].to_owned()
}

fn fixture_case_error(case_id: &str) -> &'static str {
    let case_marker = format!("\"case\": \"{case_id}\"");
    let case_start = S03_DETAILED_FIXTURE
        .find(&case_marker)
        .expect("fixture case exists");
    fixture_error_after(case_start, "\"error\": \"")
}

fn fixture_failure_error(failure_id: &str) -> &'static str {
    let failure_marker = format!("\"id\": \"{failure_id}\"");
    let failure_start = S03_DETAILED_FIXTURE
        .find(&failure_marker)
        .expect("fixture failure exists");
    fixture_error_after(failure_start, "\"expected_error\": \"")
}

fn fixture_error_after(start: usize, marker: &str) -> &'static str {
    let marker_start = S03_DETAILED_FIXTURE[start..]
        .find(marker)
        .expect("fixture error marker exists")
        + start
        + marker.len();
    let end = S03_DETAILED_FIXTURE[marker_start..]
        .find('"')
        .expect("fixture error closes")
        + marker_start;
    &S03_DETAILED_FIXTURE[marker_start..end]
}

fn s03_fixture_error_code(error: &TrpgError) -> &'static str {
    match error {
        TrpgError::ExpectedVersionConflict { .. } => "EVENT_STREAM_VERSION_CONFLICT",
        TrpgError::DuplicateCommand => "IDEMPOTENCY_REPLAYED",
        TrpgError::DirectAgentStateWrite | TrpgError::PolicyDenied => "EVENT_STORE_APPEND_ONLY",
        _ => error.code(),
    }
}

#[test]
fn projection_replay_redacts_private_keeper_and_ai_internal_events() {
    let contract =
        trpg_test_support::authority_contract("campaign_visibility_001", AuthorityMode::AiKp, 1)
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
        &governed_command_with_visibility(contract, expected_version, idempotency_key, visibility),
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
    contract: &AuthorityContract,
    expected_version: u64,
    idempotency_key: impl Into<String>,
) -> CommandEnvelope<()> {
    governed_command_with_visibility(
        contract,
        expected_version,
        idempotency_key,
        Visibility::new(VisibilityLabel::SystemOnly),
    )
}

fn governed_command_with_visibility(
    contract: &AuthorityContract,
    expected_version: u64,
    idempotency_key: impl Into<String>,
    visibility: Visibility,
) -> CommandEnvelope<()> {
    let idempotency_key = idempotency_key.into();
    let mut command =
        trpg_test_support::governed_command_for_contract(contract, (), ActorRole::Workflow);
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
