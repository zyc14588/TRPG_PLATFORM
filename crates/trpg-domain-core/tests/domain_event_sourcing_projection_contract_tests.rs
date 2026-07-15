use trpg_domain_core::ddd::{
    ActorRole, AuthorityMode, EntityId, EventStore, PrincipalScope, Visibility,
};
use trpg_domain_core::domain_event_sourcing_projection::{
    append_canon_event, rebuild_canon_projection, replay_visible_canon_events,
};
use trpg_domain_core::event_sourcing_snapshot_projection::DomainEventPayload;

#[test]
fn domain_event_sourcing_projection_rebuilds_from_canon_events() {
    let command =
        trpg_test_support::governed_command("event", ActorRole::Workflow, AuthorityMode::AiKp);
    let mut store: EventStore<DomainEventPayload> = EventStore::default();

    append_canon_event(&mut store, &command, "event_001", "DecisionRecorded").unwrap();

    let snapshot = rebuild_canon_projection(store.events());
    assert_eq!(snapshot.event_count, 1);
    assert_eq!(snapshot.last_sequence, 1);
    assert!(!snapshot.projection_hash.is_empty());
}

#[test]
fn domain_event_sourcing_projection_replay_respects_visibility() {
    let player_id = EntityId::new("player_a").unwrap();
    let mut command =
        trpg_test_support::governed_command("private", ActorRole::Workflow, AuthorityMode::AiKp);
    command.visibility = Visibility::private_to_player(player_id.clone());
    let mut store: EventStore<DomainEventPayload> = EventStore::default();

    append_canon_event(&mut store, &command, "event_001", "PrivateSceneDelta").unwrap();

    assert_eq!(
        replay_visible_canon_events(&store, &PrincipalScope::Player(player_id)).len(),
        1
    );
    assert!(replay_visible_canon_events(
        &store,
        &PrincipalScope::Player(EntityId::new("player_b").unwrap())
    )
    .is_empty());
}
