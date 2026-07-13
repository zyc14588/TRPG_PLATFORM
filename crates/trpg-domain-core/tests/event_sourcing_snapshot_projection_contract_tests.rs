use trpg_domain_core::ddd::{ActorRole, AuthorityMode, EventStore};
use trpg_domain_core::event_sourcing_snapshot_projection::{
    append_domain_event, rebuild_projection,
};

#[test]
fn event_sourcing_projection_rebuilds_from_event_log_without_new_canon_events() {
    let mut store = EventStore::default();
    let command_one =
        trpg_test_support::governed_command!("payload", ActorRole::Workflow, AuthorityMode::AiKp);
    append_domain_event(
        &mut store,
        &command_one,
        "event_authority_rejected",
        "AuthorityMutationRejected",
    )
    .unwrap();

    let mut command_two =
        trpg_test_support::governed_command!("payload", ActorRole::Workflow, AuthorityMode::AiKp);
    command_two.command_id = trpg_domain_core::ddd::EntityId::new("command_002").unwrap();
    command_two.idempotency_key = "idem_002".to_owned();
    command_two.expected_version = 1;
    append_domain_event(&mut store, &command_two, "event_forked", "CampaignForked").unwrap();

    let snapshot = rebuild_projection(store.events());

    assert_eq!(snapshot.event_count, 2);
    assert_eq!(snapshot.last_sequence, 2);
    assert_eq!(store.events().len(), 2);
    assert!(!snapshot.projection_hash.is_empty());
}
