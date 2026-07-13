use trpg_data_eventing::{
    adr_0005_postgres_pgvector, all_data_event_contracts, rebuild_projection_from_events,
    replay_visible_data_events, ActorRole, AuthorityContract, AuthorityMode, CommandEnvelope,
    DataEventPayload, EventStore, FactProvenance, FormalWritePath, PrincipalScope, ProvenanceKind,
    TrpgError, Visibility, VisibilityLabel, EVENT_STORE_TABLE, OUTBOX_TABLE,
};

#[test]
fn adr_0005_contract_uses_b027_current_safe_owner_and_module() {
    let contract = adr_0005_postgres_pgvector::contract();

    assert_eq!(contract.module_name, "adr_0005_postgres_pgvector");
    assert_eq!(contract.event_store_table, EVENT_STORE_TABLE);
    assert_eq!(contract.outbox_table, OUTBOX_TABLE);
    assert!(contract.uses_current_safe_names());
    assert!(contract
        .required_command_fields
        .contains(&"idempotency_key"));
    assert!(contract
        .required_command_fields
        .contains(&"expected_version"));
    assert!(contract.required_event_fields.contains(&"visibility"));
    assert!(contract.required_event_fields.contains(&"fact_provenance"));
    assert!(contract.projection_name.contains("pgvector"));

    let all = all_data_event_contracts();
    let b027 = all
        .iter()
        .find(|contract| contract.module_name == "adr_0005_postgres_pgvector")
        .expect("ADR-0005 contract is registered");
    assert_eq!(b027.module_name, "adr_0005_postgres_pgvector");
}

#[test]
fn adr_0005_appends_pgvector_decision_as_rebuildable_read_model_event() {
    let contract = authority_contract();
    let mut store: EventStore<DataEventPayload> = EventStore::default();
    let mut command = governed_command(0, "idem_adr0005_record");
    command.visibility = Visibility::new(VisibilityLabel::SystemOnly);
    command.fact_provenance = FactProvenance::new(
        ProvenanceKind::SystemFixture,
        "fact_adr0005",
        "fixture_adr0005",
    )
    .unwrap();

    let event = adr_0005_postgres_pgvector::append_adr_0005_postgres_pgvector_event(
        &mut store, &contract, &command,
    )
    .unwrap();
    let snapshot = rebuild_projection_from_events(store.events());

    assert_eq!(event.sequence, 1);
    assert_eq!(event.payload.module_name, "adr_0005_postgres_pgvector");
    assert!(event.payload.read_models.contains(&"rag_index"));
    assert!(event.payload.read_models.contains(&"snapshot_store"));
    assert_eq!(event.fact_provenance.reference.as_str(), "fact_adr0005");
    assert_eq!(snapshot.event_count, 1);
    assert_eq!(
        replay_visible_data_events(&store, &PrincipalScope::System).len(),
        1
    );
    assert!(replay_visible_data_events(&store, &PrincipalScope::Public).is_empty());
}

#[test]
fn adr_0005_rejects_authority_version_idempotency_and_business_bypass_failures() {
    let contract = authority_contract();
    let mut store: EventStore<DataEventPayload> = EventStore::default();

    adr_0005_postgres_pgvector::append_adr_0005_postgres_pgvector_event(
        &mut store,
        &contract,
        &governed_command(0, "idem_adr0005_once"),
    )
    .unwrap();

    let stale = adr_0005_postgres_pgvector::append_adr_0005_postgres_pgvector_event(
        &mut store,
        &contract,
        &governed_command(0, "idem_adr0005_stale"),
    )
    .unwrap_err();
    assert_eq!(
        stale,
        TrpgError::ExpectedVersionConflict {
            expected: 0,
            actual: 1,
        }
    );

    let duplicate = adr_0005_postgres_pgvector::append_adr_0005_postgres_pgvector_event(
        &mut store,
        &contract,
        &governed_command(1, "idem_adr0005_once"),
    )
    .unwrap_err();
    assert_eq!(duplicate, TrpgError::DuplicateCommand);

    let mut direct_business = governed_command(1, "idem_adr0005_direct");
    direct_business.write_path = FormalWritePath::DirectBusiness;
    let error = adr_0005_postgres_pgvector::append_adr_0005_postgres_pgvector_event(
        &mut store,
        &contract,
        &direct_business,
    )
    .unwrap_err();
    assert_eq!(error, TrpgError::PolicyDenied);
    assert_eq!(store.events().len(), 1);
}

fn authority_contract() -> AuthorityContract {
    AuthorityContract::new("campaign_b027_adr0005", AuthorityMode::AiKp, 1).unwrap()
}

fn governed_command(
    expected_version: u64,
    idempotency_key: &str,
) -> CommandEnvelope<adr_0005_postgres_pgvector::Adr0005PostgresPgvectorCommand> {
    let mut command = trpg_test_support::governed_command!(
        adr_0005_postgres_pgvector::Adr0005PostgresPgvectorCommand::record("B027 ADR-0005"),
        ActorRole::Workflow,
        AuthorityMode::AiKp,
    );
    command.command_id =
        trpg_data_eventing::EntityId::new(format!("command_{idempotency_key}")).unwrap();
    command.idempotency_key = idempotency_key.to_owned();
    command.expected_version = expected_version;
    command.correlation_id =
        trpg_data_eventing::EntityId::new(format!("corr_{idempotency_key}")).unwrap();
    command.causation_id =
        trpg_data_eventing::EntityId::new(format!("cause_{idempotency_key}")).unwrap();
    command
}
