
#[test]
fn b026_appends_only_through_governed_event_store_path() {
    let authority_contract = authority_contract(AuthorityMode::AiKp);
    let mut store: EventStore<DataEventPayload> = EventStore::default();

    let first = governed_command(
        api_websocket_nats_contracts::ApiWebsocketNatsContractsCommand::record("api contract"),
        0,
        "idem_b026_api",
        ActorRole::Workflow,
        AuthorityMode::AiKp,
    );
    let event = api_websocket_nats_contracts::append_api_websocket_nats_contracts_event(
        &mut store,
        &authority_contract,
        &first,
    )
    .unwrap();

    assert_eq!(event.sequence, 1);
    assert_eq!(
        event.payload.module_name,
        api_websocket_nats_contracts::MODULE_NAME
    );
    assert_eq!(event.fact_provenance.reference.as_str(), "fact_b026");

    let stale_version = governed_command(
        nats_subjects::NatsSubjectsCommand::record("stale subject update"),
        0,
        "idem_b026_stale",
        ActorRole::Workflow,
        AuthorityMode::AiKp,
    );
    let error =
        nats_subjects::append_nats_subjects_event(&mut store, &authority_contract, &stale_version)
            .unwrap_err();
    assert_eq!(
        error,
        TrpgError::ExpectedVersionConflict {
            expected: 0,
            actual: 1,
        }
    );

    let duplicate = governed_command(
        nats_subject_contracts::NatsSubjectContractsCommand::record("duplicate"),
        1,
        "idem_b026_api",
        ActorRole::Workflow,
        AuthorityMode::AiKp,
    );
    let error = nats_subject_contracts::append_nats_subject_contracts_event(
        &mut store,
        &authority_contract,
        &duplicate,
    )
    .unwrap_err();
    assert_eq!(error.code(), "DUPLICATE_COMMAND");

    let mut direct_agent = governed_command(
        cache_redis_impl::CacheRedisImplCommand::record("agent cache bypass"),
        1,
        "idem_b026_direct_agent",
        ActorRole::Workflow,
        AuthorityMode::AiKp,
    );
    direct_agent.write_path = FormalWritePath::DirectAgent;
    let error = cache_redis_impl::append_cache_redis_impl_event(
        &mut store,
        &authority_contract,
        &direct_agent,
    )
    .unwrap_err();
    assert_eq!(error.code(), "DIRECT_AGENT_STATE_WRITE");

    let bad_actor = governed_command(
        event_bus_nats_impl::EventBusNatsImplCommand::record("human keeper in ai mode"),
        1,
        "idem_b026_bad_actor",
        ActorRole::HumanKeeper,
        AuthorityMode::AiKp,
    );
    let error = event_bus_nats_impl::append_event_bus_nats_impl_event(
        &mut store,
        &authority_contract,
        &bad_actor,
    )
    .unwrap_err();
    assert_eq!(error.code(), "AUTHORITY_VIOLATION");
    assert_eq!(store.events().len(), 1);
}

#[test]
fn b026_visibility_provenance_and_projection_replay_are_preserved() {
    let authority_contract = authority_contract(AuthorityMode::AiKp);
    let mut store: EventStore<DataEventPayload> = EventStore::default();
    let player_a = trpg_data_eventing::EntityId::new("player_b026_a").unwrap();
    let player_b = trpg_data_eventing::EntityId::new("player_b026_b").unwrap();

    let mut private_to_a = governed_command(
        domain_event_sourcing_projection::DomainEventSourcingProjectionCommand::record(
            "private projection",
        ),
        0,
        "idem_b026_private",
        ActorRole::Workflow,
        AuthorityMode::AiKp,
    );
    private_to_a.visibility = Visibility::private_to_player(player_a.clone());
    domain_event_sourcing_projection::append_domain_event_sourcing_projection_event(
        &mut store,
        &authority_contract,
        &private_to_a,
    )
    .unwrap();

    let mut keeper_only = governed_command(
        rag_snapshot::RagSnapshotCommand::record("keeper rag snapshot"),
        1,
        "idem_b026_keeper",
        ActorRole::Workflow,
        AuthorityMode::AiKp,
    );
    keeper_only.visibility = Visibility::new(VisibilityLabel::KeeperOnly);
    rag_snapshot::append_rag_snapshot_event(&mut store, &authority_contract, &keeper_only).unwrap();

    let mut ai_internal = governed_command(
        persistence_postgresql_impl::PersistencePostgresqlImplCommand::record("internal db trace"),
        2,
        "idem_b026_ai_internal",
        ActorRole::Workflow,
        AuthorityMode::AiKp,
    );
    ai_internal.visibility = Visibility::new(VisibilityLabel::AiInternal);
    persistence_postgresql_impl::append_persistence_postgresql_impl_event(
        &mut store,
        &authority_contract,
        &ai_internal,
    )
    .unwrap();

    let first = rebuild_projection_from_events(store.events());
    let second = rebuild_projection_from_events(store.events());
    assert_eq!(first, second);
    assert_eq!(first.event_count, 3);
    assert_eq!(first.last_sequence, 3);
    assert!(store
        .events()
        .iter()
        .all(|event| event.fact_provenance.reference.as_str() == "fact_b026"));

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

fn authority_contract(mode: AuthorityMode) -> AuthorityContract {
    trpg_test_support::authority_contract("campaign_batch_026", mode, 1).unwrap()
}

fn governed_command<T>(
    payload: T,
    expected_version: u64,
    idempotency_key: &str,
    role: ActorRole,
    mode: AuthorityMode,
) -> CommandEnvelope<T> {
    let authority = authority_contract(mode);
    let mut command = trpg_test_support::governed_command_for_contract(&authority, payload, role);
    command.idempotency_key = idempotency_key.to_owned();
    command.expected_version = expected_version;
    command.fact_provenance =
        FactProvenance::new(ProvenanceKind::SystemFixture, "fact_b026", "batch_026").unwrap();
    command
}
