use trpg_data_eventing::{
    append_canonical_event, canonical_projection_descriptor, ActorRole, AuthorityContract,
    AuthorityMode, CanonicalEvent, CommandEnvelope, DataEventOperation, DataEventPayload, EntityId,
    EventStore, FactProvenance, ProvenanceKind, Visibility, VisibilityLabel,
};

fn command(expected_version: u64, event: CanonicalEvent) -> CommandEnvelope<()> {
    let mut command =
        trpg_test_support::governed_command!((), ActorRole::Workflow, AuthorityMode::AiKp);
    let suffix = event.name().to_ascii_lowercase();
    command.command_id = EntityId::new(format!("command_{suffix}")).unwrap();
    command.idempotency_key = format!("idem_{suffix}");
    command.expected_version = expected_version;
    command.visibility = Visibility::new(VisibilityLabel::SystemOnly);
    command.fact_provenance = FactProvenance::new(
        ProvenanceKind::SystemFixture,
        format!("fact_{suffix}"),
        "canonical_event_registry",
    )
    .unwrap();
    command.correlation_id = EntityId::new(format!("corr_{suffix}")).unwrap();
    command.causation_id = EntityId::new(format!("cause_{suffix}")).unwrap();
    command
}

#[test]
fn canonical_constructor_and_projection_share_the_registry() {
    let authority = AuthorityContract::new("campaign_registry", AuthorityMode::AiKp, 1).unwrap();
    let mut store: EventStore<DataEventPayload> = EventStore::default();

    for (index, canonical) in CanonicalEvent::ALL.iter().copied().enumerate() {
        let event = append_canonical_event(
            &mut store,
            &authority,
            &command(index as u64, canonical),
            "canonical_event_registry",
            canonical,
            DataEventOperation::EventStoreAppend,
            &["event_store"],
        )
        .unwrap();
        let descriptor = canonical_projection_descriptor(&event).unwrap();
        assert_eq!(event.event_type, canonical.name());
        assert_eq!(event.payload.event_name, descriptor.name());
        assert_eq!(descriptor.version(), canonical.version());
        assert_eq!(descriptor.schema_name(), canonical.schema_name());
    }
}

#[test]
fn projection_rejects_an_event_outside_the_registry() {
    let mut store: EventStore<DataEventPayload> = EventStore::default();
    let command = command(0, CanonicalEvent::CampaignCreated);
    let event = store
        .append(
            &command,
            "UnregisteredEvent",
            DataEventPayload {
                module_name: "canonical_event_registry",
                event_name: "UnregisteredEvent",
                operation: DataEventOperation::EventStoreAppend,
                read_models: &["event_store"],
            },
        )
        .unwrap();

    assert!(canonical_projection_descriptor(&event).is_err());
}
