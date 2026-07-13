use trpg_domain_core::ddd::{
    ActorRole, AuthorityMode, EntityId, EventStore, FactProvenance, PrincipalScope, ProvenanceKind,
    Visibility, VisibilityLabel,
};
use trpg_domain_core::event_sourcing_projection::{
    append_rebuildable_canon_event, rebuild_projection_from_events,
    replay_visible_projection_events, CanonEventWrite,
};
use trpg_domain_core::event_sourcing_snapshot_projection::DomainEventPayload;

#[test]
fn event_sourcing_projection_rejects_authority_violation_without_event() {
    let command = trpg_test_support::governed_command!(
        "projection",
        ActorRole::HumanKeeper,
        AuthorityMode::AiKp
    );
    let mut store: EventStore<DomainEventPayload> = EventStore::default();
    let write = CanonEventWrite::new("event_001", "DecisionRecorded");

    let error = append_rebuildable_canon_event(&mut store, &command, write).unwrap_err();

    assert_eq!(error.code(), "AUTHORITY_VIOLATION");
    assert!(store.events().is_empty());
}

#[test]
fn event_sourcing_projection_keeps_visibility_and_fact_provenance_on_replay() {
    let provenance = FactProvenance::new(
        ProvenanceKind::RulesEngineDecision,
        "event_001",
        "rules_001",
    )
    .unwrap();
    let mut command = trpg_test_support::governed_command!(
        "projection",
        ActorRole::Workflow,
        AuthorityMode::AiKp
    );
    command.visibility = Visibility::new(VisibilityLabel::KeeperOnly);
    command.fact_provenance = provenance.clone();
    let mut store: EventStore<DomainEventPayload> = EventStore::default();
    let write = CanonEventWrite::new("event_001", "DecisionRecorded");

    let event = append_rebuildable_canon_event(&mut store, &command, write).unwrap();
    let snapshot = rebuild_projection_from_events(store.events());

    assert_eq!(event.fact_provenance, provenance);
    assert_eq!(snapshot.event_count, 1);
    assert_eq!(
        replay_visible_projection_events(&store, &PrincipalScope::Keeper).len(),
        1
    );
    assert!(replay_visible_projection_events(
        &store,
        &PrincipalScope::Player(EntityId::new("player_001").unwrap())
    )
    .is_empty());
}
