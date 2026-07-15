use trpg_data_eventing::{
    adr_0002_event_sourcing_cqrs, all_data_event_contracts, rebuild_projection_from_events,
    replay_visible_data_events, ActorRole, AuthorityContract, AuthorityMode, CommandEnvelope,
    DataEventPayload, EventStore, FactProvenance, FormalWritePath, PrincipalScope, ProvenanceKind,
    TrpgError, Visibility, VisibilityLabel, EVENT_STORE_TABLE, OUTBOX_TABLE,
};

#[test]
fn adr_0002_contract_uses_b027_current_safe_owner_and_module() {
    let contract = adr_0002_event_sourcing_cqrs::contract();

    assert_eq!(contract.module_name, "adr_0002_event_sourcing_cqrs");
    trpg_test_support::assert_normalized_prompt_binding(
        "trpg-data-eventing",
        contract.module_name,
        "CODEX-0661-06-DATA-EVENTING-d4c088ceeb",
    );
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

    let all = all_data_event_contracts();
    let b027 = all
        .iter()
        .find(|contract| contract.module_name == "adr_0002_event_sourcing_cqrs")
        .expect("B027 ADR-0002 primary contract is registered");
    assert_eq!(b027.module_name, "adr_0002_event_sourcing_cqrs");
}

#[test]
fn adr_0002_appends_only_governed_events_and_preserves_metadata() {
    let contract = authority_contract();
    let mut store: EventStore<DataEventPayload> = EventStore::default();
    let mut command = governed_command(0, "idem_adr0002_record");
    command.visibility = Visibility::new(VisibilityLabel::KeeperOnly);
    command.fact_provenance =
        FactProvenance::new(ProvenanceKind::ToolResult, "fact_adr0002", "tool_adr0002").unwrap();

    let event = adr_0002_event_sourcing_cqrs::append_adr_0002_event_sourcing_cqrs_event(
        &mut store, &contract, &command,
    )
    .unwrap();
    let snapshot = rebuild_projection_from_events(store.events());

    assert_eq!(event.sequence, 1);
    assert_eq!(event.payload.module_name, "adr_0002_event_sourcing_cqrs");
    assert_eq!(
        event.visibility,
        Visibility::new(VisibilityLabel::KeeperOnly)
    );
    assert_eq!(event.fact_provenance.reference.as_str(), "fact_adr0002");
    assert_eq!(snapshot.event_count, 1);
    assert_eq!(snapshot.last_sequence, 1);
    assert_eq!(
        replay_visible_data_events(&store, &PrincipalScope::Keeper).len(),
        1
    );
    assert!(replay_visible_data_events(&store, &PrincipalScope::Public).is_empty());
}

#[test]
fn adr_0002_rejects_authority_version_idempotency_and_bypass_failures() {
    let contract = authority_contract();
    let mut store: EventStore<DataEventPayload> = EventStore::default();

    adr_0002_event_sourcing_cqrs::append_adr_0002_event_sourcing_cqrs_event(
        &mut store,
        &contract,
        &governed_command(0, "idem_adr0002_once"),
    )
    .unwrap();

    let stale = adr_0002_event_sourcing_cqrs::append_adr_0002_event_sourcing_cqrs_event(
        &mut store,
        &contract,
        &governed_command(0, "idem_adr0002_stale"),
    )
    .unwrap_err();
    assert_eq!(
        stale,
        TrpgError::ExpectedVersionConflict {
            expected: 0,
            actual: 1,
        }
    );

    let duplicate = adr_0002_event_sourcing_cqrs::append_adr_0002_event_sourcing_cqrs_event(
        &mut store,
        &contract,
        &governed_command(1, "idem_adr0002_once"),
    )
    .unwrap_err();
    assert_eq!(duplicate, TrpgError::DuplicateCommand);

    let mut direct_agent = governed_command(1, "idem_adr0002_direct");
    direct_agent.write_path = FormalWritePath::DirectAgent;
    let error = adr_0002_event_sourcing_cqrs::append_adr_0002_event_sourcing_cqrs_event(
        &mut store,
        &contract,
        &direct_agent,
    )
    .unwrap_err();
    assert_eq!(error, TrpgError::DirectAgentStateWrite);
    assert_eq!(store.events().len(), 1);
}

fn authority_contract() -> AuthorityContract {
    trpg_test_support::authority_contract("campaign_b027_adr0002", AuthorityMode::AiKp, 1).unwrap()
}

fn governed_command(
    expected_version: u64,
    idempotency_key: &str,
) -> CommandEnvelope<adr_0002_event_sourcing_cqrs::Adr0002EventSourcingCqrsCommand> {
    let authority = authority_contract();
    let mut command = trpg_test_support::governed_command_for_contract(
        &authority,
        adr_0002_event_sourcing_cqrs::Adr0002EventSourcingCqrsCommand::record("B027 ADR-0002"),
        ActorRole::Workflow,
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
